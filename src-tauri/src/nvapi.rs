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
//! Dithering (smooths the banding a gamma ramp causes on an 8-bit signal) uses
//! the same route. The dither ordinals are not in the public header either;
//! signatures follow ledoge/novideo_srgb and were verified on driver 32.0.16.2140:
//!   NvAPI_GetAssociatedDisplayOutputId        0xD995937E
//!   NvAPI_GetPhysicalGPUsFromDisplay          0x34EF9506
//!   NvAPI_GetGPUIDfromPhysicalGPU             0x6533EA3E
//!   NvAPI_SYS_GetDisplayIdFromGpuAndOutputId  0x08F2BAB4
//!   NvAPI_Disp_ColorControl                   0x92F9D80D   (read-only here: output bit depth)
//!   NvAPI_GPU_GetDitherControl                0x932AC8FB
//!   NvAPI_GPU_SetDitherControl                0xDF0DFCDD
//!
//! NO DLL injection. All vibrance writes go through the NVIDIA display driver.
//! BattlEye / EAC safe — this is the same path the Nvidia Control Panel uses.

use crate::vibrance::{clamp_level, for_each_display, VibranceInfo};
use std::os::raw::{c_int, c_void};

type NvApiStatus = c_int; // 0 == NVAPI_OK
type NvDisplayHandle = *mut c_void;
type NvPhysicalGpuHandle = *mut c_void;

const NVAPI_OK: NvApiStatus = 0;
const NVAPI_MAX_DISPLAYS: u32 = 16;
const NVAPI_MAX_PHYSICAL_GPUS: usize = 64;

// Dither control values (state, bits, mode) as the driver takes them.
const DITHER_STATE_DEFAULT: i32 = 0; // driver decides (off on an 8-bit+ signal)
const DITHER_STATE_ENABLED: i32 = 1;
const DITHER_BITS_6: i32 = 0;
const DITHER_BITS_8: i32 = 1;
const DITHER_BITS_10: i32 = 2;
const DITHER_MODE_TEMPORAL: i32 = 4;
// NV_BPC values reported by ColorControl.
const NV_BPC_6: u32 = 1;
const NV_BPC_10: u32 = 3;
const NV_COLOR_CMD_GET: u8 = 1;

// NV_GPU_DITHER_CONTROL_V1 (24 bytes, version 0x10018).
#[repr(C)]
#[derive(Clone, Copy)]
struct NvDitherControl {
    version: u32,
    state: i32,
    bits: i32,
    mode: i32,
    bits_caps: u32,
    mode_caps: u32,
}

// NV_COLOR_DATA_V5 (24 bytes). Only ever sent with NV_COLOR_CMD_GET.
#[repr(C)]
#[derive(Clone, Copy)]
struct NvColorData {
    version: u32,
    size: u16,
    cmd: u8,
    color_format: u8,
    colorimetry: u8,
    dynamic_range: u8,
    bpc: u32,
    selection_policy: u32,
    depth: u32,
}

/// One GPU output, addressed the three ways the dither calls need.
#[derive(Clone, Copy)]
struct DitherTarget {
    gpu_id: u32,
    output_id: u32,
    display_id: u32,
}

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
type FnGetDvcEx =
    unsafe extern "C" fn(NvDisplayHandle, u32, *mut NvDisplayDvcInfoEx) -> NvApiStatus;
type FnSetDvcEx =
    unsafe extern "C" fn(NvDisplayHandle, u32, *mut NvDisplayDvcInfoEx) -> NvApiStatus;
type FnOutputId = unsafe extern "C" fn(NvDisplayHandle, *mut u32) -> NvApiStatus;
type FnGpusFromDisplay =
    unsafe extern "C" fn(NvDisplayHandle, *mut NvPhysicalGpuHandle, *mut u32) -> NvApiStatus;
type FnGpuId = unsafe extern "C" fn(NvPhysicalGpuHandle, *mut u32) -> NvApiStatus;
type FnDisplayId = unsafe extern "C" fn(NvPhysicalGpuHandle, u32, *mut u32) -> NvApiStatus;
type FnColorControl = unsafe extern "C" fn(u32, *mut NvColorData) -> NvApiStatus;
type FnGetDither = unsafe extern "C" fn(u32, *mut NvDitherControl) -> NvApiStatus;
type FnSetDither = unsafe extern "C" fn(u32, u32, i32, i32, i32) -> NvApiStatus;

pub struct Nvapi {
    _lib: libloading::Library,
    query: FnQueryInterface,
}

const NO_DISPLAY: &str = "no NVIDIA display handle";

