// UI-only preferences persisted in localStorage (never display state).

export type View = "color" | "crosshair" | "library" | "games" | "settings";
export type Backdrop = "scene" | "bright" | "dark" | "noise" | "custom";

const KEY = "exfil.prefs";

type Prefs = {
  view: View | null;
  zoom: number;
  backdrop: Backdrop;
  backdropImage: string | null;
  grid: boolean;
};

const DEFAULTS: Prefs = { view: null, zoom: 4, backdrop: "scene", backdropImage: null, grid: true };

function read(): Prefs {
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? { ...DEFAULTS, ...(JSON.parse(raw) as Partial<Prefs>) } : { ...DEFAULTS };
  } catch {
    return { ...DEFAULTS };
  }
}

export const prefs = read();

export function savePrefs() {
  try {
    localStorage.setItem(KEY, JSON.stringify(prefs));
  } catch {
    // storage unavailable — prefs simply don't persist this session
  }
}
