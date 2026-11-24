//! Foreground-program tracker. Polls which window is in front and reports the
//! exe that owns it, so the UI can show what's running in front (status bar,
//! Games view). Read-only: `GetForegroundWindow` plus the Toolhelp process
//! snapshot — no injection, no hooks, and no handle opened on the game.

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId,
};

use crate::watcher;

static FOREGROUND_RUNNING: AtomicBool = AtomicBool::new(false);

/// How long a resolved pid → exe pair is trusted. A just-launched process can
/// be missing from the first snapshot, so the lookup repeats on this interval
/// even when the foreground pid hasn't changed.
const EXE_CACHE_TTL: Duration = Duration::from_secs(3);

/// Start the tracker thread. `on_front(exe)` fires whenever the foreground
/// program changes: `Some(exe)` is its lowercased exe basename, `None` means
/// the desktop, the shell, EXFIL itself, or a window whose exe is unknown.
/// Idempotent.
pub fn start<F>(interval_ms: u64, on_front: F)
where
    F: Fn(Option<String>) + Send + 'static,
{
    if FOREGROUND_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    thread::spawn(move || {
        let own_pid = unsafe { GetCurrentProcessId() };
        let mut cache: Option<(u32, Option<String>, Instant)> = None;
        let mut announced: Option<Option<String>> = None;

        loop {
            let fg = unsafe { GetForegroundWindow() };
            let mut pid = 0u32;
            unsafe {
                GetWindowThreadProcessId(fg, Some(&mut pid));
            }
            let stale = match &cache {
                Some((p, _, at)) => *p != pid || at.elapsed() > EXE_CACHE_TTL,
                None => true,
            };
            if stale {
                let exe = if pid == 0 || pid == own_pid || is_shell(fg) {
                    None
                } else {
                    watcher::exe_for_pid(pid)
                };
                cache = Some((pid, exe, Instant::now()));
            }
            let front = cache.as_ref().and_then(|(_, exe, _)| exe.clone());
            if announced.as_ref() != Some(&front) {
                on_front(front.clone());
                announced = Some(front);
            }

            thread::sleep(Duration::from_millis(interval_ms));
        }
    });
}

/// Desktop / taskbar windows count as "nothing in front".
fn is_shell(hwnd: HWND) -> bool {
    let mut buf = [0u16; 64];
    let n = unsafe { GetClassNameW(hwnd, &mut buf) };
    let name = String::from_utf16_lossy(buf.get(..n.max(0) as usize).unwrap_or(&[]));
    matches!(
        name.as_str(),
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    )
}
