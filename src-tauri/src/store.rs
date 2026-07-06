//! Preset persistence — JSON at %APPDATA%\exfil-v2\presets.json.
//! Model: a fixed read-only "Normal" native baseline plus user-created presets.
//! User presets get stable keys `p{n}` from a monotonic `next_id` (never reused),
//! so renames change only the display name and deletions never collide.

use crate::gamma::ColorDials;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A vendor's vibrance level range, stamped into export files so imports on a
/// machine with a DIFFERENT GPU/driver can rescale values. Ranges are whatever
/// the driver reports at runtime (measured NVIDIA here: 0..=100 default 50;
/// AMD saturation typically 0..=200 default 100) — carrying raw numbers across
/// scales would badly shift saturation, e.g. grayscale on AMD.
#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct VibranceScale {
    pub min: i32,
    pub max: i32,
    pub default: i32,
}

/// Legacy export files predate the scale stamp. They all came from NVIDIA
/// machines whose GetDVCInfoEx reports 0..=100 default 50 (measured live on
/// the dev machine — NOT the "0..63" folklore scale).
pub const LEGACY_SCALE: VibranceScale = VibranceScale { min: 0, max: 100, default: 50 };

/// Map a level from one vendor scale onto another, preserving its position
/// relative to the scale's NEUTRAL point (default), not its absolute value.
/// Same-scale mapping is the identity.
fn rescale_vibrance(v: i32, src: VibranceScale, dst: VibranceScale) -> i32 {
    let t = if v >= src.default {
        let span = (src.max - src.default) as f64;
        if span <= 0.0 { 0.0 } else { (v - src.default) as f64 / span }
    } else {
        let span = (src.default - src.min) as f64;
        if span <= 0.0 { 0.0 } else { (v - src.default) as f64 / span }
    };
    let out = if t >= 0.0 {
        dst.default as f64 + t * (dst.max - dst.default) as f64
    } else {
        dst.default as f64 + t * (dst.default - dst.min) as f64
    };
    (out.round() as i32).clamp(dst.min, dst.max)
}

#[derive(Serialize)]
struct ExportFile<'a> {
    vibrance_scale: VibranceScale,
    presets: Vec<&'a Preset>,
}