impl Nvapi {
    /// Load nvapi64.dll, resolve QueryInterface, call NvAPI_Initialize.
    /// Returns None on non-NVIDIA systems (DLL absent) — caller degrades gracefully.
    pub fn load() -> Option<Self> {
        let lib = unsafe { libloading::Library::new("nvapi64.dll") }.ok()?;
        // nvapi_QueryInterface is the single exported symbol; everything else
        // is resolved through it by ordinal.
        let query: FnQueryInterface = unsafe {
            let sym: libloading::Symbol<FnQueryInterface> =
                lib.get(b"nvapi_QueryInterface\0").ok()?;
            *sym
        };
        let me = Nvapi { _lib: lib, query };
        // resolve() null-checks before producing the fn pointer; a null fn
        // pointer must never exist, even transiently.
        let init: FnInitialize = me.resolve(0x0150_E828)?;
        let st = unsafe { init() };
        if st != NVAPI_OK {
            log::warn!("NvAPI_Initialize failed: {st}");
            return None;
        }
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
        let get_fn: FnGetDvcEx = self
            .resolve(0x0E45_002D)
            .ok_or("GetDVCInfoEx unavailable")?;
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
        let set_fn: FnSetDvcEx = self
            .resolve(0x4A82_C2B1)
            .ok_or("SetDVCLevelEx unavailable")?;
        let cur = self.get_vibrance_for(display)?;
        let clamped = clamp_level(level, &cur)?;
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

    /// Read current vibrance for the primary output, in the driver-reported
    /// scale (measured here: 0..=100 default 50 — not the "0..63" folklore).
    pub fn get_vibrance(&self) -> Result<VibranceInfo, String> {
        let display = self.primary_display().ok_or(NO_DISPLAY)?;
        self.get_vibrance_for(display)
    }

    /// Set the same vibrance level on EVERY connected NVIDIA output.
    /// This is the correct path for presets — otherwise monitor 2 keeps a stale value
    /// and shows a visibly different color from monitor 1.
    pub fn set_vibrance(&self, level: i32) -> Result<(), String> {
        for_each_display(self.all_displays(), NO_DISPLAY, |d| {
            self.set_vibrance_for(d, level)
        })
    }

    /// Restore every connected NVIDIA output to its OWN native default vibrance.
    /// This is what "Normal" uses so each monitor picks up the color Windows/the
    /// driver natively programmed for it, instead of a shared forced level.
    pub fn reset_vibrance_to_default(&self) -> Result<(), String> {
        for_each_display(self.all_displays(), NO_DISPLAY, |d| {
            let default = self.get_vibrance_for(d)?.default;
            self.set_vibrance_for(d, default)
        })
    }

    /// Resolve one display handle to the ids the dither calls take.
    fn dither_target(&self, display: NvDisplayHandle) -> Result<DitherTarget, String> {
        let output_fn: FnOutputId = self
            .resolve(0xD995_937E)
            .ok_or("GetAssociatedDisplayOutputId unavailable")?;
        let gpus_fn: FnGpusFromDisplay = self
            .resolve(0x34EF_9506)
            .ok_or("GetPhysicalGPUsFromDisplay unavailable")?;
        let gpu_id_fn: FnGpuId = self
            .resolve(0x6533_EA3E)
            .ok_or("GetGPUIDfromPhysicalGPU unavailable")?;
        let display_id_fn: FnDisplayId = self
            .resolve(0x08F2_BAB4)
            .ok_or("GetDisplayIdFromGpuAndOutputId unavailable")?;

        let mut output_id = 0u32;
        let st = unsafe { output_fn(display, &mut output_id) };
        if st != NVAPI_OK {
            return Err(format!("GetAssociatedDisplayOutputId failed: {st}"));
        }
        let mut gpus: [NvPhysicalGpuHandle; NVAPI_MAX_PHYSICAL_GPUS] =
            [std::ptr::null_mut(); NVAPI_MAX_PHYSICAL_GPUS];
        let mut count = 0u32;
        let st = unsafe { gpus_fn(display, gpus.as_mut_ptr(), &mut count) };
        if st != NVAPI_OK {
            return Err(format!("GetPhysicalGPUsFromDisplay failed: {st}"));
        }
        let gpu = match gpus.first() {
            Some(g) if count > 0 && !g.is_null() => *g,
            _ => return Err("display has no physical GPU".into()),
        };
        let mut gpu_id = 0u32;
        let st = unsafe { gpu_id_fn(gpu, &mut gpu_id) };
        if st != NVAPI_OK {
            return Err(format!("GetGPUIDfromPhysicalGPU failed: {st}"));
        }
        let mut display_id = 0u32;
        let st = unsafe { display_id_fn(gpu, output_id, &mut display_id) };
        if st != NVAPI_OK {
            return Err(format!("GetDisplayIdFromGpuAndOutputId failed: {st}"));
        }
        Ok(DitherTarget {
            gpu_id,
            output_id,
            display_id,
        })
    }

    fn get_dither_for(&self, target: DitherTarget) -> Result<NvDitherControl, String> {
        let get_fn: FnGetDither = self
            .resolve(0x932A_C8FB)
            .ok_or("GetDitherControl unavailable")?;
        let mut ctl = NvDitherControl {
            version: make_version::<NvDitherControl>(1),
            state: 0,
            bits: 0,
            mode: 0,
            bits_caps: 0,
            mode_caps: 0,
        };
        let st = unsafe { get_fn(target.display_id, &mut ctl) };
        if st != NVAPI_OK {
            return Err(format!("GetDitherControl failed: {st}"));
        }
        Ok(ctl)
    }

    /// Bits per channel of the signal on the output carrying the fewest, for
    /// the shades readout. None when no output answers.
    pub fn output_bits(&self) -> Option<u8> {
        self.all_displays()
            .into_iter()
            .filter_map(|d| self.dither_target(d).ok())
            .filter_map(|t| self.bpc_for(t).ok())
            .map(|bpc| match bpc {
                NV_BPC_6 => 6,
                NV_BPC_10 => 10,
                b if b > NV_BPC_10 => 12,
                _ => 8,
            })
            .min()
    }

    /// Dither depth that matches the signal the driver is sending right now.
    /// Dithering to fewer bits than the link carries would throw precision away.
    fn dither_bits_for(&self, target: DitherTarget) -> Result<i32, String> {
        Ok(match self.bpc_for(target)? {
            NV_BPC_6 => DITHER_BITS_6,
            // 12-bit and up have no matching dither depth; 10 is the highest.
            b if b >= NV_BPC_10 => DITHER_BITS_10,
            _ => DITHER_BITS_8,
        })
    }

    /// NV_BPC code of the signal on one output. Read-only: ColorControl is
    /// only ever sent with NV_COLOR_CMD_GET.
    fn bpc_for(&self, target: DitherTarget) -> Result<u32, String> {
        let color_fn: FnColorControl = self
            .resolve(0x92F9_D80D)
            .ok_or("ColorControl unavailable")?;
        let mut data = NvColorData {
            version: make_version::<NvColorData>(5),
            size: std::mem::size_of::<NvColorData>() as u16,
            cmd: NV_COLOR_CMD_GET,
            color_format: 0,
            colorimetry: 0,
            dynamic_range: 0,
            bpc: 0,
            selection_policy: 0,
            depth: 0,
        };
        let st = unsafe { color_fn(target.display_id, &mut data) };
        if st != NVAPI_OK {
            return Err(format!("ColorControl failed: {st}"));
        }
        Ok(data.bpc)
    }

    fn set_dither_for(&self, display: NvDisplayHandle, on: bool) -> Result<(), String> {
        let set_fn: FnSetDither = self
            .resolve(0xDF0D_FCDD)
            .ok_or("SetDitherControl unavailable")?;
        let target = self.dither_target(display)?;
        let (state, bits, mode) = if on {
            let caps = self.get_dither_for(target)?;
            let bits = self.dither_bits_for(target)?;
            if caps.bits_caps & (1 << bits) == 0
                || caps.mode_caps & (1 << DITHER_MODE_TEMPORAL) == 0
            {
                return Err("this output does not support temporal dithering".into());
            }
            (DITHER_STATE_ENABLED, bits, DITHER_MODE_TEMPORAL)
        } else {
            // Hand the decision back to the driver; with the state at default
            // the bits and mode are ignored.
            (DITHER_STATE_DEFAULT, 0, 0)
        };
        let st = unsafe { set_fn(target.gpu_id, target.output_id, state, bits, mode) };
        if st != NVAPI_OK {
            return Err(format!("SetDitherControl failed: {st}"));
        }
        Ok(())
    }

    /// Turn temporal dithering on for EVERY connected NVIDIA output, or hand it
    /// back to the driver default. Driver-level, like vibrance: no injection.
    pub fn set_dither(&self, on: bool) -> Result<(), String> {
        for_each_display(self.all_displays(), NO_DISPLAY, |d| {
            self.set_dither_for(d, on)
        })
    }
}
