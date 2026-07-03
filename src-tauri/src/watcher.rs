//! Per-program auto-switch watcher. Polls the running-process set on an interval
//! (read-only `CreateToolhelp32Snapshot` — NO injection, NO hooks; just an
//! enumeration the OS already exposes) and, when a bound program starts or stops,
//! tells the app which preset slot should be active.
//!
//! Priority is MOST-RECENT-LAUNCH: among all bound programs currently running,
//! the one that started most recently wins. When none are running, the app
//! reverts to the user's last manual pick (resolved by the callback, not here).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
    TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};

static WATCHER_RUNNING: AtomicBool = AtomicBool::new(false);

/// Snapshot of currently-running process exe basenames, lowercased.
fn running_exes() -> Vec<String> {
    let mut out = Vec::new();
    unsafe {
        let snap = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return out,
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snap, &mut entry).is_ok() {
            loop {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..end]).to_lowercase();
                if !name.is_empty() {
                    out.push(name);
                }
                if Process32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _: windows::core::Result<()> = CloseHandle(snap);
    }
    out
}

/// A program the user can bind a preset to, surfaced in the picker.
/// `exe` is the lowercased basename the watcher matches on; `title` is the
/// friendly top-level window title shown to the user.
#[derive(serde::Serialize)]
pub struct WindowProc {
    pub exe: String,
    pub title: String,
}

/// List programs that own a visible top-level window, as (exe basename, window
/// title) pairs. This is the preferred picker source: "has a visible window" is
/// a self-maintaining heuristic for "real user-facing app", so it needs no
/// hardcoded denylist, and the title disambiguates same-named exes. Read-only —
/// `EnumWindows` + `QueryFullProcessImageNameW` (PROCESS_QUERY_LIMITED_INFORMATION),
/// no injection, no hooks. One entry per distinct exe (first window title wins).
pub fn list_window_programs() -> Vec<WindowProc> {
    let mut raw: Vec<(u32, String)> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(enum_window_cb),
            LPARAM(&mut raw as *mut Vec<(u32, String)> as isize),
        );
    }

    // pid -> title, then resolve each pid to its exe basename, dedup by exe.
    let mut seen_exe = std::collections::HashSet::new();
    let mut out: Vec<WindowProc> = Vec::new();
    for (pid, title) in raw {
        let exe = match exe_for_pid(pid) {
            Some(e) if !is_noise(&e) => e,
            _ => continue,
        };
        if seen_exe.insert(exe.clone()) {
            out.push(WindowProc { exe, title });
        }
    }
    out.sort_by_key(|a| a.title.to_lowercase());
    out
}

/// EnumWindows callback: collect (pid, title) for visible, titled top-level windows.
extern "system" fn enum_window_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return BOOL(1);
        }
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return BOOL(1);
        }
        let mut buf = vec![0u16; len as usize + 1];
        let got = GetWindowTextW(hwnd, &mut buf);
        if got <= 0 {
            return BOOL(1);
        }
        let title = String::from_utf16_lossy(&buf[..got as usize]);
        if title.trim().is_empty() {
            return BOOL(1);
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid != 0 {
            let out = &mut *(lparam.0 as *mut Vec<(u32, String)>);
            out.push((pid, title));
        }
    }
    BOOL(1)
}

/// Resolve a pid to its lowercased exe basename via the full image path.
fn exe_for_pid(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = vec![0u16; 260];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut size,
        );
        let _: windows::core::Result<()> = CloseHandle(handle);
        ok.ok()?;
        let path = String::from_utf16_lossy(&buf[..size as usize]);
        path.rsplit(['\\', '/']).next().map(|s| s.to_lowercase())
    }
}

/// Filter obvious OS/background clutter from the user-facing process picker.
fn is_noise(exe: &str) -> bool {
    const NOISE: &[&str] = &[
        "svchost.exe",
        "runtimebroker.exe",
        "dllhost.exe",
        "conhost.exe",
        "csrss.exe",
        "wininit.exe",
        "services.exe",
        "smss.exe",
        "lsass.exe",
        "fontdrvhost.exe",
        "registry",
        "system",
        "system idle process",
        "memory compression",
        "taskhostw.exe",
        "sihost.exe",
        "ctfmon.exe",
        "searchhost.exe",
        "textinputhost.exe",
        "backgroundtaskhost.exe",
        "applicationframehost.exe",
        "wmiprvse.exe",
        "spoolsv.exe",
        "audiodg.exe",
    ];
    NOISE.contains(&exe)
}

/// Start the watcher thread. `bindings()` returns the live (exe→slot) map each
/// tick (read from the store). `on_change(Some(slot))` fires when a bound program
/// becomes the most-recent-running winner; `on_change(None)` fires when the last
/// bound program exits (caller reverts to its manual pick). Idempotent.
pub fn start<B, C>(interval_ms: u64, bindings: B, on_change: C)
where
    B: Fn() -> Vec<(String, String)> + Send + 'static,
    C: Fn(Option<String>) + Send + 'static,
{
    if WATCHER_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    thread::spawn(move || {
        // launch_seq orders bound exes by first-seen-running, so "most recent
        // launch" = highest seq among the currently-running bound set.
        let mut seq: u64 = 0;
        let mut launch_seq: HashMap<String, u64> = HashMap::new();
        let mut last_winner: Option<String> = None;
        let mut announced_any = false;

        loop {
            thread::sleep(Duration::from_millis(interval_ms));

            let binds = bindings();
            // exe -> slot, only for bound exes
            let bind_map: HashMap<String, String> = binds.into_iter().collect();

            // Nothing bound → skip the full process snapshot entirely. Enumerating
            // every process on the machine every tick is pure waste when there's
            // no exe to match; for a background tray utility with no bindings (the
            // common case) this keeps the watcher thread effectively idle. If a
            // bound winner was active when the last binding was cleared, revert to
            // the manual pick once. (`!announced_any` preserves the original
            // first-tick announce that establishes the baseline on boot.)
            if bind_map.is_empty() {
                launch_seq.clear();
                if last_winner.is_some() || !announced_any {
                    announced_any = true;
                    last_winner = None;
                    on_change(None);
                }
                continue;
            }

            let running: std::collections::HashSet<String> =
                running_exes().into_iter().collect();

            // Update launch ordering: assign a seq the first tick we see a bound
            // exe running; drop the seq once it stops so a relaunch re-orders it.
            for exe in bind_map.keys() {
                let is_up = running.contains(exe);
                if is_up && !launch_seq.contains_key(exe) {
                    seq += 1;
                    launch_seq.insert(exe.clone(), seq);
                } else if !is_up {
                    launch_seq.remove(exe);
                }
            }

            // Winner = bound+running exe with the largest launch seq.
            let winner_slot = launch_seq
                .iter()
                .filter(|(exe, _)| bind_map.contains_key(*exe))
                .max_by_key(|(_, s)| **s)
                .and_then(|(exe, _)| bind_map.get(exe).cloned());

            if winner_slot != last_winner || !announced_any {
                announced_any = true;
                last_winner = winner_slot.clone();
                on_change(winner_slot);
            }
        }
    });
}
