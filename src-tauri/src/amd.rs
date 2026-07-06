//! AMD digital-vibrance (display saturation) via raw `atiadlxx.dll` ADL2 binding.
//!
//! ADL facts verified against the official ADL SDK doxygen
//! (gpuopen-librariesandsdks.github.io/adl): ADL2 entry points + param order,
//! ADL_DISPLAY_COLOR_SATURATION = 1<<2, DISPLAYINFO CONNECTED/MAPPED masks,
//! ADLDisplayID/ADLDisplayInfo layouts, caller-frees-via-callback memory
//! contract, ADL_OK = 0. Saturation plays the role of NVIDIA's digital
//! vibrance; its range/default come from ADL2_Display_Color_Get at runtime
//! (typically 0..=200, default 100 — note 0 means GRAYSCALE, not neutral).
//!
//! NO DLL injection — all writes go through the AMD display driver, the same
//! path AMD's own Radeon software uses. BattlEye / EAC safe.
//!
//! Built to FAIL CLOSED on untested hardware (this repo's dev machine is
//! NVIDIA): a display is only kept if it passes ColorCaps AND answers a live
//! Color_Get; DisplayInfo reads are clamped to the actual allocation size; and
//! every failure degrades to "no AMD vibrance" instead of a panic
//! (`panic = "abort"` in release).

#![allow(non_snake_case)]

use crate::vibrance::VibranceInfo;
use std::os::raw::{c_int, c_void};

const ADL_OK: c_int = 0;
const ADL_DISPLAY_COLOR_SATURATION: c_int = 1 << 2;
const ADL_DISPLAY_DISPLAYINFO_DISPLAYCONNECTED: c_int = 0x1;
const ADL_DISPLAY_DISPLAYINFO_DISPLAYMAPPED: c_int = 0x2;
const ADL_MAX_PATH: usize = 256;

