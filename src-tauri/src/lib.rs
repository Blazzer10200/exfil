//! EXFIL v2 — Tauri backend entry. Wires the NVAPI vibrance + GDI gamma core
//! and preset store into IPC commands consumed by the Svelte frontend.

mod amd;
mod gamma;
mod nvapi;
mod store;
mod vibrance;
mod watcher;

use gamma::ColorDials;
use store::{Preset, PresetStore, VibranceScale};
use vibrance::{Vibrance, VibranceInfo};
use std::sync::Mutex;
use tauri::{
    tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_updater::UpdaterExt;

/// Shared app state. The vibrance backend is optional — machines without an
/// NVIDIA or AMD driver still run (gamma works; vibrance returns a clean error).
struct AppState {
    vibrance: Option<Vibrance>,
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
    /// GPU vendor providing vibrance ("nvidia"/"amd"), or None = gamma only.
    vendor: Option<&'static str>,
    vibrance: Option<VibranceInfo>,
}

#[tauri::command]
fn get_status(state: State<AppState>) -> SystemStatus {
    let vibrance = state.vibrance.as_ref().and_then(|v| v.get().ok());
    SystemStatus {
        vendor: state.vibrance.as_ref().map(|v| v.vendor()),
        vibrance,
    }
}

/// The active vendor's vibrance scale, for stamping exports and rescaling
/// imports. Falls back to the measured legacy NVIDIA scale — also what's
/// assumed for old export files that predate the scale stamp.
fn vib_scale(state: &AppState) -> VibranceScale {
    state
        .vibrance
        .as_ref()
        .and_then(|v| v.get().ok())
        .map(|i| VibranceScale { min: i.min, max: i.max, default: i.default })
        .unwrap_or(store::LEGACY_SCALE)
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
    if let Some(v) = state.vibrance.as_ref() {
        v.set(vibrance)?;
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
        if let Some(v) = state.vibrance.as_ref() {
            v.reset_to_default()?;
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

/// Create a new user preset (seeded neutral) and return it. Frontend appends +
/// selects it. "Neutral" vibrance is the DRIVER's default level, not 0 — on
/// AMD's saturation scale 0 would seed a grayscale preset.
#[tauri::command]
fn create_preset(state: State<AppState>, name: String) -> Result<Preset, String> {
    let default_vib = state.vibrance.as_ref().map(|v| v.default_level()).unwrap_or(0);
    let mut s = lock(&state.store);
    let mut p = s.add(name);
    if default_vib != 0 {
        s.update(&p.slot, p.dials, default_vib);
        p.vibrance = default_vib;
    }
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
    let scale = vib_scale(&state);
    let json = lock(&state.store).export_json(scale)?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

/// Import presets from a JSON file (from the frontend open dialog), appending each
/// as a new user preset. Returns the fresh store so the frontend re-syncs the list.
#[tauri::command]
fn import_presets(state: State<AppState>, path: String) -> Result<PresetStore, String> {
    let json = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let scale = vib_scale(&state);
    let mut s = lock(&state.store);
    s.import_json(&json, scale)?;
    s.save()?;
    Ok(s.clone())
}

/// Whether EXFIL starts with Windows (stored preference; default on).
#[tauri::command]
fn get_autostart(state: State<AppState>) -> bool {
    lock(&state.store).autostart
}

/// Toggle start-with-Windows: flips the HKCU Run key and persists the choice
/// so boot-time setup re-asserts it. Returns the new value.
#[tauri::command]
fn set_autostart(
    app: tauri::AppHandle,
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

/// Global hotkeys — work while the window is hidden / a game is fullscreen:
/// Ctrl+Shift+F9 cycles presets, Ctrl+Shift+F10 snaps to Normal. Obscure
/// combos on purpose so they don't collide with in-game binds.
fn hotkey_cycle() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F9)
}

fn hotkey_normal() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F10)
}

fn register_hotkeys(app: &tauri::AppHandle) -> Result<(), String> {
    let gs = app.global_shortcut();
    gs.register(hotkey_cycle()).map_err(|e| e.to_string())?;
    gs.register(hotkey_normal()).map_err(|e| e.to_string())?;
    Ok(())
}

fn unregister_hotkeys(app: &tauri::AppHandle) {
    let gs = app.global_shortcut();
    let _ = gs.unregister(hotkey_cycle());
    let _ = gs.unregister(hotkey_normal());
}

/// The next slot in rail order after the active one (wraps; includes Normal so
/// cycling doubles as an on/off toggle). None when only Normal exists.
fn cycle_slot(state: &AppState) -> Option<String> {
    let s = lock(&state.store);
    if s.presets.len() < 2 {
        return None;
    }
    let idx = s.presets.iter().position(|p| p.slot == s.active).unwrap_or(0);
    Some(s.presets[(idx + 1) % s.presets.len()].slot.clone())
}

/// Apply a hotkey pick exactly like a hand-pick: it becomes the watcher's
/// revert target, and "auto-switch" re-syncs the (possibly hidden) UI.
fn hotkey_apply(app: &tauri::AppHandle, slot: String) {
    let state = app.state::<AppState>();
    match apply_slot(&state, &slot) {
        Ok(p) => {
            *lock(&state.manual_active) = slot;
            let _ = app.emit("auto-switch", &p);
        }
        Err(e) => log::warn!("hotkey apply failed: {e}"),
    }
}

/// Whether the global hotkeys are enabled (stored preference; default on).
#[tauri::command]
fn get_hotkeys(state: State<AppState>) -> bool {
    lock(&state.store).hotkeys
}

/// Toggle the global hotkeys: (un)registers them live and persists the choice
/// so boot-time setup honors it. Returns the new value.
#[tauri::command]
fn set_hotkeys(
    app: tauri::AppHandle,
    state: State<AppState>,
    enabled: bool,
) -> Result<bool, String> {
    if enabled {
        register_hotkeys(&app)?;
    } else {
        unregister_hotkeys(&app);
    }
    let mut s = lock(&state.store);
    s.hotkeys = enabled;
    s.save()?;
    Ok(enabled)
}

/// Reset display to neutral gamma + every monitor's native default vibrance
/// (shared by the tray "Reset" item and the exit-time restore).
fn do_reset(state: &AppState) {
    if let Err(e) = gamma::reset() {
        log::warn!("gamma reset failed: {e}");
    }
    if let Some(v) = state.vibrance.as_ref() {
        if let Err(e) = v.reset_to_default() {
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

/// Open an https URL in the user's default browser (settings-page links).
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
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
fn uninstall_app(app: tauri::AppHandle) -> Result<(), String> {
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

/// Update metadata surfaced to the frontend ("update-available" event + the
/// Settings "Check for updates" row).
#[derive(Clone, serde::Serialize)]
struct UpdateMeta {
    version: String,
    notes: String,
}

/// Check GitHub Releases for a newer signed build. Ok(None) = up to date.
#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<Option<UpdateMeta>, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| e.to_string())?;
    Ok(update.map(|u| UpdateMeta {
        version: u.version.clone(),
        notes: u.body.clone().unwrap_or_default(),
    }))
}

/// Download + install the pending update, streaming "update-progress" (0..=100)
/// so Settings can show download progress. The plugin verifies the artifact's
/// minisign signature against the pubkey in tauri.conf.json before installing.
/// On Windows the NSIS installer exits the app itself (RunEvent::Exit still
/// restores native color); restart() is the non-Windows fallback.
#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("No update available — you're already on the latest version.")?;
    let progress_app = app.clone();
    let mut downloaded: u64 = 0;
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                if let Some(total) = total {
                    let pct = ((downloaded as f64 / total as f64) * 100.0).min(100.0) as u8;
                    let _ = progress_app.emit("update-progress", pct);
                }
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    app.restart();
}

/// An action picked in the styled tray-menu popup. Hides the popup first so it
/// never lingers over the action's result.
#[tauri::command]
fn tray_action(
    app: tauri::AppHandle,
    state: State<AppState>,
    action: String,
) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("tray") {
        let _ = w.hide();
    }
    match action.as_str() {
        "show" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
            Ok(())
        }
        "reset" => {
            do_reset(&state);
            Ok(())
        }
        "quit" => {
            // NB: native-restore on quit is handled centrally in the
            // RunEvent::Exit handler, so every exit path leaves the screen native.
            app.exit(0);
            Ok(())
        }
        _ => Err("unknown tray action".into()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();

    let vibrance = Vibrance::load();
    match vibrance.as_ref() {
        Some(v) => log::info!("{} vibrance backend initialized", v.vendor()),
        None => log::warn!("no vibrance backend (NVIDIA/AMD driver not found) — gamma still works"),
    }

    let store = PresetStore::load();
    let state = AppState {
        vibrance,
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Hotkey presses arrive here from the OS — no window focus involved,
        // which is the whole point (switch presets mid-game).
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    let slot = if shortcut == &hotkey_cycle() {
                        cycle_slot(&app.state::<AppState>())
                    } else if shortcut == &hotkey_normal() {
                        Some("Normal".into())
                    } else {
                        None
                    };
                    if let Some(slot) = slot {
                        hotkey_apply(app, slot);
                    }
                })
                .build(),
        )
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
            get_autostart,
            set_autostart,
            get_hotkeys,
            set_hotkeys,
            tray_action,
            open_url,
            uninstall_app,
            check_update,
            install_update,
        ])
        .setup(|app| {
            // Start-with-Windows honors the stored preference (default on) —
            // re-asserted each boot, toggled from the titlebar menu.
            {
                let autostart = lock(&app.state::<AppState>().store).autostart;
                let al = app.autolaunch();
                let _ = if autostart { al.enable() } else { al.disable() };
            }

            // Global hotkeys honor the stored preference (default on). A failed
            // registration (combo taken by another app) is non-fatal — warn only.
            if lock(&app.state::<AppState>().store).hotkeys {
                if let Err(e) = register_hotkeys(app.handle()) {
                    log::warn!("global hotkey registration failed: {e}");
                }
            }

            // ── System tray: Show / Reset display / Quit ──
            // The menu is a styled webview popup (routes/tray), not the OS-native
            // tray menu — Windows draws that one itself and it can't be themed to
            // match the app. The popup window is frameless/transparent/always-on-
            // top, shown at the cursor on tray click, hidden on focus loss; its
            // items invoke `tray_action`.
            let tray_menu = WebviewWindowBuilder::new(app, "tray", WebviewUrl::App("tray".into()))
                .title("EXFIL menu")
                .inner_size(224.0, 200.0)
                .resizable(false)
                .maximizable(false)
                .minimizable(false)
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .visible(false)
                .focused(false)
                .build()?;
            {
                let w = tray_menu.clone();
                tray_menu.on_window_event(move |event| {
                    if let WindowEvent::Focused(false) = event {
                        let _ = w.hide();
                    }
                });
            }

            let tray_icon = app.default_window_icon().cloned();
            let mut tray_builder = TrayIconBuilder::new().tooltip("EXFIL");
            if let Some(icon) = tray_icon {
                tray_builder = tray_builder.icon(icon);
            } else {
                log::warn!("no default window icon set — tray icon will be blank");
            }
            tray_builder
                .on_tray_icon_event(|tray, event| {
                    // Any click (left or right) → styled menu popup just above
                    // the cursor. "tray-open" replays the entrance animation +
                    // refocuses; focus loss hides it again.
                    if let TrayIconEvent::Click {
                        button_state: MouseButtonState::Up,
                        position,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("tray") {
                            if let Ok(size) = w.outer_size() {
                                let x = (position.x - size.width as f64).max(8.0);
                                let y = (position.y - size.height as f64 - 8.0).max(8.0);
                                let _ = w.set_position(PhysicalPosition::new(x as i32, y as i32));
                            }
                            let _ = app.emit_to("tray", "tray-open", ());
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
                    if let Some(v) = state.vibrance.as_ref() {
                        if let Err(e) = v.set(p.vibrance) {
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

            // ── Update check on boot ──
            // Fire-and-forget against the GitHub Releases feed; a newer signed
            // build emits "update-available" so the UI can surface a toast.
            // Offline/unreachable is normal (debug-log only) — Settings keeps a
            // manual "Check for updates" for that case.
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    match handle.updater() {
                        Ok(updater) => match updater.check().await {
                            Ok(Some(update)) => {
                                let _ = handle.emit(
                                    "update-available",
                                    UpdateMeta {
                                        version: update.version.clone(),
                                        notes: update.body.clone().unwrap_or_default(),
                                    },
                                );
                            }
                            Ok(None) => {}
                            Err(e) => log::debug!("boot update check failed: {e}"),
                        },
                        Err(e) => log::debug!("updater unavailable: {e}"),
                    }
                });
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
