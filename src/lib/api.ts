// Typed wrappers over the Tauri backend commands. Mirrors the Rust signatures
// in src-tauri/src/lib.rs — keep in sync.

import { invoke } from "@tauri-apps/api/core";

export interface ColorDials {
  gamma: number; // 0.30..2.80, 1.0 neutral
  brightness: number; // -0.50..0.50, 0 neutral
  contrast: number; // 0.50..2.00, 1.0 neutral
}

export interface Preset {
  slot: string;
  name: string;
  dials: ColorDials;
  vibrance: number; // level in the machine's driver-reported scale (see VibranceInfo)
  exe?: string | null; // bound program (lowercased exe basename) or null
}

export interface PresetStore {
  presets: Preset[];
  active: string;
  next_id: number;
  autostart: boolean;
  hotkeys: boolean;
}

export interface VibranceInfo {
  current: number;
  min: number;
  max: number;
  default: number;
}

export interface SystemStatus {
  /** GPU vendor providing vibrance, or null when only gamma is available. */
  vendor: "nvidia" | "amd" | null;
  vibrance: VibranceInfo | null;
}

export const getStatus = () => invoke<SystemStatus>("get_status");
export const getPresets = () => invoke<PresetStore>("get_presets");

export const applyColor = (dials: ColorDials, vibrance: number) =>
  invoke<void>("apply_color", { dials, vibrance });

export const selectPreset = (slot: string) =>
  invoke<Preset>("select_preset", { slot });

export const savePreset = (slot: string, dials: ColorDials, vibrance: number) =>
  invoke<void>("save_preset", { slot, dials, vibrance });

export const resetDisplay = () => invoke<void>("reset_display");

// Preset CRUD. create returns the new preset; delete returns the fresh store.
export const createPreset = (name: string) =>
  invoke<Preset>("create_preset", { name });

export const deletePreset = (slot: string) =>
  invoke<PresetStore>("delete_preset", { slot });

export const renamePreset = (slot: string, name: string) =>
  invoke<void>("rename_preset", { slot, name });

// Program binding. setBinding returns the fresh store (binding badges re-sync);
// pass exe = null to clear. listWindowPrograms → visible-window programs for the picker.
export const setBinding = (slot: string, exe: string | null) =>
  invoke<PresetStore>("set_binding", { slot, exe });

// Programs with a visible window, as {exe, title} — the binder's picker source.
// `exe` is the basename the watcher binds on; `title` is shown to the user.
export type WindowProc = { exe: string; title: string };
export const listWindowPrograms = () =>
  invoke<WindowProc[]>("list_window_programs");

// Import/export. The frontend picks the path via the dialog plugin; the backend
// does the file I/O. export writes user presets (not Normal) to `path`; import
// appends presets from `path` and returns the fresh store.
export const exportPresets = (path: string) =>
  invoke<void>("export_presets", { path });

export const importPresets = (path: string) =>
  invoke<PresetStore>("import_presets", { path });

// Start-with-Windows preference (HKCU Run key + persisted flag).
// setAutostart returns the new value.
export const getAutostart = () => invoke<boolean>("get_autostart");
export const setAutostart = (enabled: boolean) =>
  invoke<boolean>("set_autostart", { enabled });

// Global hotkeys preference (Ctrl+Shift+F9 cycles presets, Ctrl+Shift+F10
// snaps to Normal — handled backend-side, no window focus needed).
// setHotkeys returns the new value.
export const getHotkeys = () => invoke<boolean>("get_hotkeys");
export const setHotkeys = (enabled: boolean) =>
  invoke<boolean>("set_hotkeys", { enabled });

// Settings-page system actions. uninstallApp launches the NSIS uninstaller and
// exits the app (errors if this copy wasn't installed via the setup wizard);
// openUrl opens an https link in the default browser.
export const uninstallApp = () => invoke<void>("uninstall_app");
export const openUrl = (url: string) => invoke<void>("open_url", { url });

