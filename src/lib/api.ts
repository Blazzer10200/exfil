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
  vibrance: number; // 0..63 (NVAPI Ex scale)
  exe?: string | null; // bound program (lowercased exe basename) or null
}

export interface PresetStore {
  presets: Preset[];
  active: string;
  next_id: number;
}

export interface VibranceInfo {
  current: number;
  min: number;
  max: number;
  default: number;
}

export interface SystemStatus {
  nvidia: boolean;
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
