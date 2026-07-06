//! NVAPI digital vibrance control via raw `nvapi64.dll` binding.
//!
//! NVIDIA does not ship the DVC function ordinals in the public open-source
//! NVAPI header, but they are stable and well-documented from the SDK +
//! reverse-engineering (VibranceGUI, jNizM's AHK class, arcnmx/nvapi-rs).
//!
//! Flow: nvapi_QueryInterface(id) -> fn ptr -> call.
//!   NvAPI_Initialize                0x0150E828
//!   NvAPI_EnumNvidiaDisplayHandle   0x9ABDD40D
//!   NvAPI_GetDVCInfoEx              0x0E45002D   (driver-reported scale; measured here 0..=100, default 50)
//!   NvAPI_SetDVCLevelEx             0x4A82C2B1
//!
//! NO DLL injection. All vibrance writes go through the NVIDIA display driver.
//! BattlEye / EAC safe — this is the same path the Nvidia Control Panel uses.

#![allow(non_snake_case)]

use crate::vibrance::VibranceInfo;
use std::os::raw::{c_int, c_void};

type NvApiStatus = c_int; // 0 == NVAPI_OK
type NvDisplayHandle = *mut c_void;

const NVAPI_OK: NvApiStatus = 0;
const NVAPI_MAX_DISPLAYS: u32 = 16;

// NV_DISPLAY_DVC_INFO_EX — version-stamped struct. version = sizeof | (ver<<16).
#[repr(C)]
#[derive(Clone, Copy)]
struct NvDisplayDvcInfoEx {
    version: u32,
    current_level: i32,
    min_level: i32,
    max_level: i32,
    default_level: i32,
}

fn make_version<T>(ver: u32) -> u32 {
    (std::mem::size_of::<T>() as u32) | (ver << 16)
}

// Raw fn-pointer signatures resolved via QueryInterface.
type FnQueryInterface = unsafe extern "C" fn(u32) -> *mut c_void;
type FnInitialize = unsafe extern "C" fn() -> NvApiStatus;
type FnEnumDisplay = unsafe extern "C" fn(u32, *mut NvDisplayHandle) -> NvApiStatus;
type FnGetDvcEx = unsafe extern "C" fn(NvDisplayHandle, u32, *mut NvDisplayDvcInfoEx) -> NvApiStatus;
type FnSetDvcEx = unsafe extern "C" fn(NvDisplayHandle, u32, *mut NvDisplayDvcInfoEx) -> NvApiStatus;

pub struct Nvapi {
    _lib: libloading::Library,
    query: FnQueryInterface,
    initialized: bool,
}

impl Nvapi {
    /// Load nvapi64.dll, resolve QueryInterface, call NvAPI_Initialize.
    /// Returns None on non-NVIDIA systems (DLL absent) — caller degrades gracefully.
    pub fn load() -> Option<Self> {
        let lib = unsafe { libloading::Library::new("nvapi64.dll") }.ok()?;
        // nvapi_QueryInterface is the single exported symbol; everything else
        // is resolved through it by ordinal.
        let query: FnQueryInterface = unsafe {
            let sym: libloading::Symbol<FnQueryInterface> = lib.get(b"nvapi_QueryInterface\0").ok()?;
            *sym
        };
        let mut me = Nvapi { _lib: lib, query, initialized: false };
        let init: FnInitialize = unsafe { std::mem::transmute((me.query)(0x0150_E828)) };
        if (init as usize) == 0 {
            return None;
        }
        let st = unsafe { init() };
        if st != NVAPI_OK {
            log::warn!("NvAPI_Initialize failed: {st}");
            return None;
        }
        me.initialized = true;
        Some(me)
    }

    /// Resolve a QueryInterface ordinal to a typed fn pointer. `T` must be a
    /// bare fn-pointer type (pointer-sized) — the size check below catches an
    /// accidental non-pointer `T` before the transmute would read garbage.
    fn resolve<T>(&self, id: u32) -> Option<T> {
        debug_assert_eq!(std::mem::size_of::<T>(), std::mem::size_of::<*mut c_void>());
        let ptr = unsafe { (self.query)(id) };
        if ptr.is_null() || std::mem::size_of::<T>() != std::mem::size_of::<*mut c_void>() {
            None
        } else {
            Some(unsafe { std::mem::transmute_copy(&ptr) })
        }
    }

