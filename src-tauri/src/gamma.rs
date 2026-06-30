//! Windows GDI gamma ramp control via Get/SetDeviceGammaRamp (gdi32).
//!
//! A gamma ramp is 3×256 u16 entries (R,G,B). We derive a ramp from three
//! perceptual dials — gamma (mid-tone curve), brightness (offset), contrast
//! (pivot around 0.5) — matching the EXFIL v1 ColorEngine math.
//!
//! NO injection — SetDeviceGammaRamp is a standard display-driver call.

#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{CreateDCW, DeleteDC};
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

#[cfg(windows)]
fn display_dc() -> windows::Win32::Graphics::Gdi::HDC {
    // "DISPLAY" device context covers the primary adapter.
    let name: Vec<u16> = "DISPLAY\0".encode_utf16().collect();
    unsafe {
        CreateDCW(
            PCWSTR(name.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            None,
        )
    }
}

/// Read the current gamma ramp from the display.
#[cfg(windows)]
pub fn get_ramp() -> Result<GammaRamp, String> {
    let dc = display_dc();
    if dc.is_invalid() {
        return Err("CreateDCW(DISPLAY) failed".into());
    }
    let mut ramp: GammaRamp = [[0u16; 256]; 3];
    let ok = unsafe { GetDeviceGammaRamp(dc, ramp.as_mut_ptr() as *mut _) };
    unsafe { let _ = DeleteDC(dc); };
    if ok.as_bool() {
        Ok(ramp)
    } else {
        Err("GetDeviceGammaRamp failed".into())
    }
}

/// Apply a gamma ramp to the display.
#[cfg(windows)]
pub fn set_ramp(ramp: &GammaRamp) -> Result<(), String> {
    let dc = display_dc();
    if dc.is_invalid() {
        return Err("CreateDCW(DISPLAY) failed".into());
    }
    let ok = unsafe { SetDeviceGammaRamp(dc, ramp.as_ptr() as *const _) };
    unsafe { let _ = DeleteDC(dc); };
    if ok.as_bool() {
        Ok(())
    } else {
        Err("SetDeviceGammaRamp failed (driver may clamp extreme ramps)".into())
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

// ── non-Windows stubs so the crate still type-checks elsewhere ──
#[cfg(not(windows))]
pub fn get_ramp() -> Result<GammaRamp, String> { Err("gamma: Windows only".into()) }
#[cfg(not(windows))]
pub fn set_ramp(_r: &GammaRamp) -> Result<(), String> { Err("gamma: Windows only".into()) }
#[cfg(not(windows))]
pub fn apply_dials(_d: ColorDials) -> Result<(), String> { Err("gamma: Windows only".into()) }
#[cfg(not(windows))]
pub fn reset() -> Result<(), String> { Err("gamma: Windows only".into()) }
