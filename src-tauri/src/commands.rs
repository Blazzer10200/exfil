//! IPC commands for presets, bindings, import/export and system actions.
//! Hotkey, tray and updater commands live in their own modules.

use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

use crate::gamma::ColorDials;
use crate::process;
use crate::state::{lock, AppState};
use crate::store::{Preset, PresetStore, NORMAL_SLOT};
use crate::vibrance::VibranceInfo;

#[derive(serde::Serialize)]
pub struct SystemStatus {
    /// GPU vendor providing vibrance ("nvidia"/"amd"), or None = gamma only.
    vendor: Option<&'static str>,
    vibrance: Option<VibranceInfo>,
    /// The GPU driver's dithering can be switched from EXFIL (NVIDIA only).
    dither_supported: bool,
    /// Bits per channel the outputs carry (8 when the driver can't say).
    output_bits: u8,
}

#[tauri::command]
pub fn get_status(state: State<AppState>) -> SystemStatus {
    SystemStatus {
        vendor: state.vibrance.as_ref().map(|v| v.vendor()),
        vibrance: state.vibrance.as_ref().and_then(|v| v.get().ok()),
        dither_supported: state.dither_supported(),
        output_bits: state.vibrance.as_ref().map_or(8, |v| v.output_bits()),
    }
}

/// How many of the 256 shades these dials keep on a signal carrying `bits`
/// per channel (256 = none lost). Pure math, nothing is applied.
#[tauri::command]
pub fn count_shades(dials: ColorDials, bits: u8) -> u16 {
    crate::gamma::distinct_shades(dials, bits)
}

/// Toggle driver dithering: applies it live and persists the choice so boot
/// honors it. Returns the new value.
#[tauri::command]
pub fn set_dither(state: State<AppState>, enabled: bool) -> Result<bool, String> {
    state.set_dither(enabled)?;
    let mut s = lock(&state.store);
    s.dither = enabled;
    s.save()?;
    Ok(enabled)
}

#[tauri::command]
pub fn get_presets(state: State<AppState>) -> PresetStore {
    lock(&state.store).clone()
}

/// Apply gamma dials + vibrance live (used during slider drag).
#[tauri::command]
pub fn apply_color(state: State<AppState>, dials: ColorDials, vibrance: i32) -> Result<(), String> {
    state.apply_color(dials, vibrance)
}

/// Select a preset slot by hand: apply it, mark it active, and make it the
/// revert target the watcher falls back to.
#[tauri::command]
pub fn select_preset(state: State<AppState>, slot: String) -> Result<Preset, String> {
    state.pick_slot(&slot)
}

/// Persist edits to a slot (Normal is read-only).
#[tauri::command]
pub fn save_preset(
    state: State<AppState>,
    slot: String,
    dials: ColorDials,
    vibrance: i32,
) -> Result<(), String> {
    if slot == NORMAL_SLOT {
        return Err("Normal baseline is read-only".into());
    }
    let mut s = lock(&state.store);
    s.update(&slot, dials, vibrance)?;
    s.save()
}

/// Create a new user preset (seeded neutral) and return it. Frontend appends +
/// selects it. "Neutral" vibrance is the DRIVER's default level, not 0 — on
/// AMD's saturation scale 0 would seed a grayscale preset.
#[tauri::command]
pub fn create_preset(state: State<AppState>, name: String) -> Result<Preset, String> {
    let neutral = state
        .vibrance
        .as_ref()
        .map(|v| v.default_level())
        .unwrap_or(0);
    let mut s = lock(&state.store);
    let p = s.add(name, ColorDials::default(), neutral);
    s.save()?;
    Ok(p)
}

/// Delete a user preset; returns the fresh store so the frontend re-syncs list + active.
#[tauri::command]
pub fn delete_preset(state: State<AppState>, slot: String) -> Result<PresetStore, String> {
    let mut s = lock(&state.store);
    s.delete(&slot)?;
    s.save()?;
    Ok(s.clone())
}

/// Rename a user preset (display name only; Normal is read-only).
#[tauri::command]
pub fn rename_preset(state: State<AppState>, slot: String, name: String) -> Result<(), String> {
    let mut s = lock(&state.store);
    s.rename(&slot, name)?;
    s.save()
}

