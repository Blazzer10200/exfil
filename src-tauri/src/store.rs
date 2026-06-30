//! Preset persistence — JSON at %APPDATA%\exfil-v2\presets.json.
//! Model: a fixed read-only "Normal" native baseline plus user-created presets.
//! User presets get stable keys `p{n}` from a monotonic `next_id` (never reused),
//! so renames change only the display name and deletions never collide.

use crate::gamma::ColorDials;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Preset {
    pub slot: String,
    pub name: String,
    pub dials: ColorDials,
    pub vibrance: i32, // 0..=63 (NVAPI Ex scale)
    /// Bound program: a lowercased exe basename (e.g. "cs2.exe"). When that
    /// process is running, the watcher auto-applies this preset; None = unbound.
    #[serde(default)]
    pub exe: Option<String>,
}

fn default_next_id() -> u32 {
    1
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PresetStore {
    pub presets: Vec<Preset>,
    pub active: String,
    #[serde(default = "default_next_id")]
    pub next_id: u32,
}

fn normal_preset() -> Preset {
    Preset {
        slot: "Normal".into(),
        name: "Normal".into(),
        dials: ColorDials { gamma: 1.0, brightness: 0.0, contrast: 1.0 },
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
            dials: ColorDials { gamma: 1.0, brightness: 0.0, contrast: 1.0 },
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
}
