// App-wide state: presets, crosshairs, system status, foreground program,
// update metadata. Views read from `app` and call its actions; the backend
// stays the source of truth (every mutation re-syncs from the returned store).

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  applyColor,
  checkUpdate,
  createCrosshair,
  createPreset,
  deleteCrosshair,
  deletePreset,
  getCrosshairs,
  getInFront,
  getPresets,
  getStatus,
  NO_CROSSHAIR,
  primaryMonitor,
  renameCrosshair,
  renamePreset,
  resetDisplay,
  savePreset,
  selectCrosshair,
  selectPreset,
  setBinding,
  setCrosshairBinding,
  setCrosshairEnabled,
  updateCrosshair,
  type ColorDials,
  type Crosshair,
  type CrosshairStyle,
  type MonitorInfo,
  type Preset,
  type SystemStatus,
  type UpdateMeta,
} from "./api";
import { prefs, savePrefs, type View } from "./prefs";
import { toast, reason } from "./toast.svelte";

export type Binding = { exe: string; preset: Preset | null; crosshair: Crosshair | null };

class AppStore {
  // ── color ──
  presets = $state<Preset[]>([]);
  active = $state("Normal");
  dials = $state<ColorDials>({ gamma: 1, brightness: 0, contrast: 1 });
  vibrance = $state(0);
  status = $state<SystemStatus | null>(null);
  busy = $state(false);

  // ── crosshairs ──
  crosshairs = $state<Crosshair[]>([]);
  selected = $state<string>(NO_CROSSHAIR);
  overlayOn = $state(false);

