//! Vendor-neutral digital-vibrance layer over the NVIDIA (NVAPI) and AMD (ADL)
//! backends. Levels stay in whatever scale the DRIVER reports (measured NVIDIA
//! here: 0..=100 default 50; AMD saturation typically 0..=200 default 100) and
//! the frontend adapts via VibranceInfo min/max/default. Cross-vendor preset
//! imports are rescaled in store.rs using the scale stamped into export files.

use crate::amd::Adl;
use crate::nvapi::Nvapi;

#[derive(serde::Serialize, Clone, Copy, Debug)]
pub struct VibranceInfo {
    pub current: i32,
    pub min: i32,
    pub max: i32,
    pub default: i32,
}

/// Run `apply` on every display. Every display is attempted even if an earlier
/// one fails; the result is the last error, or an error when there are no
/// displays at all (`none` names the vendor in that message).
pub(crate) fn for_each_display<D>(
    displays: Vec<D>,
    none: &str,
    mut apply: impl FnMut(D) -> Result<(), String>,
) -> Result<(), String> {
    if displays.is_empty() {
        return Err(none.into());
    }
    let mut last_err = None;
    for d in displays {
        if let Err(e) = apply(d) {
            last_err = Some(e);
        }
    }
    last_err.map_or(Ok(()), Err)
}

/// Clamp a level into a driver-reported range. A driver that reports an
/// inverted range is an error, not a panic (`i32::clamp` would abort the app).
pub(crate) fn clamp_level(level: i32, info: &VibranceInfo) -> Result<i32, String> {
    if info.min > info.max {
        return Err(format!(
            "driver reported an invalid vibrance range {}..{}",
            info.min, info.max
        ));
    }
    Ok(level.clamp(info.min, info.max))
}

pub enum Vibrance {
    Nvidia(Nvapi),
    Amd(Adl),
}

impl Vibrance {
    /// Probe vendors in order (NVIDIA, then AMD). None = gamma-only machine
    /// (Intel iGPU etc.) — every caller degrades gracefully.
    pub fn load() -> Option<Self> {
        if let Some(nv) = Nvapi::load() {
            return Some(Vibrance::Nvidia(nv));
        }
        if let Some(adl) = Adl::load() {
            return Some(Vibrance::Amd(adl));
        }
        None
    }

    pub fn vendor(&self) -> &'static str {
        match self {
            Vibrance::Nvidia(_) => "nvidia",
            Vibrance::Amd(_) => "amd",
        }
    }

    /// Vibrance info for the primary output (level range + current + default).
    pub fn get(&self) -> Result<VibranceInfo, String> {
        match self {
            Vibrance::Nvidia(nv) => nv.get_vibrance(),
            Vibrance::Amd(adl) => adl.get_vibrance(),
        }
    }

    /// Set the level on EVERY connected output (clamped into the vendor range).
    pub fn set(&self, level: i32) -> Result<(), String> {
        match self {
            Vibrance::Nvidia(nv) => nv.set_vibrance(level),
            Vibrance::Amd(adl) => adl.set_vibrance(level),
        }
    }

    /// Restore every output to its OWN native default level ("Normal").
    pub fn reset_to_default(&self) -> Result<(), String> {
        match self {
            Vibrance::Nvidia(nv) => nv.reset_vibrance_to_default(),
            Vibrance::Amd(adl) => adl.reset_vibrance_to_default(),
        }
    }

    /// The driver's neutral level — what a fresh preset seeds with (seeding a
    /// literal 0 would mean MINIMUM saturation: washed-out on NVIDIA's
    /// 0..=100-default-50 scale, grayscale on AMD's). The hardcoded fallbacks
    /// are a dead-path guess: if get() fails here, set() will fail too.
    pub fn default_level(&self) -> i32 {
        if let Ok(info) = self.get() {
            return info.default;
        }
        match self {
            Vibrance::Nvidia(_) => 50,
            Vibrance::Amd(_) => 100,
        }
    }
}