type AdlContext = *mut c_void;
type MallocCallback = unsafe extern "system" fn(c_int) -> *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct ADLDisplayID {
    iDisplayLogicalIndex: c_int,
    iDisplayPhysicalIndex: c_int,
    iDisplayLogicalAdapterIndex: c_int,
    iDisplayPhysicalAdapterIndex: c_int,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ADLDisplayInfo {
    displayID: ADLDisplayID,
    iDisplayControllerIndex: c_int,
    strDisplayName: [u8; ADL_MAX_PATH],
    strDisplayManufacturerName: [u8; ADL_MAX_PATH],
    iDisplayType: c_int,
    iDisplayOutputType: c_int,
    iDisplayConnector: c_int,
    iDisplayInfoMask: c_int,
    iDisplayInfoValue: c_int,
}

// ── ADL output-buffer memory contract ──
// ADL allocates output arrays through a caller-supplied malloc and the CALLER
// frees them. Allocations are size-prefixed so free needs no bookkeeping and
// DisplayInfo reads can be clamped to what was actually allocated.
const ALLOC_HEADER: usize = 16; // keeps the returned pointer 16-aligned

unsafe extern "system" fn adl_alloc(size: c_int) -> *mut c_void {
    if size <= 0 {
        return std::ptr::null_mut();
    }
    let total = size as usize + ALLOC_HEADER;
    let Ok(layout) = std::alloc::Layout::from_size_align(total, ALLOC_HEADER) else {
        return std::ptr::null_mut();
    };
    let base = std::alloc::alloc(layout);
    if base.is_null() {
        return std::ptr::null_mut();
    }
    (base as *mut usize).write(total);
    base.add(ALLOC_HEADER) as *mut c_void
}

/// Payload bytes ADL requested for the allocation behind `ptr`.
unsafe fn adl_alloc_size(ptr: *mut c_void) -> usize {
    let base = (ptr as *mut u8).sub(ALLOC_HEADER);
    (base as *const usize).read() - ALLOC_HEADER
}

unsafe fn adl_free(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let base = (ptr as *mut u8).sub(ALLOC_HEADER);
    let total = (base as *const usize).read();
    let layout = std::alloc::Layout::from_size_align_unchecked(total, ALLOC_HEADER);
    std::alloc::dealloc(base, layout);
}

// x64-only app: `extern "system"` == `extern "C"` here; "system" documents the
// __stdcall the ADL headers declare (which only matters on x86).
type FnMainControlCreate = unsafe extern "system" fn(MallocCallback, c_int, *mut AdlContext) -> c_int;
type FnMainControlDestroy = unsafe extern "system" fn(AdlContext) -> c_int;
type FnNumAdapters = unsafe extern "system" fn(AdlContext, *mut c_int) -> c_int;
type FnAdapterActive = unsafe extern "system" fn(AdlContext, c_int, *mut c_int) -> c_int;
type FnDisplayInfoGet =
    unsafe extern "system" fn(AdlContext, c_int, *mut c_int, *mut *mut ADLDisplayInfo, c_int) -> c_int;
type FnColorCapsGet = unsafe extern "system" fn(AdlContext, c_int, c_int, *mut c_int, *mut c_int) -> c_int;
type FnColorGet = unsafe extern "system" fn(
    AdlContext,
    c_int,
    c_int,
    c_int,
    *mut c_int, // current
    *mut c_int, // default
    *mut c_int, // min
    *mut c_int, // max
    *mut c_int, // step
) -> c_int;
type FnColorSet = unsafe extern "system" fn(AdlContext, c_int, c_int, c_int, c_int) -> c_int;

pub struct Adl {
    _lib: libloading::Library,
    context: AdlContext,
    destroy: FnMainControlDestroy,
    num_adapters: FnNumAdapters,
    adapter_active: FnAdapterActive,
    display_info_get: FnDisplayInfoGet,
    color_caps_get: FnColorCapsGet,
    color_get: FnColorGet,
    color_set: FnColorSet,
}

// Raw context handle + fn pointers. ADL2 contexts exist precisely so multiple
// clients/threads can drive ADL concurrently (vs the legacy global ADL_ API).
unsafe impl Send for Adl {}
unsafe impl Sync for Adl {}

impl Drop for Adl {
    fn drop(&mut self) {
        // Fields drop after this body, so the library is still loaded here.
        unsafe { (self.destroy)(self.context) };
    }
}

macro_rules! sym {
    ($lib:expr, $name:literal) => {{
        let s = unsafe { $lib.get($name) }.ok()?;
        *s
    }};
}

impl Adl {
    /// Load atiadlxx.dll and open an ADL2 context. Returns None on non-AMD
    /// systems (DLL absent) or when no saturation-capable display answers —
    /// callers degrade to gamma-only, exactly like the NVAPI path.
    pub fn load() -> Option<Self> {
        let lib = unsafe { libloading::Library::new("atiadlxx.dll") }.ok()?;
        let create: FnMainControlCreate = sym!(lib, b"ADL2_Main_Control_Create\0");
        let destroy: FnMainControlDestroy = sym!(lib, b"ADL2_Main_Control_Destroy\0");
        let num_adapters: FnNumAdapters = sym!(lib, b"ADL2_Adapter_NumberOfAdapters_Get\0");
        let adapter_active: FnAdapterActive = sym!(lib, b"ADL2_Adapter_Active_Get\0");
        let display_info_get: FnDisplayInfoGet = sym!(lib, b"ADL2_Display_DisplayInfo_Get\0");
        let color_caps_get: FnColorCapsGet = sym!(lib, b"ADL2_Display_ColorCaps_Get\0");
        let color_get: FnColorGet = sym!(lib, b"ADL2_Display_Color_Get\0");
        let color_set: FnColorSet = sym!(lib, b"ADL2_Display_Color_Set\0");

        let mut context: AdlContext = std::ptr::null_mut();
        let st = unsafe { create(adl_alloc, 1, &mut context) };
        if st != ADL_OK || context.is_null() {
            log::warn!("ADL2_Main_Control_Create failed: {st}");
            return None;
        }
        let adl = Adl {
            _lib: lib,
            context,
            destroy,
            num_adapters,
            adapter_active,
            display_info_get,
            color_caps_get,
            color_get,
            color_set,
        };
        // Fail closed: no display that provably does saturation → no backend.
        if adl.displays().is_empty() {
            log::info!("ADL loaded but no saturation-capable display found");
            return None; // Drop closes the context
        }
        Some(adl)
    }

    /// Enumerate (adapter, display) pairs that provably support saturation.
    /// Re-run per call (like the NVAPI path) so monitor hotplug stays correct.
    fn displays(&self) -> Vec<(c_int, c_int)> {
        let mut out: Vec<(c_int, c_int)> = Vec::new();
        let mut n: c_int = 0;
        if unsafe { (self.num_adapters)(self.context, &mut n) } != ADL_OK {
            return out;
        }
        for adapter in 0..n {
            let mut active: c_int = 0;
            if unsafe { (self.adapter_active)(self.context, adapter, &mut active) } != ADL_OK
                || active == 0
            {
                continue;
            }
            let mut count: c_int = 0;
            let mut infos: *mut ADLDisplayInfo = std::ptr::null_mut();
            if unsafe { (self.display_info_get)(self.context, adapter, &mut count, &mut infos, 0) }
                != ADL_OK
                || infos.is_null()
            {
                continue;
            }
            // Clamp to what ADL actually allocated through our callback, so a
            // struct-layout mismatch can never read past the buffer.
            let fit = unsafe { adl_alloc_size(infos as *mut c_void) }
                / std::mem::size_of::<ADLDisplayInfo>();
            let count = (count.max(0) as usize).min(fit);
            for i in 0..count {
                let d = unsafe { *infos.add(i) };
                let mask =
                    ADL_DISPLAY_DISPLAYINFO_DISPLAYCONNECTED | ADL_DISPLAY_DISPLAYINFO_DISPLAYMAPPED;
                if (d.iDisplayInfoValue & mask) != mask
                    || d.displayID.iDisplayLogicalAdapterIndex != adapter
                {
                    continue;
                }
                let display = d.displayID.iDisplayLogicalIndex;
                if out.contains(&(adapter, display)) {
                    continue;
                }
                // Keep only displays that expressly support saturation AND
                // answer a live query — this also fails closed if any ADL
                // assumption is off on real hardware.
                let mut caps: c_int = 0;
                let mut valid: c_int = 0;
                if unsafe {
                    (self.color_caps_get)(self.context, adapter, display, &mut caps, &mut valid)
                } == ADL_OK
                    && (caps & valid & ADL_DISPLAY_COLOR_SATURATION) == 0
                {
                    continue;
                }
                if self.color_get_for(adapter, display).is_ok() {
                    out.push((adapter, display));
                }
            }
            unsafe { adl_free(infos as *mut c_void) };
        }
        out
    }

    /// Read saturation info for one display.
    fn color_get_for(&self, adapter: c_int, display: c_int) -> Result<VibranceInfo, String> {
        let (mut cur, mut def, mut min, mut max, mut step) = (0, 0, 0, 0, 0);
        let st = unsafe {
            (self.color_get)(
                self.context,
                adapter,
                display,
                ADL_DISPLAY_COLOR_SATURATION,
                &mut cur,
                &mut def,
                &mut min,
                &mut max,
                &mut step,
            )
        };
        if st != ADL_OK {
            return Err(format!("ADL2_Display_Color_Get failed: {st}"));
        }
        Ok(VibranceInfo { current: cur, min, max, default: def })
    }

    /// Set saturation for one display (clamped to its own range).
    fn color_set_for(&self, adapter: c_int, display: c_int, level: i32) -> Result<(), String> {
        let info = self.color_get_for(adapter, display)?;
        let clamped = level.clamp(info.min, info.max);
        let st = unsafe {
            (self.color_set)(self.context, adapter, display, ADL_DISPLAY_COLOR_SATURATION, clamped)
        };
        if st != ADL_OK {
            return Err(format!("ADL2_Display_Color_Set failed: {st}"));
        }
        Ok(())
    }

    /// Saturation info for the primary output.
    pub fn get_vibrance(&self) -> Result<VibranceInfo, String> {
        let (a, d) = self.displays().into_iter().next().ok_or("no AMD display")?;
        self.color_get_for(a, d)
    }

    /// Set the same saturation level on EVERY output (mirrors the NVAPI path —
    /// otherwise a second monitor keeps a stale level and looks different).
    pub fn set_vibrance(&self, level: i32) -> Result<(), String> {
        let displays = self.displays();
        if displays.is_empty() {
            return Err("no AMD display".into());
        }
        let mut last_err = None;
        for (a, d) in displays {
            if let Err(e) = self.color_set_for(a, d, level) {
                last_err = Some(e);
            }
        }
        match last_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Restore every output to its OWN driver-default saturation ("Normal").
    pub fn reset_vibrance_to_default(&self) -> Result<(), String> {
        let displays = self.displays();
        if displays.is_empty() {
            return Err("no AMD display".into());
        }
        let mut last_err = None;
        for (a, d) in displays {
            match self.color_get_for(a, d) {
                Ok(info) => {
                    if let Err(e) = self.color_set_for(a, d, info.default) {
                        last_err = Some(e);
                    }
                }
                Err(e) => last_err = Some(e),
            }
        }
        match last_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}
