//! EXFIL v2 — Tauri backend entry. Wires the NVAPI vibrance + GDI gamma core
//! and preset store into IPC commands consumed by the Svelte frontend.

mod gamma;
mod nvapi;
mod store;
mod watcher;

use gamma::ColorDials;
use nvapi::{Nvapi, VibranceInfo};
use store::{Preset, PresetStore};
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, State, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

/// Shared app state. NVAPI handle is optional — non-NVIDIA machines still run
/// (gamma works; vibrance commands return a clean error).
struct AppState {
    nvapi: Option<Nvapi>,
    store: Mutex<PresetStore>,
    /// The slot the USER last picked by hand. The watcher reverts here when no
    /// bound program is running (revert-to-last-manual-pick).
    manual_active: Mutex<String>,
}

/// Lock a mutex, recovering the guard on poison instead of panicking. With
/// `panic = "abort"` in release, a bare `.lock().unwrap()` would abort the
/// whole process if any prior panic ever poisoned the lock; the store/string
/// data itself is still valid after a poison, so recovering is safe here.
fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
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
    lock(&state.store).clone()
}

/// Apply gamma dials + vibrance live (used during slider drag and on slot select).
#[tauri::command]
fn apply_color(
    state: State<AppState>,
    dials: ColorDials,
    vibrance: i32,
) -> Result<(), String> {
    apply_dials_and_vibrance(&state, dials, vibrance)
}

/// Push gamma dials + vibrance to the display. Shared by `apply_color` (live
/// slider drag) and `apply_slot` (preset select) so both paths stay identical.
fn apply_dials_and_vibrance(state: &AppState, dials: ColorDials, vibrance: i32) -> Result<(), String> {
    gamma::apply_dials(dials)?;
    if let Some(nv) = state.nvapi.as_ref() {
        nv.set_vibrance(vibrance)?;
    }
    Ok(())
}

/// Select a preset slot: apply it + mark active.
/// "Normal" restores each monitor's NATIVE color — neutral gamma + per-monitor
/// default vibrance — so the display picks up exactly what Windows/the driver
/// programmed it to. All other slots stamp their stored dials + vibrance.
#[tauri::command]
fn select_preset(state: State<AppState>, slot: String) -> Result<Preset, String> {
    let preset = apply_slot(&state, &slot)?;
    // A hand-pick becomes the revert target the watcher falls back to.
    *lock(&state.manual_active) = slot;
    Ok(preset)
}

/// Apply a slot's color to the display and mark it active in the store (persisted).
/// Shared by the manual `select_preset` command and the auto-switch watcher, so
/// both paths drive the exact same gamma/vibrance + active-state logic.
/// "Normal" restores each monitor's NATIVE color (neutral gamma + per-monitor
/// default vibrance); other slots stamp their stored dials + vibrance.
fn apply_slot(state: &AppState, slot: &str) -> Result<Preset, String> {
    let preset = {
        let mut s = lock(&state.store);
        let p = s.get(slot).cloned().ok_or("unknown slot")?;
        s.active = slot.to_string();
        p
    };
    if slot == "Normal" {
        gamma::reset()?;
        if let Some(nv) = state.nvapi.as_ref() {
            nv.reset_vibrance_to_default()?;
        }
    } else {
        apply_dials_and_vibrance(state, preset.dials, preset.vibrance)?;
    }
    if let Err(e) = lock(&state.store).save() {
        log::warn!("failed to persist preset store after apply: {e}");
    }
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
    let mut s = lock(&state.store);
    s.update(&slot, dials, vibrance);
    s.save()
}

/// Create a new user preset (seeded neutral) and return it. Frontend appends + selects it.
#[tauri::command]
fn create_preset(state: State<AppState>, name: String) -> Result<Preset, String> {
    let mut s = lock(&state.store);
    let p = s.add(name);
    s.save()?;
    Ok(p)
}

/// Delete a user preset; returns the fresh store so the frontend re-syncs list + active.
#[tauri::command]
fn delete_preset(state: State<AppState>, slot: String) -> Result<PresetStore, String> {
    let mut s = lock(&state.store);
    s.delete(&slot)?;
    s.save()?;
    Ok(s.clone())
}

/// Rename a user preset (display name only; Normal is read-only).
#[tauri::command]
fn rename_preset(state: State<AppState>, slot: String, name: String) -> Result<(), String> {
    let mut s = lock(&state.store);
    s.rename(&slot, name)?;
    s.save()
}