    /// Enumerate the first NVIDIA display handle (the primary GPU output).
    fn primary_display(&self) -> Option<NvDisplayHandle> {
        self.all_displays().into_iter().next()
    }

    /// Enumerate every NVIDIA display handle (one per connected output).
    fn all_displays(&self) -> Vec<NvDisplayHandle> {
        let mut handles = Vec::new();
        let enum_fn: FnEnumDisplay = match self.resolve(0x9ABD_D40D) {
            Some(f) => f,
            None => return handles,
        };
        for i in 0..NVAPI_MAX_DISPLAYS {
            let mut handle: NvDisplayHandle = std::ptr::null_mut();
            let st = unsafe { enum_fn(i, &mut handle) };
            if st == NVAPI_OK && !handle.is_null() {
                handles.push(handle);
            }
        }
        handles
    }

    /// Read vibrance for a specific display handle.
    fn get_vibrance_for(&self, display: NvDisplayHandle) -> Result<VibranceInfo, String> {
        let get_fn: FnGetDvcEx = self.resolve(0x0E45_002D).ok_or("GetDVCInfoEx unavailable")?;
        let mut info = NvDisplayDvcInfoEx {
            version: make_version::<NvDisplayDvcInfoEx>(1),
            current_level: 0,
            min_level: 0,
            max_level: 0,
            default_level: 0,
        };
        let st = unsafe { get_fn(display, 0, &mut info) };
        if st != NVAPI_OK {
            return Err(format!("GetDVCInfoEx failed: {st}"));
        }
        Ok(VibranceInfo {
            current: info.current_level,
            min: info.min_level,
            max: info.max_level,
            default: info.default_level,
        })
    }

    /// Set vibrance for a specific display handle (clamped to its min/max).
    fn set_vibrance_for(&self, display: NvDisplayHandle, level: i32) -> Result<(), String> {
        let set_fn: FnSetDvcEx = self.resolve(0x4A82_C2B1).ok_or("SetDVCLevelEx unavailable")?;
        let cur = self.get_vibrance_for(display)?;
        let clamped = level.clamp(cur.min, cur.max);
        let mut info = NvDisplayDvcInfoEx {
            version: make_version::<NvDisplayDvcInfoEx>(1),
            current_level: clamped,
            min_level: cur.min,
            max_level: cur.max,
            default_level: cur.default,
        };
        let st = unsafe { set_fn(display, 0, &mut info) };
        if st != NVAPI_OK {
            return Err(format!("SetDVCLevelEx failed: {st}"));
        }
        Ok(())
    }

    /// Read current vibrance (0..=63 on the Ex scale) for the primary output.
    pub fn get_vibrance(&self) -> Result<VibranceInfo, String> {
        let display = self.primary_display().ok_or("no NVIDIA display handle")?;
        self.get_vibrance_for(display)
    }

    /// Set the same vibrance level on EVERY connected NVIDIA output.
    /// This is the correct path for presets — otherwise monitor 2 keeps a stale value
    /// and shows a visibly different color from monitor 1.
    pub fn set_vibrance(&self, level: i32) -> Result<(), String> {
        let displays = self.all_displays();
        if displays.is_empty() {
            return Err("no NVIDIA display handle".into());
        }
        let mut last_err = None;
        for d in displays {
            if let Err(e) = self.set_vibrance_for(d, level) {
                last_err = Some(e);
            }
        }
        match last_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Restore every connected NVIDIA output to its OWN native default vibrance.
    /// This is what "Normal" uses so each monitor picks up the color Windows/the
    /// driver natively programmed for it, instead of a shared forced level.
    pub fn reset_vibrance_to_default(&self) -> Result<(), String> {
        let displays = self.all_displays();
        if displays.is_empty() {
            return Err("no NVIDIA display handle".into());
        }
        let mut last_err = None;
        for d in displays {
            let default = match self.get_vibrance_for(d) {
                Ok(info) => info.default,
                Err(e) => {
                    last_err = Some(e);
                    continue;
                }
            };
            if let Err(e) = self.set_vibrance_for(d, default) {
                last_err = Some(e);
            }
        }
        match last_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}
