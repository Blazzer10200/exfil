// UI-only preferences persisted in localStorage (never display state).

export const VIEWS = ["color", "games", "settings"] as const;
export type View = (typeof VIEWS)[number];

const KEY = "exfil.prefs";

type Prefs = {
  view: View | null;
  grid: boolean;
};

const DEFAULTS: Prefs = { view: null, grid: true };

function read(): Prefs {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return { ...DEFAULTS };
    // Read field by field: a stored value from an older build (a view that no
    // longer exists, a non-boolean) falls back to its default instead of
    // leaking into the app.
    const saved = JSON.parse(raw) as Record<string, unknown>;
    return {
      view: VIEWS.find((v) => v === saved.view) ?? DEFAULTS.view,
      grid: typeof saved.grid === "boolean" ? saved.grid : DEFAULTS.grid,
    };
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
