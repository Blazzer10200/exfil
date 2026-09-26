//! Crosshair overlay: a native, click-through, always-on-top layered window
//! drawn with per-pixel alpha (`UpdateLayeredWindow`). It never touches the
//! game — no injection, no hooks, no process handle. It only asks the OS which
//! window is in front, where that window's client area is, and (via the
//! read-only Toolhelp snapshot) which exe owns it.
//!
//! Visible over borderless/windowed games; exclusive-fullscreen games own the
//! display and draw over every overlay (a Windows limit, not ours).

use std::ffi::c_void;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC,
    GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, ReleaseDC, SelectObject,
    AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION,
    DIB_RGB_COLORS, MONITORINFO, MONITOR_DEFAULTTONULL, MONITOR_DEFAULTTOPRIMARY,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::HiDpi::{
    SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClassNameW, GetClientRect,
    GetForegroundWindow, GetWindowThreadProcessId, IsIconic, PeekMessageW, RegisterClassW,
    SetWindowPos, ShowWindow, TranslateMessage, UpdateLayeredWindow, HWND_TOPMOST, MSG,
    PM_REMOVE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA,
    WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::crosshair::{render, Bitmap, CrosshairStyle};
use crate::watcher;

static OVERLAY_RUNNING: AtomicBool = AtomicBool::new(false);

extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

/// Start the overlay thread. `resolve(fg_exe)` returns the style to draw for
/// the current foreground program (None = hide); `fg_exe` is None when the
/// foreground window is EXFIL itself or its exe is unknown. Idempotent.
pub fn start<F>(resolve: F)
where
    F: Fn(Option<&str>) -> Option<CrosshairStyle> + Send + 'static,
{
    if OVERLAY_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    thread::spawn(move || {
        // Physical pixels for every coordinate this thread reads or writes,
        // even when the target game is DPI-unaware (no scaling virtualization).
        unsafe {
            let _ = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        }
        match unsafe { create_window() } {
            Ok(hwnd) => run(hwnd, resolve),
            Err(e) => {
                log::warn!("crosshair overlay unavailable: {e}");
                OVERLAY_RUNNING.store(false, Ordering::SeqCst);
            }
        }
    });
}

unsafe fn create_window() -> windows::core::Result<HWND> {
    let instance = GetModuleHandleW(None)?;
    let class = w!("EXFIL_CrosshairOverlay");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(wndproc),
        hInstance: instance.into(),
        lpszClassName: class,
        ..Default::default()
    };
    // A 0 return (already registered / failure) surfaces via CreateWindowExW.
    RegisterClassW(&wc);
    // LAYERED + TRANSPARENT = per-pixel alpha, clicks fall through to the game.
    // TOOLWINDOW + NOACTIVATE = no taskbar/alt-tab entry, never steals focus.
    CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
        class,
        w!("EXFIL crosshair"),
        WS_POPUP,
        0,
        0,
        1,
        1,
        None,
        None,
        instance,
        None,
    )
}

fn run<F>(hwnd: HWND, resolve: F)
where
    F: Fn(Option<&str>) -> Option<CrosshairStyle>,
{
    let own_pid = unsafe { GetCurrentProcessId() };
    // Foreground pid → exe, refreshed when the pid changes (and every few
    // seconds, in case a just-launched process wasn't in the first snapshot).
    let mut exe_cache: Option<(u32, Option<String>, Instant)> = None;
    let mut bitmap: Option<(CrosshairStyle, Bitmap)> = None;
    let mut drawn: Option<(CrosshairStyle, i32, i32)> = None;
    let mut visible = false;
    let mut topmost_at = Instant::now();

    loop {
        pump();

        let fg = unsafe { GetForegroundWindow() };
        let mut pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(fg, Some(&mut pid));
        }
        let own = pid == own_pid;
        let stale = match &exe_cache {
            Some((p, _, at)) => *p != pid || at.elapsed() > Duration::from_secs(3),
            None => true,
        };
        if stale {
            let exe = if pid == 0 || own { None } else { watcher::exe_for_pid(pid) };
            exe_cache = Some((pid, exe, Instant::now()));
        }
        let exe = exe_cache.as_ref().and_then(|(_, e, _)| e.as_deref());

        let target = resolve(if own { None } else { exe })
            .and_then(|st| anchor(fg, own).map(|c| (st, c)));

        match target {
            None => {
                if visible {
                    unsafe {
                        let _ = ShowWindow(hwnd, SW_HIDE);
                    }
                    visible = false;
                    drawn = None;
                }
            }
            Some((st, (cx, cy))) => {
                if !matches!(&bitmap, Some((s, _)) if *s == st) {
                    bitmap = Some((st.clone(), render(&st)));
                }
                if let Some((_, bmp)) = bitmap.as_ref() {
                    let clean = st.sanitized();
                    let x = cx - bmp.half as i32 + clean.offset_x;
                    let y = cy - bmp.half as i32 + clean.offset_y;
                    let key = (st, x, y);
                    if drawn.as_ref() != Some(&key) {
                        if let Err(e) = unsafe { push(hwnd, bmp, x, y) } {
                            log::warn!("crosshair draw failed: {e}");
                        }
                        drawn = Some(key);
                    }
                    // Games and other overlays can claim TOPMOST too — re-assert
                    // on show and once a second while visible.
                    if !visible || topmost_at.elapsed() > Duration::from_secs(1) {
                        unsafe {
                            if !visible {
                                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                            }
                            let _ = SetWindowPos(
                                hwnd,
                                HWND_TOPMOST,
                                0,
                                0,
                                0,
                                0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                            );
                        }
                        visible = true;
                        topmost_at = Instant::now();
                    }
                }
            }
        }

        // ~30 Hz while shown (tracks window moves / alt-tab), slower when idle.
        thread::sleep(Duration::from_millis(if visible { 33 } else { 120 }));
    }
}

