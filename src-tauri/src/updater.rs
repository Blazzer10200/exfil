//! Auto-update against GitHub Releases. The updater plugin verifies each
//! artifact's minisign signature against the pubkey in tauri.conf.json before
//! installing.

use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::{Update, UpdaterExt};

/// Update metadata surfaced to the frontend ("update-available" event + the
/// Settings "Check for updates" row).
#[derive(Clone, serde::Serialize)]
pub struct UpdateMeta {
    version: String,
    notes: String,
}

impl From<&Update> for UpdateMeta {
    fn from(u: &Update) -> Self {
        Self {
            version: u.version.clone(),
            notes: u.body.clone().unwrap_or_default(),
        }
    }
}

async fn fetch(app: &AppHandle) -> Result<Option<Update>, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    updater.check().await.map_err(|e| e.to_string())
}

/// Check GitHub Releases for a newer signed build. Ok(None) = up to date.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<UpdateMeta>, String> {
    Ok(fetch(&app).await?.as_ref().map(UpdateMeta::from))
}

/// Download + install the pending update, streaming "update-progress" (0..=100)
/// so Settings can show download progress. On Windows the NSIS installer exits
/// the app itself (RunEvent::Exit still restores native color); restart() is
/// the non-Windows fallback.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    let update = fetch(&app)
        .await?
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

/// Fire-and-forget check at startup; a newer signed build emits
/// "update-available" so the UI can surface a toast. Offline/unreachable is
/// normal (debug-log only) — Settings keeps a manual "Check for updates".
pub fn check_on_boot(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        match fetch(&app).await {
            Ok(Some(update)) => {
                let _ = app.emit("update-available", UpdateMeta::from(&update));
            }
            Ok(None) => {}
            Err(e) => log::debug!("boot update check failed: {e}"),
        }
    });
}
