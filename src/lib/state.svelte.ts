// App-wide state: presets, system status, foreground program,
// update metadata. Views read from `app` and call its actions; the backend
// stays the source of truth (every mutation re-syncs from the returned store).

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  applyColor,
  checkUpdate,
  countShades,
  createPreset,
  deletePreset,
  getInFront,
  getPresets,
  getStatus,
  installUpdate,
  primaryMonitor,
  renamePreset,
  resetDisplay,
  savePreset,
  selectPreset,
  setBinding,
  type ColorDials,
  type MonitorInfo,
  type Preset,
  type SystemStatus,
  type UpdateMeta,
} from "./api";
import { prefs, savePrefs, type View } from "./prefs";
import { toast, reason } from "./toast.svelte";

/** A program bound to a preset (a preset with an `exe`). */
export type Binding = { exe: string; preset: Preset };

/** Where the Settings → About update row is: idle, checking, "up to date", installing. */
export type UpdatePhase = "idle" | "checking" | "none" | "installing";


class AppStore {
  // ── color ──
  presets = $state<Preset[]>([]);
  active = $state("Normal");
  dials = $state<ColorDials>({ gamma: 1, brightness: 0, contrast: 1 });
  vibrance = $state(0);
  status = $state<SystemStatus | null>(null);
  busy = $state(false);

  // ── system ──
  inFront = $state<string | null>(null);
  monitor = $state<MonitorInfo | null>(null);
  updateMeta = $state<UpdateMeta | null>(null);
  // Lives here, not in SettingsView: views remount on every tab switch, and a
  // download in flight must survive leaving Settings.
  updatePhase = $state<UpdatePhase>("idle");
  updateProgress = $state(0);
  hotkeys = $state(true);
  /** Driver dithering is switched on (Settings → General). */
  dither = $state(false);
  /** Shades (of 256) the current dials keep on an 8-bit signal. */
  shades = $state(256);
  view = $state<View>("games");
  grid = $state(prefs.grid);

  current = $derived(this.presets.find((p) => p.slot === this.active));
  readOnly = $derived(this.active === "Normal");
  vendor = $derived(this.status?.vendor ?? null);
  vibranceMax = $derived(this.status?.vibrance?.max ?? 100);
  vibranceMin = $derived(this.status?.vibrance?.min ?? 0);
  // Preview saturation normalized around the driver DEFAULT: 0 at default,
  // 1 at max, negative below — so neutral previews the same on every vendor.
  vibranceNorm = $derived.by(() => {
    const info = this.status?.vibrance;
    if (!info) return 0;
    if (this.vibrance >= info.default)
      return info.max > info.default ? (this.vibrance - info.default) / (info.max - info.default) : 0;
    return info.default > info.min ? (this.vibrance - info.default) / (info.default - info.min) : 0;
  });
  dirty = $derived(
    !!this.current &&
      (this.current.dials.gamma !== this.dials.gamma ||
        this.current.dials.brightness !== this.dials.brightness ||
        this.current.dials.contrast !== this.dials.contrast ||
        this.current.vibrance !== this.vibrance),
  );
  /** Per-program bindings: every preset that has a program bound, by exe. */
  bindings = $derived.by((): Binding[] =>
    this.presets
      .flatMap((p) => (p.exe ? [{ exe: p.exe, preset: p }] : []))
      .sort((a, b) => a.exe.localeCompare(b.exe)),
  );
  /** A bound program is in front (drives the Games badge + IN FRONT readout). */
  inFrontBound = $derived(!!this.inFront && this.bindings.some((b) => b.exe === this.inFront));

  #unlisten: UnlistenFn[] = [];

  setView(v: View) {
    this.view = v;
    prefs.view = v;
    savePrefs();
  }
  setGrid(on: boolean) {
    this.grid = on;
    prefs.grid = on;
    savePrefs();
  }