fn pump() {
    let mut msg = MSG::default();
    unsafe {
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Desktop / taskbar windows: their "client area" isn't a play area, so they
/// fall back to the monitor center like EXFIL's own windows do.
fn is_shell(hwnd: HWND) -> bool {
    let mut buf = [0u16; 64];
    let n = unsafe { GetClassNameW(hwnd, &mut buf) };
    let name = String::from_utf16_lossy(buf.get(..n.max(0) as usize).unwrap_or(&[]));
    matches!(
        name.as_str(),
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
    )
}

/// Screen-space center boundary to anchor on. The overlay lives on the PRIMARY
/// monitor only: a foreground window on it anchors to its client-area center
/// (windowed/borderless games); anything else — a window on another monitor,
/// EXFIL itself, the desktop, a minimized window — anchors to the primary
/// monitor's center, so the crosshair never jumps to a second screen.
fn anchor(fg: HWND, own: bool) -> Option<(i32, i32)> {
    unsafe {
        // The primary monitor always contains the virtual-screen origin.
        let primary = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        if !fg.is_invalid()
            && !own
            && !IsIconic(fg).as_bool()
            && !is_shell(fg)
            && MonitorFromWindow(fg, MONITOR_DEFAULTTONULL) == primary
        {
            let mut rc = RECT::default();
            if GetClientRect(fg, &mut rc).is_ok() {
                let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
                let mut origin = POINT { x: 0, y: 0 };
                if w > 0 && h > 0 && ClientToScreen(fg, &mut origin).as_bool() {
                    return Some((origin.x + w / 2, origin.y + h / 2));
                }
            }
        }
        let mut mi = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(primary, &mut mi).as_bool() {
            let r = mi.rcMonitor;
            return Some((r.left + (r.right - r.left) / 2, r.top + (r.bottom - r.top) / 2));
        }
        None
    }
}

/// Upload a bitmap to the layered window at screen (x, y). Converts straight
/// RGBA to the premultiplied BGRA that `UpdateLayeredWindow` requires.
unsafe fn push(hwnd: HWND, bmp: &Bitmap, x: i32, y: i32) -> Result<(), String> {
    let n = bmp.size as i32;
    let screen = GetDC(None);
    let mem = CreateCompatibleDC(screen);
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: n,
            biHeight: -n, // top-down rows, same order as the renderer
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut c_void = std::ptr::null_mut();
    let result = match CreateDIBSection(mem, &bmi, DIB_RGB_COLORS, &mut bits, None, 0) {
        Ok(dib) if !bits.is_null() => {
            let dst = std::slice::from_raw_parts_mut(bits as *mut u8, bmp.rgba.len());
            for (d, s) in dst.as_chunks_mut::<4>().0.iter_mut().zip(bmp.rgba.as_chunks::<4>().0) {
                let a = s[3] as u32;
                d[0] = ((s[2] as u32 * a + 127) / 255) as u8;
                d[1] = ((s[1] as u32 * a + 127) / 255) as u8;
                d[2] = ((s[0] as u32 * a + 127) / 255) as u8;
                d[3] = s[3];
            }
            let old = SelectObject(mem, dib);
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let dst_pt = POINT { x, y };
            let src_pt = POINT { x: 0, y: 0 };
            let sz = SIZE { cx: n, cy: n };
            let r = UpdateLayeredWindow(
                hwnd,
                screen,
                Some(&dst_pt as *const POINT),
                Some(&sz as *const SIZE),
                mem,
                Some(&src_pt as *const POINT),
                COLORREF(0),
                Some(&blend as *const BLENDFUNCTION),
                ULW_ALPHA,
            )
            .map_err(|e| e.to_string());
            let _ = SelectObject(mem, old);
            let _ = DeleteObject(dib);
            r
        }
        Ok(dib) => {
            let _ = DeleteObject(dib);
            Err("CreateDIBSection returned no pixel buffer".into())
        }
        Err(e) => Err(e.to_string()),
    };
    let _ = DeleteDC(mem);
    let _ = ReleaseDC(None, screen);
    result
}
