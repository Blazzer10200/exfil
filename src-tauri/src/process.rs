//! Read-only process and window enumeration: which programs are running, which
//! exe owns a pid, and which programs have a visible window (the binder's
//! picker). Everything here only reads what the OS already exposes — NO
//! injection, NO hooks, no memory access.

use std::collections::HashSet;

use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};

/// Lowercased exe basenames of every running process.
pub fn running_exes() -> HashSet<String> {
    process_list().into_iter().map(|(_, name)| name).collect()
}

/// Exe basename (lowercased) of one pid, via the same read-only snapshot —
/// deliberately NOT OpenProcess, so the foreground tracker never opens a handle
/// on an anti-cheat-protected game while it polls the foreground window.
pub fn exe_for_pid(pid: u32) -> Option<String> {
    process_list()
        .into_iter()
        .find(|(p, _)| *p == pid)
        .map(|(_, name)| name)
}

/// Snapshot of running processes as (pid, lowercased exe basename).
fn process_list() -> Vec<(u32, String)> {
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
                    out.push((entry.th32ProcessID, name));
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
/// title) pairs. This is the picker source: "has a visible window" is a
/// self-maintaining heuristic for "real user-facing app" (a short denylist,
/// `is_noise`, drops the shell apps that slip through), and the title
/// disambiguates same-named exes. One entry per distinct exe (first window
/// title wins).
///
/// Read-only — no injection, no hooks. Unlike the background polls, this runs
/// only while the user has the picker open, and it does open each windowed
/// process with `PROCESS_QUERY_LIMITED_INFORMATION` (the access Task Manager
/// uses) to read its image path for the game-first ranking.
pub fn list_window_programs() -> Vec<WindowProc> {
    let mut raw: Vec<(u32, String)> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(enum_window_cb),
            LPARAM(&mut raw as *mut Vec<(u32, String)> as isize),
        );
    }

    // pid -> title, then resolve each pid to its exe basename, dedup by exe.
    // EXFIL itself is never a valid binding target (it would apply always),
    // and likely games sort ahead of everything else so the thing the user
    // came here to bind is at the top of the picker.
    let self_exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    let mut seen_exe = HashSet::new();
    let mut scored: Vec<(bool, WindowProc)> = Vec::new();
    for (pid, title) in raw {
        let path = match path_for_pid(pid) {
            Some(p) => p,
            None => continue,
        };
        let exe = match path.rsplit(['\\', '/']).next() {
            Some(b) if !b.is_empty() => b.to_string(),
            _ => continue,
        };
        if is_noise(&exe) || self_exe.as_deref() == Some(exe.as_str()) {
            continue;
        }
        if seen_exe.insert(exe.clone()) {
            scored.push((is_probably_game(&path), WindowProc { exe, title }));
        }
    }
    scored.sort_by_key(|(game, wp)| (!*game, wp.title.to_lowercase()));
    scored.into_iter().map(|(_, wp)| wp).collect()
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

/// Resolve a pid to its full lowercased image path.
fn path_for_pid(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        // Comfortably over MAX_PATH — deep storefront library paths would make
        // QueryFullProcessImageNameW fail on a 260-char buffer and silently
        // drop that game from the binder picker.
        let mut buf = vec![0u16; 1024];
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
        Some(path.to_lowercase())
    }
}

/// Heuristic "this is probably a game": installed under a storefront library
/// dir, or carrying an engine-style exe name (UE's *-Win64-Shipping.exe etc.).
/// Only used to rank the picker — never to filter, so a miss just sorts lower.
fn is_probably_game(path: &str) -> bool {
    const DIR_HINTS: &[&str] = &[
        "steamapps",
        "epic games",
        "gog galaxy",
        "gog games",
        "riot games",
        "battle.net",
        "origin games",
        "ea games",
        "xboxgames",
    ];
    const NAME_HINTS: &[&str] = &["shipping", "-win64-", "-wingdk-"];
    DIR_HINTS.iter().any(|h| path.contains(h)) || NAME_HINTS.iter().any(|h| path.contains(h))
}

/// Filter obvious OS/background clutter from the user-facing process picker.
/// Only processes whose image path could be read get this far.
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
        // Shell/system apps that own visible windows but are never games.
        "explorer.exe",
        "systemsettings.exe",
        "shellexperiencehost.exe",
        "startmenuexperiencehost.exe",
        "searchapp.exe",
        "lockapp.exe",
        "widgets.exe",
        "msedgewebview2.exe",
    ];
    NOISE.contains(&exe)
}
