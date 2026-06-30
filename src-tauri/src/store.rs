//! Preset persistence — JSON at %APPDATA%\exfil-v2\presets.json.
//! 4 slots: Normal (native baseline, read-only), Preset 1, Preset 2, Preset 3.

use crate::gamma::ColorDials;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Preset {
    pub slot: String,
    pub name: String,
    pub dials: ColorDials,
    pub vibrance: i32, // 0..=63 (NVAPI Ex scale)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PresetStore {
    pub presets: Vec<Preset>,
    pub active: String,
}

impl Default for PresetStore {
    fn default() -> Self {
        let mk = |slot: &str, name: &str, g: f64, b: f64, c: f64, v: i32| Preset {
            slot: slot.into(),
            name: name.into(),
            dials: ColorDials { gamma: g, brightness: b, contrast: c },
            vibrance: v,
        };
        PresetStore {
            presets: vec![
                mk("Normal", "Normal", 1.0, 0.0, 1.0, 0),
                mk("Preset1", "Preset 1", 1.15, 0.05, 1.10, 28),
                mk("Preset2", "Preset 2", 0.90, -0.05, 1.05, 16),
                mk("Preset3", "Preset 3", 1.0, 0.0, 1.0, 20),
            ],
            active: "Normal".into(),
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
}
