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