// Auto-update (GitHub Releases, signature-verified). checkUpdate resolves to
// metadata for a newer build or null when up to date. installUpdate downloads +
// installs, emitting "update-progress" (0..=100); on success the app restarts,
// so the promise only settles on failure. The backend also checks on boot and
// emits "update-available" with the same metadata shape.
export type UpdateMeta = { version: string; notes: string };
export const checkUpdate = () => invoke<UpdateMeta | null>("check_update");
export const installUpdate = () => invoke<void>("install_update");

// ── Crosshairs ── (mirrors src-tauri/src/crosshair.rs; ranges are clamped backend-side)
export type CrosshairStyle = {
  color: string; // "#rrggbb"
  opacity: number; // 0.1..1
  arms: boolean;
  length: number; // 1..40 px
  thickness: number; // 1..10 px
  gap: number; // 0..30 px
  t_style: boolean; // hide the top arm
  dot: boolean;
  dot_size: number; // 1..10 px
  ring: boolean;
  ring_radius: number; // 2..60 px
  ring_thickness: number; // 1..6 px
  outline: boolean;
  outline_thickness: number; // 1..3 px
  outline_color: string;
  offset_x: number; // -50..50 px on-screen nudge
  offset_y: number;
};

export type Crosshair = {
  id: string;
  name: string;
  style: CrosshairStyle;
  exe?: string | null; // bound = switches in while this program is in front
};

// `selected` value for "no crosshair outside bound games" (crosshair.rs NONE).
export const NO_CROSSHAIR = "none";

export type CrosshairStore = {
  crosshairs: Crosshair[];
  selected: string; // the user's pick, or NO_CROSSHAIR
  enabled: boolean; // master overlay switch (Ctrl+Shift+F11)
  next_id: number;
};

export const getCrosshairs = () => invoke<CrosshairStore>("get_crosshairs");
export const createCrosshair = (name: string, style: CrosshairStyle | null) =>
  invoke<Crosshair>("create_crosshair", { name, style });
export const updateCrosshair = (id: string, style: CrosshairStyle) =>
  invoke<void>("update_crosshair", { id, style });
export const renameCrosshair = (id: string, name: string) =>
  invoke<void>("rename_crosshair", { id, name });
export const deleteCrosshair = (id: string) =>
  invoke<CrosshairStore>("delete_crosshair", { id });
export const selectCrosshair = (id: string) => invoke<void>("select_crosshair", { id });
export const setCrosshairBinding = (id: string, exe: string | null) =>
  invoke<CrosshairStore>("set_crosshair_binding", { id, exe });
export const setCrosshairEnabled = (enabled: boolean) =>
  invoke<boolean>("set_crosshair_enabled", { enabled });

// Pixel-exact render from the same Rust renderer the overlay uses. `half` is the
// center boundary in bitmap pixels (where the screen's center lands).
export type CrosshairImage = { image: ImageData; half: number };
export async function renderCrosshair(style: CrosshairStyle): Promise<CrosshairImage> {
  const buf = await invoke<ArrayBuffer>("render_crosshair", { style });
  const view = new DataView(buf);
  const size = view.getUint32(0, true);
  const half = view.getUint32(4, true);
  const pixels = new Uint8ClampedArray(buf, 8, size * size * 4);
  return { image: new ImageData(pixels, size, size), half };
}

// Accent palette cycled by a preset's position among non-Normal presets.
// Normal is fixed grey; everything else pulls from a 6-hue set (see app.css).
const ACCENT_CYCLE = [
  "var(--slot-a)",
  "var(--slot-b)",
  "var(--slot-c)",
  "var(--slot-d)",
  "var(--slot-e)",
  "var(--slot-f)",
];

export const slotAccent = (slot: string, index = 0): string => {
  if (slot === "Normal") return "var(--slot-normal)";
  return ACCENT_CYCLE[index % ACCENT_CYCLE.length];
};
