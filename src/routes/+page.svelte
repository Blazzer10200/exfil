<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { RotateCcw, Save, Cpu, CircleAlert, Gamepad2, CheckCircle2, XCircle } from "lucide-svelte";
  import Titlebar from "$lib/Titlebar.svelte";
  import SlotRail from "$lib/SlotRail.svelte";
  import SettingsModal from "$lib/SettingsModal.svelte";
  import Slider from "$lib/Slider.svelte";
  import {
    getStatus,
    getPresets,
    applyColor,
    selectPreset,
    savePreset,
    resetDisplay,
    createPreset,
    deletePreset,
    renamePreset,
    setBinding,
    exportPresets,
    importPresets,
    type Preset,
    type ColorDials,
    type SystemStatus,
    type UpdateMeta,
  } from "$lib/api";

  let presets = $state<Preset[]>([]);
  // Rail instance — exposes openBindFor so the main-panel "Bind…" link can
  // open the binder for the active slot.
  let rail = $state<{ openBindFor: (slot: string) => Promise<void> } | undefined>();
  let active = $state("Normal");
  let dials = $state<ColorDials>({ gamma: 1.0, brightness: 0.0, contrast: 1.0 });
  let vibrance = $state(0);
  let status = $state<SystemStatus | null>(null);
  let busy = $state(false);
  let toast = $state<{ msg: string; kind: "ok" | "err" } | null>(null);
  let settingsOpen = $state(false);
  // Update the boot-time check found (if any) — Settings opens ready to install.
  let updateMeta = $state<UpdateMeta | null>(null);

  const current = $derived(presets.find((p) => p.slot === active));
  const readOnly = $derived(active === "Normal");
  const vibranceMax = $derived(status?.vibrance?.max ?? 100);
  const vibranceMin = $derived(status?.vibrance?.min ?? 0);
  // Preview saturation, normalized around the driver's DEFAULT level so the
  // neutral point previews identically across vendors (NVIDIA default 0,
  // AMD saturation default ~100): 0 at default, 1 at max, negative below.
  const vibranceNorm = $derived.by(() => {
    const info = status?.vibrance;
    if (!info) return 0;
    if (vibrance >= info.default)
      return info.max > info.default ? (vibrance - info.default) / (info.max - info.default) : 0;
    return info.default > info.min ? (vibrance - info.default) / (info.default - info.min) : 0;
  });
  const dirty = $derived(
    !!current &&
      (current.dials.gamma !== dials.gamma ||
        current.dials.brightness !== dials.brightness ||
        current.dials.contrast !== dials.contrast ||
        current.vibrance !== vibrance),
  );

  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  function flash(msg: string, kind: "ok" | "err" = "ok") {
    toast = { msg, kind };
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), 2200);
  }

  function loadInto(p: Preset) {
    dials = { ...p.dials };
    vibrance = p.vibrance;
  }

  let unlistenAuto: (() => void) | undefined;
  let unlistenUpdate: (() => void) | undefined;

  onMount(async () => {
    try {
      const store = await getPresets();
      presets = store.presets;
      active = store.active;
      status = await getStatus();
      const p = presets.find((x) => x.slot === active);
      if (p) loadInto(p);
    } catch (e) {
      flash(String(e), "err");
    }

    // The backend watcher auto-applies a bound preset when its program runs
    // (and reverts on exit). Re-sync the UI to whatever it switched to.
    unlistenAuto = await listen<Preset>("auto-switch", (e) => {
      const p = e.payload;
      const idx = presets.findIndex((x) => x.slot === p.slot);
      if (idx >= 0) presets[idx] = p;
      // The watcher re-announces the current slot (boot first-tick baseline,
      // hotkey re-pick of the active preset) — only a real slot change should
      // retune the dials + toast, or a no-op event clobbers unsaved edits.
      if (p.slot === active) return;
      active = p.slot;
      loadInto(p);
      flash(`Auto-switched to ${p.name}`);
    });

    // Backend checks GitHub for a newer signed build on boot; surface it as a
    // toast, and remember it so Settings → Updates opens ready to install.
    unlistenUpdate = await listen<UpdateMeta>("update-available", (e) => {
      updateMeta = e.payload;
      flash(`Update v${e.payload.version} available — install from Settings`);
    });
  });

  onMount(() => () => {
    unlistenAuto?.();
    unlistenUpdate?.();
  });

  async function onSelect(slot: string) {
    if (slot === active || busy) return;
    busy = true;
    try {
      const p = await selectPreset(slot);
      active = slot;
      loadInto(p);
      // keep the local store copy current
      const idx = presets.findIndex((x) => x.slot === slot);
      if (idx >= 0) presets[idx] = p;
    } catch (e) {
      flash(String(e), "err");
    } finally {
      busy = false;
    }
  }

  // Live-apply on slider drag (skip for read-only Normal baseline).
  let applyTimer: ReturnType<typeof setTimeout> | undefined;
  function liveApply() {
    if (readOnly) return;
    clearTimeout(applyTimer);
    applyTimer = setTimeout(async () => {
      try {
        await applyColor($state.snapshot(dials), vibrance);
      } catch (e) {
        flash(String(e), "err");
      }
    }, 40);
  }

  async function onSave() {
    if (!dirty || readOnly) return;
    try {
      await savePreset(active, $state.snapshot(dials), vibrance);
      const idx = presets.findIndex((x) => x.slot === active);
      if (idx >= 0)
        presets[idx] = {
          ...presets[idx],
          dials: { ...dials },
          vibrance,
        };
      flash(`Saved ${current?.name}`);
    } catch (e) {
      flash(String(e), "err");
    }
  }

  async function onReset() {
    busy = true;
    try {
      await resetDisplay();
      flash("Display reset to neutral");
    } catch (e) {
      flash(String(e), "err");
    } finally {
      busy = false;
    }
  }

  // ── Preset CRUD ──
  async function onCreate() {
    if (busy) return;
    busy = true;
    try {
      const p = await createPreset(""); // backend auto-names "Preset N"
      presets = [...presets, p];
      // Select it so the user tunes the fresh preset live straight away.
      await selectPreset(p.slot);
      active = p.slot;
      loadInto(p);
      flash(`Created ${p.name}`);
    } catch (e) {
      flash(String(e), "err");
    } finally {
      busy = false;
    }
  }

  // Create a preset directly from a running game: name it after the game's
  // window title and bind it to that exe, so it auto-applies when the game runs.
  async function onCreateFromGame(exe: string, title: string) {
    if (busy) return;
    busy = true;
    try {
      const name = title.trim() || exe;
      const p = await createPreset(name);
      const store = await setBinding(p.slot, exe);
      presets = store.presets;
      await selectPreset(p.slot);
      active = p.slot;
      const fresh = presets.find((x) => x.slot === p.slot) ?? p;
      loadInto(fresh);
      flash(`Created ${name} · bound ${exe}`);
    } catch (e) {
      flash(String(e), "err");
    } finally {
      busy = false;
    }
  }

  async function onDelete(slot: string) {
    if (busy || slot === "Normal") return;
    busy = true;
    try {
      const wasActive = slot === active;
      const store = await deletePreset(slot);
      presets = store.presets;
      if (wasActive) {
        // Deleting the active slot falls back to Normal → re-apply it. Deleting
        // any other slot leaves the current selection (and unsaved edits) alone.
        active = store.active;
        const p = presets.find((x) => x.slot === active);
        if (p) {
          await selectPreset(active);
          loadInto(p);
        }
      }
      flash("Preset deleted");
    } catch (e) {
      flash(String(e), "err");
    } finally {
      busy = false;
    }
  }

  async function onRename(slot: string, name: string) {
    try {
      await renamePreset(slot, name);
      const idx = presets.findIndex((x) => x.slot === slot);
      if (idx >= 0) presets[idx] = { ...presets[idx], name };
    } catch (e) {
      flash(String(e), "err");
    }
  }

  // Bind/unbind a program to a slot. Returns the fresh store so binding badges
  // (and any cleared same-exe binding on another slot) re-sync.
  async function onBind(slot: string, exe: string | null) {
    try {
      const store = await setBinding(slot, exe);
      presets = store.presets;
      flash(exe ? `Bound ${exe}` : "Unbound");
    } catch (e) {
      flash(String(e), "err");
    }
  }

  // Export user presets to a JSON file; import appends presets from one.
  async function onExport() {
    try {
      const path = await saveDialog({
        defaultPath: "exfil-presets.json",
        filters: [{ name: "EXFIL presets", extensions: ["json"] }],
      });
      if (!path) return;
      await exportPresets(path);
      flash("Presets exported");
    } catch (e) {
      flash(String(e), "err");
    }
  }

  async function onImport() {
    try {
      const picked = await openDialog({
        multiple: false,
        directory: false,
        filters: [{ name: "EXFIL presets", extensions: ["json"] }],
      });
      if (typeof picked !== "string") return;
      const store = await importPresets(picked);
      presets = store.presets;
      active = store.active;
      flash("Presets imported");
    } catch (e) {
      flash(String(e), "err");
    }
  }
