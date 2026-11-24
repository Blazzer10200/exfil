//! Windows GDI gamma ramp control via Get/SetDeviceGammaRamp (gdi32).
//!
//! A gamma ramp is 3×256 u16 entries (R,G,B). We derive a ramp from three
//! perceptual dials — gamma (mid-tone curve), brightness (offset), contrast
//! (pivot around 0.5) — matching the EXFIL v1 ColorEngine math.
//!
//! NO injection — SetDeviceGammaRamp is a standard display-driver call.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{CreateDCW, DeleteDC, HDC};
use windows::Win32::UI::ColorSystem::{GetDeviceGammaRamp, SetDeviceGammaRamp};

type GammaRamp = [[u16; 256]; 3];

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
        ColorDials {
            gamma: 1.0,
            brightness: 0.0,
            contrast: 1.0,
        }
    }
}

/// Build a 3×256 gamma ramp from the dials. Same channel for R/G/B (neutral).
fn build_ramp(d: ColorDials) -> GammaRamp {
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
fn display_dcs() -> Vec<HDC> {
    let mut dcs = Vec::new();
    for n in 1..=16u32 {
        let name: Vec<u16> = format!("\\\\.\\DISPLAY{n}\0").encode_utf16().collect();
        let dc = unsafe { CreateDCW(PCWSTR(name.as_ptr()), PCWSTR::null(), PCWSTR::null(), None) };
        if dc.is_invalid() {
            continue;
        }
        // Confirm the DC can actually do gamma before trusting it.
        let mut probe: GammaRamp = [[0u16; 256]; 3];
        let ok = unsafe { GetDeviceGammaRamp(dc, probe.as_mut_ptr() as *mut _) };
        if ok.as_bool() {
            dcs.push(dc);
        } else {
            unsafe {
                let _ = DeleteDC(dc);
            }
        }
    }
    dcs
}

/// Push a ramp to every gamma-capable monitor. Succeeds if at least one took it.
fn write_ramp(ramp: &GammaRamp) -> Result<(), String> {
    let dcs = display_dcs();
    if dcs.is_empty() {
        return Err("no display DC available".into());
    }
    let mut any_ok = false;
    for dc in dcs {
        unsafe {
            any_ok |= SetDeviceGammaRamp(dc, ramp.as_ptr() as *const _).as_bool();
            let _ = DeleteDC(dc);
        }
    }
    if any_ok {
        Ok(())
    } else {
        Err("SetDeviceGammaRamp failed (driver may clamp extreme ramps)".into())
    }
}

// ── Re-apply pulse ──
// Fullscreen-exclusive games can steal the gamma ramp on focus. We periodically
// re-assert the last ramp we applied so the active preset survives alt-tabs and
// game launches. The pulse is a no-op until a ramp has been applied.

/// The ramp most recently applied. Held for the whole hardware write on both
/// paths (apply and pulse), so the pulse can never read an old ramp, lose the
/// race to a newer apply or the exit-time reset, and then write the old one
/// back over it.
static LAST_RAMP: Mutex<Option<GammaRamp>> = Mutex::new(None);
static PULSE_RUNNING: AtomicBool = AtomicBool::new(false);

fn last_ramp() -> std::sync::MutexGuard<'static, Option<GammaRamp>> {
    LAST_RAMP.lock().unwrap_or_else(|e| e.into_inner())
}

/// Apply a ramp and remember it for the re-apply pulse.
fn set_ramp(ramp: &GammaRamp) -> Result<(), String> {
    let mut last = last_ramp();
    write_ramp(ramp)?;
    *last = Some(*ramp);
    Ok(())
}

/// Apply color dials directly.
pub fn apply_dials(d: ColorDials) -> Result<(), String> {
    set_ramp(&build_ramp(d))
}

/// Reset to a linear (neutral) ramp.
pub fn reset() -> Result<(), String> {
    apply_dials(ColorDials::default())
}

/// Start the background re-apply pulse (idempotent). `interval_ms` is clamped to
/// 250..=60000 to match v1 behaviour.
pub fn start_pulse(interval_ms: u64) {
    if PULSE_RUNNING.swap(true, Ordering::SeqCst) {
        return; // already running
    }
    let interval = std::time::Duration::from_millis(interval_ms.clamp(250, 60_000));
    std::thread::spawn(move || loop {
        std::thread::sleep(interval);
        let last = last_ramp();
        if let Some(ramp) = last.as_ref() {
            let _ = write_ramp(ramp);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_dials_build_a_linear_ramp() {
        let ramp = build_ramp(ColorDials::default());
        for (i, v) in ramp[0].iter().enumerate() {
            assert_eq!(*v as usize, i * 257); // 0..=255 stretched onto 0..=65535
        }
        assert_eq!(ramp[0], ramp[1]);
        assert_eq!(ramp[1], ramp[2]);
    }

    #[test]
    fn ramps_never_decrease() {
        for (gamma, brightness, contrast) in [
            (0.3, -0.5, 0.5),
            (2.8, 0.5, 2.0),
            (1.4, 0.1, 1.3),
            (0.6, -0.2, 1.8),
        ] {
            let ramp = build_ramp(ColorDials {
                gamma,
                brightness,
                contrast,
            });
            assert!(ramp[0].windows(2).all(|w| w[0] <= w[1]));
        }
    }

    #[test]
    fn out_of_range_dials_are_clamped() {
        let wild = build_ramp(ColorDials {
            gamma: 99.0,
            brightness: 9.0,
            contrast: -4.0,
        });
        let edge = build_ramp(ColorDials {
            gamma: 2.8,
            brightness: 0.5,
            contrast: 0.5,
        });
        assert_eq!(wild, edge);
    }
}
