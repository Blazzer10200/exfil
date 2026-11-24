//! System tray: icon + a styled menu popup (Show / Reset display / Quit).
//!
//! The menu is a webview window (routes/tray), not the OS-native tray menu —
//! Windows draws that one itself and it can't be themed to match the app. The
//! popup is frameless/transparent/always-on-top, shown at the cursor on tray
//! click and hidden on focus loss; its items invoke `tray_action`.

use tauri::{
    tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewUrl, WebviewWindowBuilder,
    WindowEvent,
};

use crate::state::AppState;
use crate::show_main_window;

const TRAY_WINDOW: &str = "tray";

/// Build the tray icon and (on a background thread) its menu popup.
pub fn setup(app: &App) -> tauri::Result<()> {
    // Built on its own thread: at boot WebView2 can be slow to start, and
    // building the popup inline made `setup` stall before the main window was
    // hidden (white box, app not responding). The tray click handler already
    // tolerates the window not existing yet.
    let handle = app.handle().clone();
    std::thread::spawn(move || build_menu_window(&handle));

    let mut builder = TrayIconBuilder::new().tooltip("EXFIL");
    match app.default_window_icon().cloned() {
        Some(icon) => builder = builder.icon(icon),
        None => log::warn!("no default window icon set — tray icon will be blank"),
    }
    builder
        .on_tray_icon_event(|tray, event| {
            // Any click (left or right) → menu popup just above the cursor.
            // "tray-open" replays the entrance animation + refreshes the
            // readout; focus loss hides it again.
            let TrayIconEvent::Click { button_state: MouseButtonState::Up, position, .. } = event
            else {
                return;
            };
            let app = tray.app_handle();
            let Some(w) = app.get_webview_window(TRAY_WINDOW) else { return };
            if let Ok(size) = w.outer_size() {
                let x = (position.x - size.width as f64).max(8.0);
                let y = (position.y - size.height as f64 - 8.0).max(8.0);
                let _ = w.set_position(PhysicalPosition::new(x as i32, y as i32));
            }
            let _ = app.emit_to(TRAY_WINDOW, "tray-open", ());
            let _ = w.show();
            let _ = w.set_focus();
        })
        .build(app)?;
    Ok(())
}

fn build_menu_window(app: &AppHandle) {
    let built = WebviewWindowBuilder::new(app, TRAY_WINDOW, WebviewUrl::App("tray".into()))
        .title("EXFIL menu")
        // Placeholder size — the page resizes the window to fit its menu.
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
        .build();
    match built {
        Ok(menu) => {
            let w = menu.clone();
            menu.on_window_event(move |event| {
                if let WindowEvent::Focused(false) = event {
                    let _ = w.hide();
                }
            });
        }
        Err(e) => log::warn!("tray menu window failed to build: {e}"),
    }
}

/// An action picked in the tray-menu popup. Hides the popup first so it never
/// lingers over the action's result.
#[tauri::command]
pub fn tray_action(app: AppHandle, state: State<AppState>, action: String) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(TRAY_WINDOW) {
        let _ = w.hide();
    }
    match action.as_str() {
        "show" => show_main_window(&app),
        "reset" => state.reset_display(),
        // Native-restore on quit is handled centrally in the RunEvent::Exit
        // handler, so every exit path leaves the screen native.
        "quit" => app.exit(0),
        _ => return Err("unknown tray action".into()),
    }
    Ok(())
}