</script>

<div class="app">
  <Titlebar onsettings={() => (settingsOpen = true)} />

  <div class="body">
    <SlotRail
      bind:this={rail}
      {presets}
      {active}
      {dirty}
      onselect={onSelect}
      oncreate={onCreate}
      oncreategame={onCreateFromGame}
      ondelete={onDelete}
      onrename={onRename}
      onbind={onBind}
      onerror={(msg) => flash(msg, "err")}
    />

    <main class="panel">
      {#key active}
        <header class="hero hero-anim">
          <div>
            <h1>{current?.name ?? "—"}</h1>
            <p class="sub">
              {#if readOnly}
                Baseline reference — read-only neutral profile
              {:else}
                Adjust color, gamma &amp; vibrance for this preset
              {/if}
            </p>
          </div>
          <div class="status">
            {#if status?.vendor}
              <span class="chip ok"><Cpu size={13} /> {status.vendor === "amd" ? "AMD" : "NVIDIA"}</span>
            {:else}
              <span class="chip warn"><CircleAlert size={13} /> Gamma only</span>
            {/if}
          </div>
        </header>
      {/key}

      <section class="controls card">
        <div class="preview" style="--p-brightness: {dials.brightness}; --p-contrast: {dials.contrast}; --p-vibrance: {vibranceNorm};">
          <div class="preview-stage">
            <div class="orb"></div>
            <div class="chips">
              {#each ["#ff4757", "#ffa502", "#2ed573", "#1e90ff", "#a55eea"] as c}
                <span class="chip-color" style="--c: {c}"></span>
              {/each}
            </div>
          </div>
          <div class="ramp"></div>
          <div class="preview-legend">Live preview — reflects the sliders below, not the exact on-screen color</div>
        </div>
        <div class="grid">
          <div class="col">
            <Slider
              label="Gamma"
              bind:value={dials.gamma}
              min={0.3}
              max={2.8}
              step={0.01}
              disabled={readOnly}
              format={(v) => v.toFixed(2)}
              onchange={liveApply}
            />
            <Slider
              label="Brightness"
              bind:value={dials.brightness}
              min={-0.5}
              max={0.5}
              step={0.01}
              disabled={readOnly}
              format={(v) => (v > 0 ? "+" : "") + v.toFixed(2)}
              onchange={liveApply}
            />
            <Slider
              label="Contrast"
              bind:value={dials.contrast}
              min={0.5}
              max={2.0}
              step={0.01}
              disabled={readOnly}
              format={(v) => v.toFixed(2)}
              onchange={liveApply}
            />
          </div>
          <div class="col">
            <Slider
              label="Digital Vibrance"
              bind:value={vibrance}
              min={vibranceMin}
              max={vibranceMax}
              step={1}
              disabled={readOnly || !status?.vendor}
              format={(v) => `${Math.round((v / vibranceMax) * 100)}%`}
              onchange={liveApply}
            />
            <div class="vibrance-note">
              {#if !status?.vendor}
                Vibrance needs an NVIDIA or AMD GPU — gamma still works.
              {:else}
                Level {vibrance} / {vibranceMax} · driver-level, anti-cheat safe.
              {/if}
            </div>
            {#if !readOnly}
              <div class="autoswitch" class:bound={!!current?.exe}>
                <Gamepad2 size={13} />
                {#if current?.exe}
                  <span>Auto-applies while <strong>{current.exe}</strong> runs</span>
                  <button class="link" onclick={() => onBind(active, null)}>Unbind</button>
                {:else}
                  <span>Not bound to a game</span>
                  <button class="link" onclick={() => rail?.openBindFor(active)}>Bind…</button>
                {/if}
              </div>
            {/if}
          </div>
        </div>
      </section>

      <footer class="actions">
        <button class="btn danger" onclick={onReset} disabled={busy}>
          <RotateCcw size={14} /> Reset display
        </button>
        <div class="spacer"></div>
        {#if toast}
          <span class="toast" class:err={toast.kind === "err"}>
            {#if toast.kind === "err"}<XCircle size={13} />{:else}<CheckCircle2 size={13} />{/if}
            {toast.msg}
          </span>
        {/if}
        <button
          class="btn primary"
          onclick={onSave}
          disabled={!dirty || readOnly}
        >
          <Save size={14} /> {dirty ? "Save changes" : "Saved"}
        </button>
      </footer>
    </main>
  </div>

  {#if settingsOpen}
    <SettingsModal
      knownUpdate={updateMeta}
      onclose={() => (settingsOpen = false)}
      onimport={() => {
        settingsOpen = false;
        onImport();
      }}
      onexport={() => {
        settingsOpen = false;
        onExport();
      }}
      onerror={(msg) => flash(msg, "err")}
    />
  {/if}
</div>

<style>
  .app {
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .body { flex: 1; display: flex; min-height: 0; }
  .panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: 22px 26px;
    gap: 18px;
    min-width: 0;
  }
  .hero {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
  }
  .hero-anim { animation: hero-in 180ms var(--ease-soft, ease); }
  @keyframes hero-in {
    from { opacity: 0; transform: translateY(3px); }
    to { opacity: 1; transform: translateY(0); }
  }
  h1 {
    margin: 0;
    font-size: var(--fs-hero);
    font-weight: 650;
    letter-spacing: -0.02em;
    color: var(--fg);
  }
  .sub { margin: 4px 0 0; font-size: var(--fs-md); color: var(--fg-muted); }
  .status { flex-shrink: 0; }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 9px;
    border-radius: 999px;
    font-size: var(--fs-xs);
    font-weight: 500;
    border: 1px solid var(--border);
  }
  .chip.ok { color: var(--ok); background: var(--ok-soft); border-color: transparent; }
  .chip.warn { color: var(--warn); background: var(--warn-soft); border-color: transparent; }
  .controls { padding: 20px 22px; display: flex; flex-direction: column; gap: 20px; }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 28px; }
  .col { display: flex; flex-direction: column; }

  .preview {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 18px 20px 16px;
    border-radius: var(--radius-lg, 12px);
    background:
      radial-gradient(120% 160% at 18% 0%, oklch(0.24 0.03 270 / 0.5), transparent 55%),
      linear-gradient(160deg, oklch(0.16 0.005 255), oklch(0.105 0.004 255));
    border: 1px solid var(--border);
    box-shadow: inset 0 1px 0 oklch(1 0 0 / 0.03), inset 0 0 40px oklch(0 0 0 / 0.35);
  }
  .preview-stage {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    padding: 10px 4px 4px;
  }
  .orb {
    --orb-base: oklch(0.62 0.18 250);
    width: 88px;
    height: 88px;
    flex-shrink: 0;
    border-radius: 50%;
    background:
      radial-gradient(
        circle at 32% 28%,
        oklch(0.95 0.06 95 / 0.9) 0%,
        color-mix(in oklab, var(--orb-base) 70%, white 30%) 14%,
        var(--orb-base) 46%,
        color-mix(in oklab, var(--orb-base) 70%, black 45%) 82%,
        oklch(0.1 0.01 255) 100%
      );
    box-shadow:
      0 10px 24px -6px color-mix(in oklab, var(--orb-base) 55%, transparent),
      inset 0 -10px 18px -4px oklch(0 0 0 / 0.55);
    filter:
      brightness(calc(1 + var(--p-brightness, 0)))
      contrast(var(--p-contrast, 1))
      saturate(max(0, calc(1 + var(--p-vibrance, 0) * 1.6)));
    transition: filter 60ms linear;
  }
  .chips {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 8px;
    flex: 1;
  }
  .chip-color {
    aspect-ratio: 1;
    border-radius: var(--radius-sm);
    background:
      linear-gradient(160deg, color-mix(in oklab, var(--c) 85%, white 15%), var(--c) 55%, color-mix(in oklab, var(--c) 75%, black 30%));
    box-shadow: inset 0 1px 0 oklch(1 0 0 / 0.18), 0 2px 8px -2px color-mix(in oklab, var(--c) 50%, transparent);
    filter:
      brightness(calc(1 + var(--p-brightness, 0)))
      contrast(var(--p-contrast, 1))
      saturate(max(0, calc(1 + var(--p-vibrance, 0) * 1.6)));
    transition: filter 60ms linear, transform 140ms var(--ease-soft, ease);
  }
  .ramp {
    height: 14px;
    border-radius: 999px;
    background: linear-gradient(90deg, #050505, #ffffff);
    box-shadow: inset 0 1px 2px oklch(0 0 0 / 0.6);
    filter:
      brightness(calc(1 + var(--p-brightness, 0)))
      contrast(var(--p-contrast, 1));
    transition: filter 60ms linear;
  }

  .autoswitch {
    margin-top: 16px;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 9px 11px;
    border-radius: var(--radius);
    border: 1px dashed var(--border-strong);
    color: var(--fg-subtle);
    font-size: var(--fs-xs);
  }
  .autoswitch.bound {
    border-style: solid;
    border-color: color-mix(in oklab, var(--accent) 30%, transparent);
    background: color-mix(in oklab, var(--accent) 7%, transparent);
    color: var(--fg-muted);
  }
  .autoswitch :global(svg) { flex-shrink: 0; opacity: 0.8; }
  .autoswitch span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .autoswitch strong { color: var(--fg-2); font-weight: 600; }
  .autoswitch .link {
    margin-left: auto;
    flex-shrink: 0;
    padding: 0;
    border: none;
    background: none;
    color: var(--accent);
    font: inherit;
    font-size: var(--fs-xs);
    cursor: pointer;
    opacity: 0.9;
  }
  .autoswitch .link:hover { opacity: 1; text-decoration: underline; }
  .autoswitch .link:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--ring);
    border-radius: 3px;
  }
  .vibrance-note {
    margin-top: 2px;
    font-size: var(--fs-xs);
    color: var(--fg-subtle);
    line-height: 1.5;
  }
  .actions { display: flex; align-items: center; gap: 12px; margin-top: auto; }
  .spacer { flex: 1; }
  .toast {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: var(--fs-sm);
    color: var(--ok);
    padding: 4px 10px;
    border-radius: var(--radius-sm);
    background: var(--ok-soft);
    animation: toast-in 160ms var(--ease-soft, ease);
  }
  .toast.err { color: var(--danger); background: var(--danger-soft); }
  @keyframes toast-in {
    from { opacity: 0; transform: translateY(2px) scale(0.97); }
    to { opacity: 1; transform: translateY(0) scale(1); }
  }
  .preview-legend {
    font-size: 10px;
    color: var(--fg-faint);
    letter-spacing: 0.01em;
    text-align: center;
  }
</style>
