//! Preset persistence — JSON at %APPDATA%\exfil-v2\presets.json.
//! Model: a fixed read-only "Normal" native baseline plus user-created presets.
//! User presets get stable keys `p{n}` from a monotonic `next_id` (never reused),
//! so renames change only the display name and deletions never collide.

use crate::gamma::ColorDials;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

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

/// Slot key of the fixed, read-only native baseline. Never stored with dials
/// that matter: selecting it restores each monitor's own native color.
pub const NORMAL_SLOT: &str = "Normal";

/// Legacy export files predate the scale stamp. They all came from NVIDIA
/// machines whose GetDVCInfoEx reports 0..=100 default 50 (measured live on
/// the dev machine — NOT the "0..63" folklore scale).
pub const LEGACY_SCALE: VibranceScale = VibranceScale {
    min: 0,
    max: 100,
    default: 50,
};

/// Map a level from one vendor scale onto another, preserving its position
/// relative to the scale's NEUTRAL point (default), not its absolute value.
/// Same-scale mapping is the identity.
///
/// The scales come from a driver or an import file, so the math runs in f64
/// (no i32 overflow on extreme values) and a nonsensical destination range
/// (min > max) maps everything to its default instead of panicking.
fn rescale_vibrance(v: i32, src: VibranceScale, dst: VibranceScale) -> i32 {
    if dst.min > dst.max {
        return dst.default;
    }
    let (v, s_min, s_def, s_max) = (v as f64, src.min as f64, src.default as f64, src.max as f64);
    let (d_min, d_def, d_max) = (dst.min as f64, dst.default as f64, dst.max as f64);
    let span = if v >= s_def {
        s_max - s_def
    } else {
        s_def - s_min
    };
    let t = if span <= 0.0 { 0.0 } else { (v - s_def) / span };
    let out = if t >= 0.0 {
        d_def + t * (d_max - d_def)
    } else {
        d_def + t * (d_def - d_min)
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
    /// Global hotkeys enabled (Ctrl+Shift+F9 cycle / Ctrl+Shift+F10 Normal).
    #[serde(default = "default_true")]
    pub hotkeys: bool,
    /// Ask the GPU driver to dither (NVIDIA only). Smooths the banding a preset
    /// causes on an 8-bit signal. Off by default: EXFIL leaves the driver's own
    /// dithering alone until the user turns this on.
    #[serde(default)]
    pub dither: bool,
}

fn normal_preset() -> Preset {
    Preset {
        slot: NORMAL_SLOT.into(),
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
            active: NORMAL_SLOT.into(),
            next_id: 1,
            autostart: true,
            hotkeys: true,
            dither: false,
        }
    }
}

fn store_path() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join("exfil-v2").join("presets.json")
}

/// Set when presets.json existed at startup but could be neither read nor
/// copied aside. While set, `save()` must not replace the file.
static UNSAVED_ORIGINAL: AtomicBool = AtomicBool::new(false);

