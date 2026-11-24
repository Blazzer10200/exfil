//! EXFIL v2 — Tauri backend entry. Wires the vibrance (NVAPI / ADL) + GDI gamma
//! core and the preset store into IPC commands consumed by the Svelte frontend.
//!
//! Module map:
//! - `state`      shared `AppState` + the display operations everything uses
//! - `commands`   IPC commands (presets, bindings, import/export, system)
//! - `hotkeys`    global shortcuts
//! - `tray`       tray icon + styled menu popup
//! - `updater`    signed auto-update via GitHub Releases
//! - `store`      preset model + persistence
//! - `gamma`      GDI gamma ramps
//! - `vibrance`   vendor-neutral digital vibrance (`nvapi` / `amd` backends)
//! - `watcher`    per-program auto-switch
//! - `foreground` foreground-program tracker
//! - `process`    read-only process / window enumeration behind both

mod amd;
mod commands;
mod foreground;
mod gamma;
mod hotkeys;
mod nvapi;
mod process;
mod state;
mod store;
mod tray;
mod updater;
mod vibrance;
mod watcher;

use state::{lock, AppState};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

const MAIN_WINDOW: &str = "main";
/// Passed by the autostart entry so a boot-time launch stays in the tray.
const HIDDEN_ARG: &str = "--hidden";
/// How often the active gamma ramp is re-asserted.
const GAMMA_PULSE_MS: u64 = 1000;
/// How often the auto-switch watcher polls the process list.
const WATCHER_INTERVAL_MS: u64 = 2000;
/// How often the foreground program is sampled.
const FOREGROUND_INTERVAL_MS: u64 = 250;

/// Bring the main window to the front (tray "Show", second launch).
fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    tauri::Builder::default()
        // Single-instance guard MUST be the first plugin. A second launch (e.g.
        // autostart + a manual double-click) routes here instead of spawning a
        // second pulse thread + watcher fighting over the same monitors — we just
        // surface the already-running window, even if it was started hidden.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            show_main_window(app)
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![HIDDEN_ARG]),
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(hotkeys::plugin())
        .manage(AppState::load())
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_presets,
            commands::apply_color,
            commands::select_preset,
            commands::save_preset,
            commands::create_preset,
            commands::delete_preset,
            commands::rename_preset,
            commands::set_binding,
            commands::list_window_programs,
            commands::export_presets,
            commands::import_presets,
            commands::reset_display,
            commands::get_autostart,
            commands::set_autostart,
            commands::get_in_front,
            commands::primary_monitor,
            commands::open_url,
            commands::uninstall_app,
            hotkeys::set_hotkeys,
            tray::tray_action,
            updater::check_update,
            updater::install_update,
        ])
        .setup(|app| {
            let state = app.state::<AppState>();
            let (autostart, hotkeys_on) = {
                let s = lock(&state.store);
                (s.autostart, s.hotkeys)
            };

            // Start-with-Windows honors the stored preference (default on),
            // re-asserted each boot.
            let al = app.autolaunch();
            let _ = if autostart { al.enable() } else { al.disable() };

            // A failed registration (combo taken by another app) is non-fatal.
            if hotkeys_on {
                if let Err(e) = hotkeys::register(app.handle()) {
                    log::warn!("global hotkey registration failed: {e}");
                }
            }

            tray::setup(app)?;

            // Close-to-tray: the X (and titlebar close) hides instead of quitting.
            if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = w.hide();
                    }
                });
                // The window is created hidden (tauri.conf.json); an autostart
                // launch leaves it that way.
                if !std::env::args().any(|a| a == HIDDEN_ARG) {
                    let _ = window.show();
                }
            }

            state.apply_active_on_boot();
            // Re-assert the active ramp on an interval so fullscreen-exclusive
            // games can't permanently steal the gamma on focus.
            gamma::start_pulse(GAMMA_PULSE_MS);

            // Per-program auto-switch. When a bound program is the most-recent
            // running one, apply its preset; when the last bound program exits,
            // revert to the user's last manual pick. The frontend listens for
            // "auto-switch" to re-sync the UI.
            {
                let bindings = {
                    let h = app.handle().clone();
                    move || lock(&h.state::<AppState>().store).bindings()
                };
                let on_change = {
                    let h = app.handle().clone();
                    move |winner: Option<String>| {
                        let state = h.state::<AppState>();
                        let slot = winner.unwrap_or_else(|| lock(&state.manual_active).clone());
                        if let Ok(p) = state.apply_slot(&slot) {
                            let _ = h.emit("auto-switch", &p);
                        }
                    }
                };
                watcher::start(WATCHER_INTERVAL_MS, bindings, on_change);
            }

            // Foreground program → "in-front", so the status bar + Games view
            // can show what's running in front.
            {
                let h = app.handle().clone();
                foreground::start(FOREGROUND_INTERVAL_MS, move |exe| {
                    *lock(&h.state::<AppState>().in_front) = exe.clone();
                    let _ = h.emit("in-front", exe);
                });
            }

            updater::check_on_boot(app.handle().clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building EXFIL")
        .run(|app, event| {
            // Restore native display on the way out so quitting (tray Quit, OS
            // shutdown, app.exit) never leaves a preset's gamma/vibrance stamped
            // on the monitors with no app left to clear it.
            if let tauri::RunEvent::Exit = event {
                app.state::<AppState>().reset_display();
            }
        });
}