#[derive(Deserialize)]
struct ImportFile {
    vibrance_scale: Option<VibranceScale>,
    presets: Vec<Preset>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Preset {
    pub slot: String,
    pub name: String,
    pub dials: ColorDials,
    /// Level in the machine's own driver-reported vibrance scale (measured
    /// NVIDIA here: 0..=100 default 50; AMD saturation typically 0..=200
    /// default 100). Applies are clamped into range; cross-vendor imports are
    /// rescaled via the VibranceScale stamped into export files.
    pub vibrance: i32,
    /// Bound program: a lowercased exe basename (e.g. "cs2.exe"). When that
    /// process is running, the watcher auto-applies this preset; None = unbound.
    #[serde(default)]
    pub exe: Option<String>,
}

fn default_next_id() -> u32 {
    1
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PresetStore {
    pub presets: Vec<Preset>,
    pub active: String,
    #[serde(default = "default_next_id")]
    pub next_id: u32,
    /// Launch EXFIL when Windows starts (drives the HKCU Run key on boot).
    #[serde(default = "default_true")]
    pub autostart: bool,
}

fn normal_preset() -> Preset {
    Preset {
        slot: "Normal".into(),
        name: "Normal".into(),
        dials: ColorDials::default(),
        vibrance: 0,
        exe: None,
    }
}

impl Default for PresetStore {
    fn default() -> Self {
        PresetStore {
            presets: vec![normal_preset()],
            active: "Normal".into(),
            next_id: 1,
            autostart: true,
        }
    }
}

fn store_path() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join("exfil-v2").join("presets.json")
}

impl PresetStore {
    pub fn load() -> Self {
        let path = store_path();
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
                log::warn!("presets.json parse failed ({e}); using defaults");
                PresetStore::default()
            }),
            Err(_) => PresetStore::default(),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = store_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())
    }

    pub fn get(&self, slot: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.slot == slot)
    }

    pub fn update(&mut self, slot: &str, dials: ColorDials, vibrance: i32) {
        if let Some(p) = self.presets.iter_mut().find(|p| p.slot == slot) {
            p.dials = dials;
            p.vibrance = vibrance;
        }
    }

    /// Create a new user preset seeded neutral. Returns a clone of the new preset.
    pub fn add(&mut self, name: String) -> Preset {
        let slot = format!("p{}", self.next_id);
        self.next_id += 1;
        let name = if name.trim().is_empty() {
            format!("Preset {}", self.presets.len())
        } else {
            name.trim().into()
        };
        let preset = Preset {
            slot,
            name,
            dials: ColorDials::default(),
            vibrance: 0,
            exe: None,
        };
        self.presets.push(preset.clone());
        preset
    }

    /// Delete a user preset. Normal is protected. If the deleted slot was active,
    /// active falls back to Normal.
    pub fn delete(&mut self, slot: &str) -> Result<(), String> {
        if slot == "Normal" {
            return Err("Normal baseline cannot be deleted".into());
        }
        let before = self.presets.len();
        self.presets.retain(|p| p.slot != slot);
        if self.presets.len() == before {
            return Err("unknown slot".into());
        }
        if self.active == slot {
            self.active = "Normal".into();
        }
        Ok(())
    }

    /// Rename a user preset (display name only). Normal is protected.
    pub fn rename(&mut self, slot: &str, name: String) -> Result<(), String> {
        if slot == "Normal" {
            return Err("Normal baseline cannot be renamed".into());
        }
        let name = name.trim();
        if name.is_empty() {
            return Err("name cannot be empty".into());
        }
        match self.presets.iter_mut().find(|p| p.slot == slot) {
            Some(p) => {
                p.name = name.into();
                Ok(())
            }
            None => Err("unknown slot".into()),
        }
    }

    /// Bind (or clear, with `None`) a program to a user preset. The exe is stored
    /// lowercased so process matching is case-insensitive. Normal is protected.
    /// Any other preset bound to the same exe is cleared so an exe maps to one slot.
    pub fn set_binding(&mut self, slot: &str, exe: Option<String>) -> Result<(), String> {
        if slot == "Normal" {
            return Err("Normal baseline cannot be bound".into());
        }
        let exe = exe.and_then(|e| {
            let e = e.trim().to_lowercase();
            if e.is_empty() { None } else { Some(e) }
        });
        if self.presets.iter().all(|p| p.slot != slot) {
            return Err("unknown slot".into());
        }
        if let Some(ref target) = exe {
            for p in self.presets.iter_mut() {
                if p.slot != slot && p.exe.as_deref() == Some(target.as_str()) {
                    p.exe = None;
                }
            }
        }
        if let Some(p) = self.presets.iter_mut().find(|p| p.slot == slot) {
            p.exe = exe;
        }
        Ok(())
    }

    /// All (exe, slot) bindings currently set, exe lowercased.
    pub fn bindings(&self) -> Vec<(String, String)> {
        self.presets
            .iter()
            .filter_map(|p| p.exe.clone().map(|e| (e, p.slot.clone())))
            .collect()
    }

    /// Serialize the user presets (everything except the fixed Normal baseline)
    /// for export/sharing, stamped with this machine's vibrance scale so a
    /// cross-vendor import can rescale. Slot keys + bindings are dropped since
    /// they're machine-local — only name/dials/vibrance carry over.
    pub fn export_json(&self, scale: VibranceScale) -> Result<String, String> {
        let file = ExportFile {
            vibrance_scale: scale,
            presets: self.presets.iter().filter(|p| p.slot != "Normal").collect(),
        };
        serde_json::to_string_pretty(&file).map_err(|e| e.to_string())
    }

    /// Import presets from an export file, appending each as a NEW user preset
    /// with a fresh `p{next_id}` slot (additive — never overwrites existing
    /// presets, never collides on keys). Imported bindings are cleared and
    /// vibrance is rescaled from the file's vendor scale onto `dst`. Accepts
    /// both the current `{vibrance_scale, presets}` shape and the legacy bare
    /// array (NVIDIA-era exports). Returns the number of presets added.
    pub fn import_json(&mut self, json: &str, dst: VibranceScale) -> Result<usize, String> {
        let (src, incoming) = match serde_json::from_str::<ImportFile>(json) {
            Ok(f) => (f.vibrance_scale.unwrap_or(LEGACY_SCALE), f.presets),
            Err(_) => {
                let legacy: Vec<Preset> = serde_json::from_str(json)
                    .map_err(|e| format!("invalid preset file: {e}"))?;
                (LEGACY_SCALE, legacy)
            }
        };
        let mut added = 0;
        for p in incoming {
            if p.slot == "Normal" {
                continue;
            }
            // add() mints a fresh slot key and pushes a neutral preset; stamp the
            // imported color onto it via last_mut() so keys never collide.
            self.add(p.name);
            if let Some(slot) = self.presets.last_mut() {
                slot.dials = p.dials;
                slot.vibrance = rescale_vibrance(p.vibrance, src, dst);
            }
            added += 1;
        }
        Ok(added)
    }
}
