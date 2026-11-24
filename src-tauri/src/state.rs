//! Shared app state plus the display operations every entry point funnels
//! through (IPC commands, hotkeys, the auto-switch watcher, tray, exit).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};

use crate::gamma::{self, ColorDials};
use crate::store::{self, Preset, PresetStore, VibranceScale, NORMAL_SLOT};
use crate::vibrance::Vibrance;

/// The vibrance backend is optional — machines without an NVIDIA or AMD driver
/// still run (gamma works; vibrance returns a clean error).
pub struct AppState {
    pub vibrance: Option<Vibrance>,
    pub store: Mutex<PresetStore>,
    /// The slot the USER last picked by hand. The watcher reverts here when no
    /// bound program is running (revert-to-last-manual-pick).
    pub manual_active: Mutex<String>,
    /// Exe basename of the foreground window (None = desktop / shell / EXFIL),
    /// kept current by the foreground thread and mirrored to the UI as "in-front".
    pub in_front: Mutex<Option<String>>,
    /// True while EXFIL has the driver's dithering switched on, so exit only
    /// hands back what EXFIL itself changed.
    dither_on: AtomicBool,
}

/// Lock a mutex, recovering the guard on poison instead of panicking. With
/// `panic = "abort"` in release, a bare `.lock().unwrap()` would abort the
/// whole process if any prior panic ever poisoned the lock; the store/string
/// data itself is still valid after a poison, so recovering is safe here.
pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl AppState {
    /// Load the vibrance backend and the persisted preset store.
    pub fn load() -> Self {
        let vibrance = Vibrance::load();
        match vibrance.as_ref() {
            Some(v) => log::info!("{} vibrance backend initialized", v.vendor()),
            None => {
                log::warn!("no vibrance backend (NVIDIA/AMD driver not found) — gamma still works")
            }
        }
        let store = PresetStore::load();
        Self {
            vibrance,
            manual_active: Mutex::new(store.active.clone()),
            store: Mutex::new(store),
            in_front: Mutex::new(None),
            dither_on: AtomicBool::new(false),
        }
    }

    /// The active vendor's vibrance scale, for stamping exports and rescaling
    /// imports. Falls back to the measured legacy NVIDIA scale — also what's
    /// assumed for old export files that predate the scale stamp.
    pub fn vibrance_scale(&self) -> VibranceScale {
        self.vibrance
            .as_ref()
            .and_then(|v| v.get().ok())
            .map(|i| VibranceScale {
                min: i.min,
                max: i.max,
                default: i.default,
            })
            .unwrap_or(store::LEGACY_SCALE)
    }

    /// Push gamma dials + vibrance to the display. Shared by the live slider
    /// drag and preset select so both paths stay identical.
    pub fn apply_color(&self, dials: ColorDials, vibrance: i32) -> Result<(), String> {
        gamma::apply_dials(dials)?;
        if let Some(v) = self.vibrance.as_ref() {
            v.set(vibrance)?;
        }
        Ok(())
    }

    /// Apply a slot's color to the display and mark it active in the store
    /// (persisted). Shared by the manual select, the hotkeys and the
    /// auto-switch watcher, so every path drives the exact same logic.
    /// "Normal" restores each monitor's NATIVE color (neutral gamma +
    /// per-monitor default vibrance); other slots stamp their stored dials +
    /// vibrance.
    pub fn apply_slot(&self, slot: &str) -> Result<Preset, String> {
        let preset = {
            let mut s = lock(&self.store);
            let p = s.get(slot).cloned().ok_or("unknown slot")?;
            s.active = slot.to_string();
            p
        };
        if slot == NORMAL_SLOT {
            gamma::reset()?;
            if let Some(v) = self.vibrance.as_ref() {
                v.reset_to_default()?;
            }
        } else {
            self.apply_color(preset.dials, preset.vibrance)?;
        }
        if let Err(e) = lock(&self.store).save() {
            log::warn!("failed to persist preset store after apply: {e}");
        }
        Ok(preset)
    }

    /// Apply a slot exactly like a hand-pick: it also becomes the revert
    /// target the watcher falls back to.
    pub fn pick_slot(&self, slot: &str) -> Result<Preset, String> {
        let preset = self.apply_slot(slot)?;
        *lock(&self.manual_active) = slot.to_string();
        Ok(preset)
    }

    /// Re-apply the last-active preset at startup so the display matches the
    /// UI. Failures are logged, never fatal.
    pub fn apply_active_on_boot(&self) {
        let active = {
            let s = lock(&self.store);
            s.get(&s.active).cloned()
        };
        let Some(p) = active else { return };
        if p.slot == NORMAL_SLOT {
            self.reset_display();
            return;
        }
        if let Err(e) = gamma::apply_dials(p.dials) {
            log::warn!("boot-time gamma apply failed: {e}");
        }
        if let Some(v) = self.vibrance.as_ref() {
            if let Err(e) = v.set(p.vibrance) {
                log::warn!("boot-time vibrance apply failed: {e}");
            }
        }
    }

    /// Whether the GPU driver's dithering can be switched from here.
    pub fn dither_supported(&self) -> bool {
        self.vibrance.as_ref().is_some_and(|v| v.supports_dither())
    }

    /// Switch the driver's dithering on, or hand it back to the driver default.
    pub fn set_dither(&self, on: bool) -> Result<(), String> {
        let v = self
            .vibrance
            .as_ref()
            .ok_or("dithering control needs an NVIDIA GPU")?;
        v.set_dither(on)?;
        self.dither_on.store(on, Ordering::Relaxed);
        Ok(())
    }

    /// Turn dithering on at startup when the stored setting asks for it. With
    /// the setting off the driver is never touched.
    pub fn apply_dither_on_boot(&self) {
        if !lock(&self.store).dither {
            return;
        }
        if let Err(e) = self.set_dither(true) {
            log::warn!("boot-time dithering enable failed: {e}");
        }
    }

    /// Exit-time restore: hand dithering back to the driver, but only if EXFIL
    /// was the one that switched it on.
    pub fn release_dither(&self) {
        if !self.dither_on.load(Ordering::Relaxed) {
            return;
        }
        if let Err(e) = self.set_dither(false) {
            log::warn!("dithering reset failed: {e}");
        }
    }

    /// Reset to neutral gamma + every monitor's native default vibrance.
    /// Shared by the panic button, the tray "Reset" item and the exit-time
    /// restore; failures are logged so one backend can't block the other.
    pub fn reset_display(&self) {
        if let Err(e) = gamma::reset() {
            log::warn!("gamma reset failed: {e}");
        }
        if let Some(v) = self.vibrance.as_ref() {
            if let Err(e) = v.reset_to_default() {
                log::warn!("vibrance reset failed: {e}");
            }
        }
    }
}