/// Bind a program (`exe`, e.g. "cs2.exe") to a slot, or clear with `exe = None`.
/// Returns the fresh store so the frontend re-syncs binding badges. When that
/// program runs, the watcher auto-applies this preset; on exit it reverts to the
/// last manual pick.
#[tauri::command]
pub fn set_binding(
    state: State<AppState>,
    slot: String,
    exe: Option<String>,
) -> Result<PresetStore, String> {
    let mut s = lock(&state.store);
    s.set_binding(&slot, exe)?;
    s.save()?;
    Ok(s.clone())
}

/// List programs that own a visible window, as {exe, title} pairs, for the binder.
/// The window title disambiguates same-named exes. Read-only enumeration — no
/// injection.
#[tauri::command]
pub fn list_window_programs() -> Vec<process::WindowProc> {
    process::list_window_programs()
}

/// Export user presets (all but Normal) to a JSON file the user chose via the
/// frontend save dialog. Shareable / a backup; slot keys + bindings are dropped.
#[tauri::command]
pub fn export_presets(state: State<AppState>, path: String) -> Result<(), String> {
    let json = lock(&state.store).export_json(state.vibrance_scale())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

/// Import presets from a JSON file (from the frontend open dialog), appending each
/// as a new user preset. Returns the fresh store so the frontend re-syncs the list.
#[tauri::command]
pub fn import_presets(state: State<AppState>, path: String) -> Result<PresetStore, String> {
    let json = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let scale = state.vibrance_scale();
    let mut s = lock(&state.store);
    s.import_json(&json, scale)?;
    s.save()?;
    Ok(s.clone())
}

/// Whether EXFIL starts with Windows (stored preference; default on).
#[tauri::command]
pub fn get_autostart(state: State<AppState>) -> bool {
    lock(&state.store).autostart
}

/// Toggle start-with-Windows: flips the HKCU Run key and persists the choice
/// so boot-time setup re-asserts it. Returns the new value.
#[tauri::command]
pub fn set_autostart(
    app: AppHandle,
    state: State<AppState>,
    enabled: bool,
) -> Result<bool, String> {
    let al = app.autolaunch();
    let res = if enabled { al.enable() } else { al.disable() };
    res.map_err(|e| e.to_string())?;
    let mut s = lock(&state.store);
    s.autostart = enabled;
    s.save()?;
    Ok(enabled)
}

/// Reset display to neutral gamma + every monitor's native default vibrance
/// (panic button).
#[tauri::command]
pub fn reset_display(state: State<AppState>) {
    state.reset_display();
}

/// Foreground program right now (same value the "in-front" event carries).
#[tauri::command]
pub fn get_in_front(state: State<AppState>) -> Option<String> {
    lock(&state.in_front).clone()
}

/// Primary monitor as the preview labels it: device name + physical size.
#[derive(serde::Serialize)]
pub struct MonitorInfo {
    name: String,
    width: u32,
    height: u32,
}

#[tauri::command]
pub fn primary_monitor(app: AppHandle) -> Option<MonitorInfo> {
    let m = app.primary_monitor().ok().flatten()?;
    let raw = m.name().cloned().unwrap_or_default();
    // "\\.\DISPLAY1" → "DISPLAY1"
    let name = raw.rsplit('\\').next().unwrap_or("DISPLAY1").to_string();
    let size = m.size();
    Some(MonitorInfo {
        name,
        width: size.width,
        height: size.height,
    })
}

/// Open an https URL in the user's default browser (settings-page links).
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("only https URLs can be opened".into());
    }
    std::process::Command::new("explorer")
        .arg(&url)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Launch the NSIS uninstaller that the setup wizard drops next to the exe,
/// then quit so it can remove the files (RunEvent::Exit restores native color
/// on the way out). Portable/dev copies have no uninstaller — clean error.
#[tauri::command]
pub fn uninstall_app(app: AppHandle) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let uninstaller = exe
        .parent()
        .map(|dir| dir.join("uninstall.exe"))
        .filter(|p| p.exists())
        .ok_or("No uninstaller found — this copy of EXFIL wasn't installed with the setup wizard. Just delete the .exe to remove it.")?;
    std::process::Command::new(&uninstaller)
        .spawn()
        .map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}
