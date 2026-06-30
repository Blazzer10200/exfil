//! EXFIL v2 — Tauri backend entry. Wires the NVAPI vibrance + GDI gamma core
//! and preset store into IPC commands consumed by the Svelte frontend.

mod gamma;
mod nvapi;
mod store;

use gamma::ColorDials;
use nvapi::{Nvapi, VibranceInfo};
use store::{Preset, PresetStore};
use std::sync::Mutex;
use tauri::{Manager, State};

/// Shared app state. NVAPI handle is optional — non-NVIDIA machines still run
/// (gamma works; vibrance commands return a clean error).
struct AppState {
    nvapi: Option<Nvapi>,
    store: Mutex<PresetStore>,
}

#[derive(serde::Serialize)]
struct SystemStatus {
    nvidia: bool,
    vibrance: Option<VibranceInfo>,
}

#[tauri::command]
fn get_status(state: State<AppState>) -> SystemStatus {
    let vibrance = state.nvapi.as_ref().and_then(|n| n.get_vibrance().ok());
    SystemStatus {
        nvidia: state.nvapi.is_some(),
        vibrance,
    }
}

#[tauri::command]
fn get_presets(state: State<AppState>) -> PresetStore {
    state.store.lock().unwrap().clone()
}

/// Apply gamma dials + vibrance live (used during slider drag and on slot select).
#[tauri::command]
fn apply_color(
    state: State<AppState>,
    dials: ColorDials,
    vibrance: i32,
) -> Result<(), String> {
    gamma::apply_dials(dials)?;
    if let Some(nv) = state.nvapi.as_ref() {
        nv.set_vibrance(vibrance)?;
    }
    Ok(())
}

/// Select a preset slot: apply it + mark active. Normal resets to neutral.
#[tauri::command]
fn select_preset(state: State<AppState>, slot: String) -> Result<Preset, String> {
    let preset = {
        let mut s = state.store.lock().unwrap();
        let p = s.get(&slot).cloned().ok_or("unknown slot")?;
        s.active = slot.clone();
        p
    };
    gamma::apply_dials(preset.dials)?;
    if let Some(nv) = state.nvapi.as_ref() {
        nv.set_vibrance(preset.vibrance)?;
    }
    let _ = state.store.lock().unwrap().save();
    Ok(preset)
}

/// Persist edits to a slot (Normal is read-only, ignored).
#[tauri::command]
fn save_preset(
    state: State<AppState>,
    slot: String,
    dials: ColorDials,
    vibrance: i32,
) -> Result<(), String> {
    if slot == "Normal" {
        return Err("Normal baseline is read-only".into());
    }
    let mut s = state.store.lock().unwrap();
    s.update(&slot, dials, vibrance);
    s.save()
}

/// Reset display to neutral gamma + default vibrance (panic button).
#[tauri::command]
fn reset_display(state: State<AppState>) -> Result<(), String> {
    gamma::reset()?;
    if let Some(nv) = state.nvapi.as_ref() {
        if let Ok(info) = nv.get_vibrance() {
            let _ = nv.set_vibrance(info.default);
        }
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    let nvapi = Nvapi::load();
    if nvapi.is_some() {
        log::info!("NVAPI initialized — digital vibrance available");
    } else {
        log::warn!("NVAPI unavailable — vibrance disabled, gamma still works");
    }

    let state = AppState {
        nvapi,
        store: Mutex::new(PresetStore::load()),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_presets,
            apply_color,
            select_preset,
            save_preset,
            reset_display,
        ])
        .setup(|app| {
            // Apply the last-active preset on boot so the display matches UI.
            let state = app.state::<AppState>();
            let active = {
                let s = state.store.lock().unwrap();
                s.get(&s.active).cloned()
            };
            if let Some(p) = active {
                let _ = gamma::apply_dials(p.dials);
                if let Some(nv) = state.nvapi.as_ref() {
                    let _ = nv.set_vibrance(p.vibrance);
                }
            }
            // Re-assert the active ramp on an interval so fullscreen-exclusive
            // games can't permanently steal the gamma on focus.
            gamma::start_pulse(1000);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running EXFIL");
}