  // ── system ──
  inFront = $state<string | null>(null);
  monitor = $state<MonitorInfo | null>(null);
  updateMeta = $state<UpdateMeta | null>(null);
  hotkeys = $state(true);
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
  selectedCrosshair = $derived(this.crosshairs.find((c) => c.id === this.selected) ?? null);
  /** Per-program bindings, joined by exe across presets + crosshairs. */
  bindings = $derived.by((): Binding[] => {
    const map = new Map<string, Binding>();
    for (const p of this.presets)
      if (p.exe) map.set(p.exe, { exe: p.exe, preset: p, crosshair: null });
    for (const c of this.crosshairs)
      if (c.exe) {
        const b = map.get(c.exe) ?? { exe: c.exe, preset: null, crosshair: null };
        b.crosshair = c;
        map.set(c.exe, b);
      }
    return [...map.values()].sort((a, b) => a.exe.localeCompare(b.exe));
  });
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
  }
  #syncCrosshairs(s: { crosshairs: Crosshair[]; selected: string; enabled: boolean }) {
    this.crosshairs = s.crosshairs;
    this.selected = s.selected;
    this.overlayOn = s.enabled;
  }

  async boot() {
    try {
      const store = await getPresets();
      this.presets = store.presets;
      this.active = store.active;
      this.hotkeys = store.hotkeys;
      const p = this.presets.find((x) => x.slot === this.active);
      if (p) this.#loadInto(p);
      this.status = await getStatus();
      this.#syncCrosshairs(await getCrosshairs());
      this.inFront = await getInFront();
      this.monitor = await primaryMonitor();
    } catch (e) {
      toast.error(`✕ LOAD FAILED · ${reason(e)}`);
    }
    // First run lands on Games (nothing bound yet); afterwards, the last view.
    this.view = prefs.view ?? (this.bindings.length ? "color" : "games");

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
        const x = this.selectedCrosshair?.name.toUpperCase() ?? "NONE";
        toast.info(`⇄ AUTO-SWITCHED → ${p.name.toUpperCase()} · ${x}`);
      }),
      await listen<UpdateMeta>("update-available", (e) => {
        this.updateMeta = e.payload;
        toast.info(`⇡ UPDATE READY · v${e.payload.version} · SETTINGS → ABOUT`);
      }),
      await listen<string | null>("in-front", (e) => {
        this.inFront = e.payload;
      }),
      await listen<boolean>("crosshair-toggled", (e) => {
        this.overlayOn = e.payload;
      }),
      await listen<string>("crosshair-selected", (e) => {
        this.selected = e.payload;
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
      await selectPreset(p.slot);
      this.active = p.slot;
      this.#loadInto(p);
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
      await selectPreset(p.slot);
      this.active = p.slot;
      this.#loadInto(this.presets.find((x) => x.slot === p.slot) ?? p);
      toast.ok(`✓ BOUND · ${exe.toUpperCase()}`);
    } catch (e) {
      toast.error(`✕ BIND FAILED · ${reason(e)}`);
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
      await selectPreset(p.slot);
      this.active = p.slot;
      this.#loadInto(dup);
      toast.ok(`✓ DUPLICATED · ${dup.name.toUpperCase()}`);
    } catch (e) {
      toast.error(`✕ DUPLICATE FAILED · ${reason(e)}`);
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

  async bindPreset(slot: string, exe: string | null) {
    try {
      const store = await setBinding(slot, exe);
      this.presets = store.presets;
      toast.ok(exe ? `✓ BOUND · ${exe.toUpperCase()}` : "✓ UNBOUND");
    } catch (e) {
      toast.error(`✕ BIND FAILED · ${reason(e)}`);
    }
  }

  // ── crosshair actions ──
  async pickCrosshair(id: string) {
    if (id === this.selected) return;
    const prev = this.selected;
    this.selected = id;
    try {
      await selectCrosshair(id);
    } catch (e) {
      this.selected = prev;
      toast.error(`✕ SELECT FAILED · ${reason(e)}`);
    }
  }

  async createCrosshair(name: string, style: CrosshairStyle | null, exe: string | null = null) {
    // "From running program" on a game that already has a crosshair: open
    // that one. Minting a fresh default and stealing the binding would leave
    // the tuned crosshair orphaned and a stock one showing in the game.
    const bound = exe ? this.crosshairs.find((c) => c.exe === exe) : undefined;
    if (bound) {
      await this.pickCrosshair(bound.id);
      toast.info(`↳ ALREADY BOUND · ${bound.name.toUpperCase()}`);
      return bound;
    }
    try {
      let c = await createCrosshair(name, style);
      if (exe) {
        const store = await setCrosshairBinding(c.id, exe);
        this.#syncCrosshairs(store);
        c = this.crosshairs.find((x) => x.id === c.id) ?? c;
      } else {
        this.crosshairs = [...this.crosshairs, c];
      }
      await this.pickCrosshair(c.id);
      return c;
    } catch (e) {
      toast.error(`✕ CREATE FAILED · ${reason(e)}`);
      return null;
    }
  }

  #pending = new Map<string, CrosshairStyle>();
  #saveTimer: ReturnType<typeof setTimeout> | undefined;
  /** Style edits: update in place now, persist debounced (150ms). */
  editCrosshair(id: string, style: CrosshairStyle) {
    const idx = this.crosshairs.findIndex((c) => c.id === id);
    if (idx < 0) return;
    this.crosshairs[idx] = { ...this.crosshairs[idx], style };
    this.#pending.set(id, $state.snapshot(style));
    clearTimeout(this.#saveTimer);
    this.#saveTimer = setTimeout(() => void this.flushCrosshairs(), 150);
  }
  async flushCrosshairs() {
    clearTimeout(this.#saveTimer);
    const batch = [...this.#pending];
    this.#pending.clear();
    for (const [id, style] of batch) {
      try {
        await updateCrosshair(id, style);
      } catch (e) {
        toast.error(`✕ SAVE FAILED · ${reason(e)}`, {
          label: "RETRY",
          run: () => this.editCrosshair(id, style),
        });
      }
    }
  }

  async renameCrosshair(id: string, name: string) {
    try {
      await renameCrosshair(id, name);
      const idx = this.crosshairs.findIndex((c) => c.id === id);
      if (idx >= 0) this.crosshairs[idx] = { ...this.crosshairs[idx], name };
      toast.ok(`✓ RENAMED · ${name.toUpperCase()}`);
    } catch (e) {
      toast.error(`✕ RENAME FAILED · ${reason(e)}`);
    }
  }

  async duplicateCrosshair(id: string) {
    const src = this.crosshairs.find((c) => c.id === id);
    if (!src) return;
    const c = await this.createCrosshair(`${src.name} copy`, $state.snapshot(src.style));
    if (c) toast.ok(`✓ DUPLICATED · ${c.name.toUpperCase()}`);
  }

  async deleteCrosshair(id: string, opts: { silent?: boolean } = {}) {
    try {
      this.#syncCrosshairs(await deleteCrosshair(id));
      if (!opts.silent) toast.ok("✓ CROSSHAIR DELETED");
    } catch (e) {
      toast.error(`✕ DELETE FAILED · ${reason(e)}`);
    }
  }

  async bindCrosshair(id: string, exe: string | null) {
    try {
      this.#syncCrosshairs(await setCrosshairBinding(id, exe));
      toast.ok(exe ? `✓ BOUND · ${exe.toUpperCase()}` : "✓ UNBOUND");
    } catch (e) {
      toast.error(`✕ BIND FAILED · ${reason(e)}`);
    }
  }

  async setOverlay(on: boolean) {
    try {
      this.overlayOn = await setCrosshairEnabled(on);
    } catch (e) {
      toast.error(`✕ OVERLAY · ${reason(e)}`);
    }
  }

  async refreshUpdate() {
    try {
      this.updateMeta = await checkUpdate();
      return this.updateMeta;
    } catch (e) {
      toast.error(`✕ UPDATE CHECK FAILED · ${reason(e)}`);
      return null;
    }
  }
}

export const app = new AppStore();