/// Bind a program (`exe`, e.g. "cs2.exe") to a slot, or clear with `exe = None`.
/// Returns the fresh store so the frontend re-syncs binding badges. When that
/// program runs, the watcher auto-applies this preset; on exit it reverts to the
/// last manual pick.
#[tauri::command]
fn set_binding(
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
/// "Has a visible window" disambiguates same-named exes via the window title and
/// needs no hardcoded denylist. Read-only enumeration — no injection.
#[tauri::command]
fn list_window_programs() -> Vec<watcher::WindowProc> {
    watcher::list_window_programs()
}

/// Export user presets (all but Normal) to a JSON file the user chose via the
/// frontend save dialog. Shareable / a backup; slot keys + bindings are dropped.
#[tauri::command]
fn export_presets(state: State<AppState>, path: String) -> Result<(), String> {
    let json = lock(&state.store).export_json()?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

/// Import presets from a JSON file (from the frontend open dialog), appending each
/// as a new user preset. Returns the fresh store so the frontend re-syncs the list.
#[tauri::command]
fn import_presets(state: State<AppState>, path: String) -> Result<PresetStore, String> {
    let json = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut s = lock(&state.store);
    s.import_json(&json)?;
    s.save()?;
    Ok(s.clone())
}

/// Reset display to neutral gamma + every monitor's native default vibrance
/// (shared by the tray "Reset" item and the exit-time restore).
fn do_reset(state: &AppState) {
    if let Err(e) = gamma::reset() {
        log::warn!("gamma reset failed: {e}");
    }
    if let Some(nv) = state.nvapi.as_ref() {
        if let Err(e) = nv.reset_vibrance_to_default() {
            log::warn!("vibrance reset failed: {e}");
        }
    }
}

/// Reset display to neutral gamma + every monitor's native default vibrance (panic button).
#[tauri::command]
fn reset_display(state: State<AppState>) -> Result<(), String> {
    do_reset(&state);
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

    let store = PresetStore::load();
    let state = AppState {
        nvapi,
        manual_active: Mutex::new(store.active.clone()),
        store: Mutex::new(store),
    };

    tauri::Builder::default()
        // Single-instance guard MUST be the first plugin. A second launch (e.g.
        // autostart + a manual double-click) routes here instead of spawning a
        // second pulse thread + watcher fighting over the same monitors — we just
        // surface the already-running window. `--hidden` is dropped so a manual
        // re-launch of a tray-hidden app pops it back open.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_presets,
            apply_color,
            select_preset,
            save_preset,
            create_preset,
            delete_preset,
            rename_preset,
            set_binding,
            list_window_programs,
            export_presets,
            import_presets,
            reset_display,
        ])
        .setup(|app| {
            // Run on Windows startup (hidden to tray). Idempotent — safe each boot.
            let _ = app.autolaunch().enable();

            // ── System tray: Show / Reset display / Quit ──
            let show_i = MenuItem::with_id(app, "show", "Show EXFIL", true, None::<&str>)?;
            let reset_i = MenuItem::with_id(app, "reset", "Reset display", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &reset_i, &quit_i])?;

            let tray_icon = app.default_window_icon().cloned();
            let mut tray_builder = TrayIconBuilder::new().tooltip("EXFIL");
            if let Some(icon) = tray_icon {
                tray_builder = tray_builder.icon(icon);
            } else {
                log::warn!("no default window icon set — tray icon will be blank");
            }
            tray_builder
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "reset" => do_reset(&app.state::<AppState>()),
                    "quit" => app.exit(0),
                    // NB: native-restore on quit is handled centrally in the
                    // RunEvent::Exit handler below, so EVERY exit path (tray
                    // Quit, OS shutdown, app.exit) leaves the screen at native.
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    // Left-click the tray icon → show + focus the window.
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;

            // ── Close-to-tray: the X (and titlebar close) hides instead of quitting ──
            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = w.hide();
                    }
                });

                // Start hidden when autostarted (--hidden); otherwise show normally.
                let hidden = std::env::args().any(|a| a == "--hidden");
                if hidden {
                    let _ = window.hide();
                } else {
                    let _ = window.show();
                }
            }

            // Apply the last-active preset on boot so the display matches UI.
            let state = app.state::<AppState>();
            let active = {
                let s = lock(&state.store);
                s.get(&s.active).cloned()
            };
            if let Some(p) = active {
                if lock(&state.store).active == "Normal" {
                    do_reset(&state);
                } else {
                    if let Err(e) = gamma::apply_dials(p.dials) {
                        log::warn!("boot-time gamma apply failed: {e}");
                    }
                    if let Some(nv) = state.nvapi.as_ref() {
                        if let Err(e) = nv.set_vibrance(p.vibrance) {
                            log::warn!("boot-time vibrance apply failed: {e}");
                        }
                    }
                }
            }
            // Re-assert the active ramp on an interval so fullscreen-exclusive
            // games can't permanently steal the gamma on focus.
            gamma::start_pulse(1000);

            // ── Per-program auto-switch ──
            // Poll running processes (~2s). When a bound program is the most-recent
            // running one, apply its preset; when the last bound program exits,
            // revert to the user's last manual pick. Read-only enumeration — no
            // injection. The frontend listens for "auto-switch" to re-sync the UI.
            {
                let handle = app.handle().clone();
                let bindings = {
                    let h = handle.clone();
                    move || lock(&h.state::<AppState>().store).bindings()
                };
                let on_change = {
                    let h = handle.clone();
                    move |winner: Option<String>| {
                        let state = h.state::<AppState>();
                        let slot = winner
                            .unwrap_or_else(|| lock(&state.manual_active).clone());
                        if let Ok(p) = apply_slot(&state, &slot) {
                            let _ = h.emit("auto-switch", &p);
                        }
                    }
                };
                watcher::start(2000, bindings, on_change);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building EXFIL")
        .run(|app, event| {
            // Restore native display on the way out so quitting (tray Quit, OS
            // shutdown, app.exit) never leaves a preset's gamma/vibrance stamped
            // on the monitors with no app left to clear it.
            if let tauri::RunEvent::Exit = event {
                do_reset(&app.state::<AppState>());
            }
        });
}
