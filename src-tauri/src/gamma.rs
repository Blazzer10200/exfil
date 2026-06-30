//! Windows GDI gamma ramp control via Get/SetDeviceGammaRamp (gdi32).
//!
//! A gamma ramp is 3×256 u16 entries (R,G,B). We derive a ramp from three
//! perceptual dials — gamma (mid-tone curve), brightness (offset), contrast
//! (pivot around 0.5) — matching the EXFIL v1 ColorEngine math.
//!
//! NO injection — SetDeviceGammaRamp is a standard display-driver call.

#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{CreateDCW, DeleteDC, HDC};
#[cfg(windows)]
use windows::Win32::UI::ColorSystem::{GetDeviceGammaRamp, SetDeviceGammaRamp};
#[cfg(windows)]
use windows::core::PCWSTR;

pub type GammaRamp = [[u16; 256]; 3];

#[derive(serde::Deserialize, serde::Serialize, Clone, Copy, Debug)]
pub struct ColorDials {
    /// 0.30 ..= 2.80, 1.0 = neutral
    pub gamma: f64,
    /// -0.50 ..= 0.50, 0 = neutral
    pub brightness: f64,
    /// 0.50 ..= 2.00, 1.0 = neutral
    pub contrast: f64,
}

impl Default for ColorDials {
    fn default() -> Self {
        ColorDials { gamma: 1.0, brightness: 0.0, contrast: 1.0 }
    }
}

/// Build a 3×256 gamma ramp from the dials. Same channel for R/G/B (neutral).
pub fn build_ramp(d: ColorDials) -> GammaRamp {
    let gamma = d.gamma.clamp(0.30, 2.80);
    let bright = d.brightness.clamp(-0.50, 0.50);
    let contrast = d.contrast.clamp(0.50, 2.00);
    let inv_gamma = 1.0 / gamma;

    let mut channel = [0u16; 256];
    for (i, slot) in channel.iter_mut().enumerate() {
        let mut v = i as f64 / 255.0;
        // contrast pivots around mid-grey
        v = (v - 0.5) * contrast + 0.5;
        // brightness offset
        v += bright;
        // gamma curve
        v = v.clamp(0.0, 1.0).powf(inv_gamma);
        let out = (v * 65535.0).round().clamp(0.0, 65535.0) as u16;
        *slot = out;
    }
    [channel, channel, channel]
}

/// Open a gamma-capable DC for every real monitor.
///
/// We probe `\\.\DISPLAY1..16` directly rather than `EnumDisplayDevicesW(null,…)`
/// — on multi-GPU / mixed setups that enumeration returns zero adapters, and the
/// bare `"DISPLAY"` DC reports success but silently *cannot* read/write gamma
/// (the cause of the v2 "stuck grayscale on one monitor" bug). A DC is kept only
/// if `GetDeviceGammaRamp` actually succeeds on it, so callers only ever touch
/// outputs that genuinely support ramp control.
#[cfg(windows)]
fn display_dcs() -> Vec<HDC> {
    let mut dcs = Vec::new();
    for n in 1..=16u32 {
        let name: Vec<u16> = format!("\\\\.\\DISPLAY{n}\0").encode_utf16().collect();
        let dc = unsafe {
            CreateDCW(PCWSTR(name.as_ptr()), PCWSTR::null(), PCWSTR::null(), None)
        };
        if dc.is_invalid() {
            continue;
        }
        // Confirm the DC can actually do gamma before trusting it.
        let mut probe: GammaRamp = [[0u16; 256]; 3];
        let ok = unsafe { GetDeviceGammaRamp(dc, probe.as_mut_ptr() as *mut _) };
        if ok.as_bool() {
            dcs.push(dc);
        } else {
            unsafe { let _ = DeleteDC(dc); }
        }
    }
    dcs
}
#[cfg(windows)]
pub fn set_ramp(ramp: &GammaRamp) -> Result<(), String> {
    let dcs = display_dcs();
    if dcs.is_empty() {
        return Err("no display DC available".into());
    }
    let mut any_ok = false;
    let mut last_err = String::new();
    for dc in &dcs {
        let ok = unsafe { SetDeviceGammaRamp(*dc, ramp.as_ptr() as *const _) };
        if ok.as_bool() {
            any_ok = true;
        } else {
            last_err = "SetDeviceGammaRamp failed (driver may clamp extreme ramps)".into();
        }
    }
    for dc in dcs {
        unsafe { let _ = DeleteDC(dc); };
    }
    if any_ok {
        // Remember what we last pushed so the re-apply pulse can re-assert it.
        if let Ok(mut last) = LAST_RAMP.lock() {
            *last = Some(*ramp);
        }
        Ok(())
    } else {
        Err(last_err)
    }
}

/// Apply color dials directly.
#[cfg(windows)]
pub fn apply_dials(d: ColorDials) -> Result<(), String> {
    set_ramp(&build_ramp(d))
}

/// Reset to a linear (neutral) ramp.
#[cfg(windows)]
pub fn reset() -> Result<(), String> {
    apply_dials(ColorDials::default())
}

// ── Re-apply pulse ──
// Fullscreen-exclusive games can steal the gamma ramp on focus. We periodically
// re-assert the last ramp we applied so the active preset survives alt-tabs and
// game launches. The pulse is a no-op until a ramp has been applied.

#[cfg(windows)]
static LAST_RAMP: std::sync::Mutex<Option<GammaRamp>> = std::sync::Mutex::new(None);
#[cfg(windows)]
static PULSE_RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Re-assert the last applied ramp once. Cheap; safe to call on a timer.
#[cfg(windows)]
pub fn reapply_last() {
    let ramp = LAST_RAMP.lock().ok().and_then(|g| *g);
    if let Some(ramp) = ramp {
        let _ = set_ramp(&ramp);
    }
}

/// Start the background re-apply pulse (idempotent). `interval_ms` is clamped to
/// 250..=60000 to match v1 behaviour.
#[cfg(windows)]
pub fn start_pulse(interval_ms: u64) {
    use std::sync::atomic::Ordering;
    if PULSE_RUNNING.swap(true, Ordering::SeqCst) {
        return; // already running
    }
    let interval = interval_ms.clamp(250, 60_000);
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(interval));
        reapply_last();
    });
}

// ── non-Windows stubs so the crate still type-checks elsewhere ──
#[cfg(not(windows))]
pub fn set_ramp(_r: &GammaRamp) -> Result<(), String> { Err("gamma: Windows only".into()) }
#[cfg(not(windows))]
pub fn apply_dials(_d: ColorDials) -> Result<(), String> { Err("gamma: Windows only".into()) }
#[cfg(not(windows))]
pub fn reset() -> Result<(), String> { Err("gamma: Windows only".into()) }
#[cfg(not(windows))]
pub fn reapply_last() {}
#[cfg(not(windows))]
pub fn start_pulse(_interval_ms: u64) {}