  #loadInto(p: Preset) {
    this.dials = { ...p.dials };
    this.vibrance = p.vibrance;
    void this.#refreshShades();
  }
  async #refreshShades() {
    try {
      this.shades = await countShades($state.snapshot(this.dials));
    } catch (e) {
      toast.error(`✕ SHADES READOUT · ${reason(e)}`);
    }
  }
  /** Make `p` the active preset on the display and in the UI. */
  async #activate(p: Preset) {
    await selectPreset(p.slot);
    this.active = p.slot;
    this.#loadInto(p);
  }
  /** Re-read the preset list after a multi-step action failed part-way, so
   *  whatever the backend did create shows up instead of waiting for a restart. */
  async #resync() {
    try {
      this.presets = (await getPresets()).presets;
    } catch {
      // the original failure is already on screen
    }
  }
  /** Name of the preset a program is bound to, if any. */
  boundTo = (exe: string) => this.bindings.find((b) => b.exe === exe)?.preset.name ?? null;

  async boot() {
    try {
      const store = await getPresets();
      this.presets = store.presets;
      this.active = store.active;
      this.hotkeys = store.hotkeys;
      this.dither = store.dither;
      const p = this.presets.find((x) => x.slot === this.active);
      if (p) this.#loadInto(p);
      this.status = await getStatus();
      this.inFront = await getInFront();
      this.monitor = await primaryMonitor();
    } catch (e) {
      toast.error(`✕ LOAD FAILED · ${reason(e)}`);
    }
    // First run lands on Games (nothing bound yet); afterwards, the last view.
    this.view = prefs.view ?? (this.bindings.length ? "color" : "games");

    try {
      await this.#listen();
    } catch (e) {
      toast.error(`✕ EVENTS UNAVAILABLE · ${reason(e)}`);
    }
  }

  async #listen() {
    // Watcher auto-applied a bound preset (or reverted on exit). The watcher
    // also re-announces the current slot (boot baseline, hotkey re-pick) —
    // only a real slot change should retune the dials, or a no-op event
    // clobbers unsaved edits.
    this.#unlisten.push(
      await listen<Preset>("auto-switch", (e) => {
        const p = e.payload;
        const idx = this.presets.findIndex((x) => x.slot === p.slot);
        if (idx >= 0) this.presets[idx] = p;
        if (p.slot === this.active) return;
        this.active = p.slot;
        this.#loadInto(p);
        toast.info(`⇄ AUTO-SWITCHED → ${p.name.toUpperCase()}`);
      }),
      await listen<UpdateMeta>("update-available", (e) => {
        this.updateMeta = e.payload;
        toast.info(`⇡ UPDATE READY · v${e.payload.version} · SETTINGS → ABOUT`);
      }),
      await listen<string | null>("in-front", (e) => {
        this.inFront = e.payload;
      }),
      await listen<number>("update-progress", (e) => {
        this.updateProgress = e.payload;
        toast.info(`UPDATING · ${e.payload}%`, { spin: true, sticky: true });
      }),
    );
  }
  dispose() {
    for (const u of this.#unlisten) u();
    this.#unlisten = [];
  }

  // ── color actions ──
  async pickPreset(slot: string) {
    if (slot === this.active || this.busy) return;
    this.busy = true;
    try {
      const p = await selectPreset(slot);
      this.active = slot;
      this.#loadInto(p);
      const idx = this.presets.findIndex((x) => x.slot === slot);
      if (idx >= 0) this.presets[idx] = p;
    } catch (e) {
      toast.error(`✕ APPLY FAILED · ${reason(e)}`);
    } finally {
      this.busy = false;
    }
  }

  #applyTimer: ReturnType<typeof setTimeout> | undefined;
  /** Live-apply on dial drag (40ms coalesce); no-op for Normal. */
  liveApply() {
    if (this.readOnly) return;
    clearTimeout(this.#applyTimer);
    this.#applyTimer = setTimeout(async () => {
      try {
        await applyColor($state.snapshot(this.dials), this.vibrance);
        await this.#refreshShades();
      } catch (e) {
        toast.error(`✕ APPLY FAILED · ${reason(e)}`);
      }
    }, 40);
  }

  async save() {
    if (!this.dirty || this.readOnly) return;
    try {
      await savePreset(this.active, $state.snapshot(this.dials), this.vibrance);
      const idx = this.presets.findIndex((x) => x.slot === this.active);
      if (idx >= 0)
        this.presets[idx] = { ...this.presets[idx], dials: { ...this.dials }, vibrance: this.vibrance };
      toast.ok(`✓ SAVED · ${this.current?.name.toUpperCase()}`);
    } catch (e) {
      toast.error(`✕ SAVE FAILED · ${reason(e)}`, { label: "RETRY", run: () => void this.save() });
    }
  }

  /** RESET = back to the saved values of this preset (re-applied live). */
  revert() {
    if (!this.current || this.readOnly) return;
    this.#loadInto(this.current);
    this.liveApply();
  }

  async resetDisplay() {
    this.busy = true;
    try {
      await resetDisplay();
      toast.ok("✓ DISPLAY RESET · NATIVE");
    } catch (e) {
      toast.error(`✕ RESET FAILED · ${reason(e)}`);
    } finally {
      this.busy = false;
    }
  }

  async createPreset(name = "") {
    if (this.busy) return null;
    this.busy = true;
    try {
      const p = await createPreset(name); // "" → backend auto-names "Preset N"
      this.presets = [...this.presets, p];
      await this.#activate(p);
      return p;
    } catch (e) {
      toast.error(`✕ CREATE FAILED · ${reason(e)}`);
      return null;
    } finally {
      this.busy = false;
    }
  }

  /** New preset named after a running game, bound to it in one action. */
  async createPresetFromGame(exe: string, title: string) {
    if (this.busy) return;
    const bound = this.presets.find((p) => p.exe === exe);
    if (bound) {
      await this.pickPreset(bound.slot);
      toast.info(`↳ ALREADY BOUND · ${bound.name.toUpperCase()}`);
      return;
    }
    this.busy = true;
    try {
      const name = title.trim() || exe;
      const p = await createPreset(name);
      const store = await setBinding(p.slot, exe);
      this.presets = store.presets;
      await this.#activate(this.presets.find((x) => x.slot === p.slot) ?? p);
      toast.ok(`✓ BOUND · ${exe.toUpperCase()}`);
    } catch (e) {
      toast.error(`✕ BIND FAILED · ${reason(e)}`);
      await this.#resync();
    } finally {
      this.busy = false;
    }
  }

  async duplicatePreset(slot: string) {
    const src = this.presets.find((p) => p.slot === slot);
    if (!src || this.busy) return;
    this.busy = true;
    try {
      const p = await createPreset(`${src.name} copy`);
      await savePreset(p.slot, { ...src.dials }, src.vibrance);
      const dup = { ...p, dials: { ...src.dials }, vibrance: src.vibrance };
      this.presets = [...this.presets, dup];
      await this.#activate(dup);
      toast.ok(`✓ DUPLICATED · ${dup.name.toUpperCase()}`);
    } catch (e) {
      toast.error(`✕ DUPLICATE FAILED · ${reason(e)}`);
      await this.#resync();
    } finally {
      this.busy = false;
    }
  }

  async deletePreset(slot: string) {
    if (this.busy || slot === "Normal") return;
    this.busy = true;
    try {
      const wasActive = slot === this.active;
      const store = await deletePreset(slot);
      this.presets = store.presets;
      if (wasActive) {
        this.active = store.active;
        const p = this.presets.find((x) => x.slot === this.active);
        if (p) {
          await selectPreset(this.active);
          this.#loadInto(p);
        }
      }
      toast.ok("✓ PRESET DELETED");
    } catch (e) {
      toast.error(`✕ DELETE FAILED · ${reason(e)}`);
    } finally {
      this.busy = false;
    }
  }

  async renamePreset(slot: string, name: string) {
    try {
      await renamePreset(slot, name);
      const idx = this.presets.findIndex((x) => x.slot === slot);
      if (idx >= 0) this.presets[idx] = { ...this.presets[idx], name };
      toast.ok(`✓ RENAMED · ${name.toUpperCase()}`);
    } catch (e) {
      toast.error(`✕ RENAME FAILED · ${reason(e)}`);
    }
  }

  /** Bind (or, with `exe = null`, unbind) a program. Resolves to whether it worked. */
  async bindPreset(slot: string, exe: string | null) {
    try {
      const store = await setBinding(slot, exe);
      this.presets = store.presets;
      toast.ok(exe ? `✓ BOUND · ${exe.toUpperCase()}` : "✓ UNBOUND");
      return true;
    } catch (e) {
      toast.error(`✕ BIND FAILED · ${reason(e)}`);
      return false;
    }
  }

  // ── updates ──
  #upToDateTimer: ReturnType<typeof setTimeout> | undefined;
  async checkForUpdate() {
    if (this.updatePhase !== "idle" && this.updatePhase !== "none") return;
    clearTimeout(this.#upToDateTimer);
    this.updatePhase = "checking";
    try {
      this.updateMeta = await checkUpdate();
    } catch (e) {
      // A failed check is not "up to date".
      this.updatePhase = "idle";
      toast.error(`✕ UPDATE CHECK FAILED · ${reason(e)}`);
      return;
    }
    if (this.updateMeta) {
      this.updatePhase = "idle";
      return;
    }
    this.updatePhase = "none";
    this.#upToDateTimer = setTimeout(() => {
      if (this.updatePhase === "none") this.updatePhase = "idle";
    }, 3000);
  }

  async installUpdate() {
    if (this.updatePhase === "installing") return;
    this.updatePhase = "installing";
    this.updateProgress = 0;
    try {
      await installUpdate(); // only settles on failure — success relaunches
    } catch (e) {
      this.updatePhase = "idle";
      toast.error(`✕ UPDATE FAILED · ${reason(e)}`, { label: "RETRY", run: () => void this.installUpdate() });
    }
  }
}

export const app = new AppStore();
