//! Global hotkeys — work while the window is hidden / a game is fullscreen:
//! Ctrl+Shift+F9 cycles presets, Ctrl+Shift+F10 snaps to Normal. Obscure
//! combos on purpose so they don't collide with in-game binds.

use tauri::{AppHandle, Emitter, Manager, Runtime, State};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::state::{lock, AppState};
use crate::store::NORMAL_SLOT;

fn cycle_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F9)
}

fn normal_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::F10)
}

/// The plugin that delivers hotkey presses. They arrive from the OS with no
/// window focus involved, which is the whole point (switch presets mid-game).
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            let state = app.state::<AppState>();
            let slot = if shortcut == &cycle_shortcut() {
                next_slot(&state)
            } else if shortcut == &normal_shortcut() {
                Some(NORMAL_SLOT.to_string())
            } else {
                None
            };
            let Some(slot) = slot else { return };
            // Same as a hand-pick; "auto-switch" re-syncs the (possibly hidden) UI.
            match state.pick_slot(&slot) {
                Ok(p) => {
                    let _ = app.emit("auto-switch", &p);
                }
                Err(e) => log::warn!("hotkey apply failed: {e}"),
            }
        })
        .build()
}

pub fn register<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let gs = app.global_shortcut();
    gs.register(cycle_shortcut()).map_err(|e| e.to_string())?;
    gs.register(normal_shortcut()).map_err(|e| e.to_string())?;
    Ok(())
}

fn unregister<R: Runtime>(app: &AppHandle<R>) {
    let gs = app.global_shortcut();
    let _ = gs.unregister(cycle_shortcut());
    let _ = gs.unregister(normal_shortcut());
}

/// The slot after the active one in list order (wraps; includes Normal so
/// cycling doubles as an on/off toggle). None when only Normal exists.
fn next_slot(state: &AppState) -> Option<String> {
    let s = lock(&state.store);
    if s.presets.len() < 2 {
        return None;
    }
    let idx = s
        .presets
        .iter()
        .position(|p| p.slot == s.active)
        .unwrap_or(0);
    Some(s.presets[(idx + 1) % s.presets.len()].slot.clone())
}

/// Toggle the global hotkeys: (un)registers them live and persists the choice
/// so boot-time setup honors it. Returns the new value.
#[tauri::command]
pub fn set_hotkeys(app: AppHandle, state: State<AppState>, enabled: bool) -> Result<bool, String> {
    if enabled {
        register(&app)?;
    } else {
        unregister(&app);
    }
    let mut s = lock(&state.store);
    s.hotkeys = enabled;
    s.save()?;
    Ok(enabled)
}