/// Copy an unreadable presets.json to `presets.json.bad`. A file that is gone
/// counts as done: there is nothing left to protect.
fn backup_unreadable(path: &std::path::Path) -> std::io::Result<()> {
    match std::fs::copy(path, path.with_extension("json.bad")) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

impl PresetStore {
    /// Load presets.json. A missing file is the normal first run. A file that
    /// exists but can't be read or parsed is copied to `presets.json.bad`
    /// before defaults take over, because the next save would otherwise
    /// overwrite the only copy of the user's presets.
    pub fn load() -> Self {
        let path = store_path();
        // Another program (antivirus, a backup tool) can hold the file for a
        // moment at boot, so a failed read is retried before giving up.
        let mut read = std::fs::read_to_string(&path);
        for _ in 0..5 {
            match &read {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    read = std::fs::read_to_string(&path);
                }
                _ => break,
            }
        }
        let failure = match read {
            Ok(s) => match serde_json::from_str::<PresetStore>(&s) {
                Ok(store) => return store.normalized(),
                Err(e) => format!("parse failed ({e})"),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return PresetStore::default(),
            Err(e) => format!("read failed ({e})"),
        };
        match backup_unreadable(&path) {
            Ok(()) => log::warn!("presets.json {failure}; kept a copy as presets.json.bad"),
            Err(e) => {
                // No copy exists yet, so saving now would destroy the user's
                // presets. save() refuses until the copy can be made.
                UNSAVED_ORIGINAL.store(true, Ordering::Relaxed);
                log::warn!("presets.json {failure}; backup also failed ({e})");
            }
        }
        PresetStore::default()
    }

    /// Repair invariants a hand-edited or older file can break: the Normal
    /// baseline exists and comes first, `active` names a real slot, and
    /// `next_id` is past every existing `p{n}` key so a new preset can never
    /// collide with one already in the file.
    fn normalized(mut self) -> Self {
        if let Some(i) = self.presets.iter().position(|p| p.slot == NORMAL_SLOT) {
            if i != 0 {
                let normal = self.presets.remove(i);
                self.presets.insert(0, normal);
            }
        } else {
            self.presets.insert(0, normal_preset());
        }
        if self.get(&self.active).is_none() {
            self.active = NORMAL_SLOT.into();
        }
        let highest = self
            .presets
            .iter()
            .filter_map(|p| p.slot.strip_prefix('p')?.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        self.next_id = self.next_id.max(highest.saturating_add(1));
        self
    }

    pub fn save(&self) -> Result<(), String> {
        let path = store_path();
        if UNSAVED_ORIGINAL.load(Ordering::Relaxed) {
            backup_unreadable(&path).map_err(|e| {
                format!("presets.json could not be read at startup and is still locked ({e}); not overwriting it")
            })?;
            UNSAVED_ORIGINAL.store(false, Ordering::Relaxed);
        }
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        // Write-then-rename so a crash mid-write can never leave a truncated
        // presets.json behind (load() would silently fall back to defaults,
        // losing every preset).
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
    }

    pub fn get(&self, slot: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.slot == slot)
    }

    /// Overwrite a slot's color values.
    pub fn update(&mut self, slot: &str, dials: ColorDials, vibrance: i32) -> Result<(), String> {
        let p = self
            .presets
            .iter_mut()
            .find(|p| p.slot == slot)
            .ok_or("unknown slot")?;
        p.dials = dials;
        p.vibrance = vibrance;
        Ok(())
    }

    /// Create a new user preset with a fresh `p{next_id}` key and return a
    /// clone of it. A blank name becomes "Preset N".
    pub fn add(&mut self, name: String, dials: ColorDials, vibrance: i32) -> Preset {
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
            dials,
            vibrance,
            exe: None,
        };
        self.presets.push(preset.clone());
        preset
    }

    /// Delete a user preset. Normal is protected. If the deleted slot was active,
    /// active falls back to Normal.
    pub fn delete(&mut self, slot: &str) -> Result<(), String> {
        if slot == NORMAL_SLOT {
            return Err("Normal baseline cannot be deleted".into());
        }
        let before = self.presets.len();
        self.presets.retain(|p| p.slot != slot);
        if self.presets.len() == before {
            return Err("unknown slot".into());
        }
        if self.active == slot {
            self.active = NORMAL_SLOT.into();
        }
        Ok(())
    }

    /// Rename a user preset (display name only). Normal is protected.
    pub fn rename(&mut self, slot: &str, name: String) -> Result<(), String> {
        if slot == NORMAL_SLOT {
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
        if slot == NORMAL_SLOT {
            return Err("Normal baseline cannot be bound".into());
        }
        let exe = exe.and_then(|e| {
            let e = e.trim().to_lowercase();
            if e.is_empty() {
                None
            } else {
                Some(e)
            }
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
            presets: self
                .presets
                .iter()
                .filter(|p| p.slot != NORMAL_SLOT)
                .collect(),
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
        let invalid = |e: serde_json::Error| format!("invalid preset file: {e}");
        // Pick the shape first so a bad entry reports its own parse error
        // rather than the other shape's.
        let value: serde_json::Value = serde_json::from_str(json).map_err(invalid)?;
        let (src, incoming) = if value.is_array() {
            (
                LEGACY_SCALE,
                serde_json::from_value::<Vec<Preset>>(value).map_err(invalid)?,
            )
        } else {
            let f: ImportFile = serde_json::from_value(value).map_err(invalid)?;
            (f.vibrance_scale.unwrap_or(LEGACY_SCALE), f.presets)
        };
        let mut added = 0;
        for p in incoming {
            if p.slot == NORMAL_SLOT {
                continue;
            }
            self.add(p.name, p.dials, rescale_vibrance(p.vibrance, src, dst));
            added += 1;
        }
        Ok(added)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A neutral user preset, as the UI's "Blank" creates it.
    fn add(s: &mut PresetStore, name: &str) -> Preset {
        s.add(name.into(), ColorDials::default(), 0)
    }

    const NV: VibranceScale = VibranceScale {
        min: 0,
        max: 100,
        default: 50,
    };
    const AMD: VibranceScale = VibranceScale {
        min: 0,
        max: 200,
        default: 100,
    };

    #[test]
    fn rescale_same_scale_is_identity() {
        for v in [0, 25, 50, 75, 100] {
            assert_eq!(rescale_vibrance(v, NV, NV), v);
        }
    }

    #[test]
    fn rescale_preserves_position_relative_to_default() {
        assert_eq!(rescale_vibrance(50, NV, AMD), 100); // neutral → neutral
        assert_eq!(rescale_vibrance(100, NV, AMD), 200); // max → max
        assert_eq!(rescale_vibrance(0, NV, AMD), 0); // min → min
        assert_eq!(rescale_vibrance(75, NV, AMD), 150); // halfway above neutral
        assert_eq!(rescale_vibrance(150, AMD, NV), 75); // and back
    }

    #[test]
    fn rescale_clamps_out_of_range_input() {
        assert_eq!(rescale_vibrance(999, NV, AMD), 200);
        assert_eq!(rescale_vibrance(-5, NV, AMD), 0);
    }

    #[test]
    fn rescale_degenerate_span_maps_to_default() {
        let flat = VibranceScale {
            min: 50,
            max: 50,
            default: 50,
        };
        assert_eq!(rescale_vibrance(50, flat, AMD), 100);
    }

    #[test]
    fn add_mints_monotonic_keys_never_reused() {
        let mut s = PresetStore::default();
        let a = add(&mut s, "A");
        let b = add(&mut s, "B");
        assert_eq!(a.slot, "p1");
        assert_eq!(b.slot, "p2");
        s.delete(&b.slot).unwrap();
        assert_eq!(add(&mut s, "C").slot, "p3"); // deleted key never reused
    }

    #[test]
    fn add_blank_name_gets_placeholder() {
        let mut s = PresetStore::default();
        assert_eq!(add(&mut s, "   ").name, "Preset 1");
    }

    #[test]
    fn normal_is_protected() {
        let mut s = PresetStore::default();
        assert!(s.delete("Normal").is_err());
        assert!(s.rename("Normal", "X".into()).is_err());
        assert!(s.set_binding("Normal", Some("game.exe".into())).is_err());
    }

    #[test]
    fn delete_active_falls_back_to_normal() {
        let mut s = PresetStore::default();
        let p = add(&mut s, "A");
        s.active = p.slot.clone();
        s.delete(&p.slot).unwrap();
        assert_eq!(s.active, "Normal");
    }

    #[test]
    fn binding_an_exe_clears_it_off_other_slots() {
        let mut s = PresetStore::default();
        let a = add(&mut s, "A");
        let b = add(&mut s, "B");
        s.set_binding(&a.slot, Some("Game.EXE".into())).unwrap();
        s.set_binding(&b.slot, Some("game.exe".into())).unwrap();
        // lowercased, and the exe moved off slot A — one program, one preset
        assert_eq!(s.bindings(), vec![("game.exe".to_string(), b.slot.clone())]);
    }

    #[test]
    fn export_skips_normal_import_is_additive_and_rescales() {
        let mut src = PresetStore::default();
        let p = add(&mut src, "Comp");
        src.update(&p.slot, ColorDials::default(), 75).unwrap();
        let json = src.export_json(NV).unwrap();
        assert!(!json.contains("\"Normal\""));

        let mut dst = PresetStore::default();
        let existing = add(&mut dst, "Mine");
        let added = dst.import_json(&json, AMD).unwrap();
        assert_eq!(added, 1);
        assert_eq!(dst.presets.len(), 3); // Normal + Mine + imported
        let imported = dst.presets.last().unwrap();
        assert_ne!(imported.slot, existing.slot);
        assert_eq!(imported.vibrance, 150); // 75 @ NV scale → 150 @ AMD scale
        assert_eq!(imported.exe, None); // bindings are machine-local, dropped
    }

    #[test]
    fn import_accepts_legacy_bare_array_as_nvidia_scale() {
        let legacy = r#"[{"slot":"p9","name":"Old","dials":{"gamma":1.0,"brightness":0.0,"contrast":1.0},"vibrance":100}]"#;
        let mut s = PresetStore::default();
        assert_eq!(s.import_json(legacy, AMD).unwrap(), 1);
        let p = s.presets.last().unwrap();
        assert_eq!(p.slot, "p1"); // fresh key, not the file's p9
        assert_eq!(p.vibrance, 200); // legacy = NVIDIA scale, rescaled
    }

    #[test]
    fn import_skips_normal_entries_and_rejects_garbage() {
        let mut s = PresetStore::default();
        let file = r#"{"vibrance_scale":{"min":0,"max":100,"default":50},"presets":[{"slot":"Normal","name":"Normal","dials":{"gamma":1.0,"brightness":0.0,"contrast":1.0},"vibrance":50}]}"#;
        assert_eq!(s.import_json(file, NV).unwrap(), 0);
        assert!(s.import_json("not json", NV).is_err());
    }

    #[test]
    fn rescale_below_neutral_and_invalid_destination() {
        assert_eq!(rescale_vibrance(25, NV, AMD), 50); // halfway below neutral
        let inverted = VibranceScale {
            min: 200,
            max: 0,
            default: 100,
        };
        assert_eq!(rescale_vibrance(75, NV, inverted), 100); // no panic, default
        assert_eq!(rescale_vibrance(i32::MAX, NV, AMD), 200); // no overflow
        assert_eq!(rescale_vibrance(i32::MIN, NV, AMD), 0);
    }

    #[test]
    fn update_unknown_slot_errors() {
        let mut s = PresetStore::default();
        assert!(s.update("p404", ColorDials::default(), 10).is_err());
    }

    #[test]
    fn rename_trims_and_rejects_blank_or_unknown() {
        let mut s = PresetStore::default();
        let p = add(&mut s, "A");
        s.rename(&p.slot, "  Comp  ".into()).unwrap();
        assert_eq!(s.get(&p.slot).unwrap().name, "Comp");
        assert!(s.rename(&p.slot, "   ".into()).is_err());
        assert!(s.rename("p404", "X".into()).is_err());
    }

    #[test]
    fn delete_unknown_slot_errors() {
        let mut s = PresetStore::default();
        assert!(s.delete("p404").is_err());
    }

    #[test]
    fn binding_clears_with_none_or_blank_and_rejects_unknown() {
        let mut s = PresetStore::default();
        let p = add(&mut s, "A");
        s.set_binding(&p.slot, Some("game.exe".into())).unwrap();
        s.set_binding(&p.slot, None).unwrap();
        assert!(s.bindings().is_empty());
        s.set_binding(&p.slot, Some("game.exe".into())).unwrap();
        s.set_binding(&p.slot, Some("   ".into())).unwrap();
        assert!(s.bindings().is_empty());
        assert!(s.set_binding("p404", Some("game.exe".into())).is_err());
    }

    #[test]
    fn import_without_scale_stamp_uses_legacy_scale() {
        let file = r#"{"presets":[{"slot":"p1","name":"Old","dials":{"gamma":1.0,"brightness":0.0,"contrast":1.0},"vibrance":75}]}"#;
        let mut s = PresetStore::default();
        assert_eq!(s.import_json(file, AMD).unwrap(), 1);
        assert_eq!(s.presets.last().unwrap().vibrance, 150);
    }

    #[test]
    fn import_blank_name_gets_placeholder() {
        let file = r#"[{"slot":"p1","name":"  ","dials":{"gamma":1.0,"brightness":0.0,"contrast":1.0},"vibrance":50}]"#;
        let mut s = PresetStore::default();
        s.import_json(file, NV).unwrap();
        assert_eq!(s.presets.last().unwrap().name, "Preset 1");
    }

    // On-disk compatibility: a presets.json written before `next_id`,
    // `autostart`, `hotkeys` and `exe` existed must still load.
    #[test]
    fn minimal_old_file_loads_with_defaults_and_safe_next_id() {
        let file = r#"{"presets":[
            {"slot":"Normal","name":"Normal","dials":{"gamma":1.0,"brightness":0.0,"contrast":1.0},"vibrance":0},
            {"slot":"p4","name":"Comp","dials":{"gamma":1.2,"brightness":0.0,"contrast":1.0},"vibrance":80}
        ],"active":"p4"}"#;
        let mut s = serde_json::from_str::<PresetStore>(file)
            .unwrap()
            .normalized();
        assert!(s.autostart && s.hotkeys);
        assert!(!s.dither);
        assert_eq!(s.active, "p4");
        assert_eq!(s.get("p4").unwrap().exe, None);
        assert_eq!(add(&mut s, "New").slot, "p5"); // never collides with p4
    }

    #[test]
    fn normalized_restores_normal_and_a_valid_active() {
        let file = r#"{"presets":[
            {"slot":"p1","name":"A","dials":{"gamma":1.0,"brightness":0.0,"contrast":1.0},"vibrance":50}
        ],"active":"gone","next_id":2}"#;
        let s = serde_json::from_str::<PresetStore>(file)
            .unwrap()
            .normalized();
        assert_eq!(s.presets[0].slot, NORMAL_SLOT);
        assert_eq!(s.presets.len(), 2);
        assert_eq!(s.active, NORMAL_SLOT);
        assert_eq!(s.next_id, 2);
    }
}
