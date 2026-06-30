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
}

export interface PresetStore {
  presets: Preset[];
  active: string;
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

// Per-slot accent CSS var (matches app.css --slot-* tokens).
export const slotAccent = (slot: string): string => {
  const map: Record<string, string> = {
    Normal: "var(--slot-normal)",
    Preset1: "var(--slot-p1)",
    Preset2: "var(--slot-p2)",
    Preset3: "var(--slot-p3)",
  };
  return map[slot] ?? "var(--accent)";
};
