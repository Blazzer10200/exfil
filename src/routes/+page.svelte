<script lang="ts">
  import { onMount } from "svelte";
  import { RotateCcw, Save, Cpu, CircleAlert } from "lucide-svelte";
  import Titlebar from "$lib/Titlebar.svelte";
  import SlotRail from "$lib/SlotRail.svelte";
  import Slider from "$lib/Slider.svelte";
  import {
    getStatus,
    getPresets,
    applyColor,
    selectPreset,
    savePreset,
    resetDisplay,
    type Preset,
    type ColorDials,
    type SystemStatus,
  } from "$lib/api";

  let presets = $state<Preset[]>([]);
  let active = $state("Normal");
  let dials = $state<ColorDials>({ gamma: 1.0, brightness: 0.0, contrast: 1.0 });
  let vibrance = $state(0);
  let status = $state<SystemStatus | null>(null);
  let busy = $state(false);
  let toast = $state<{ msg: string; kind: "ok" | "err" } | null>(null);

  const current = $derived(presets.find((p) => p.slot === active));
  const readOnly = $derived(active === "Normal");
  const vibranceMax = $derived(status?.vibrance?.max ?? 63);
  const dirty = $derived(
    !!current &&
      (current.dials.gamma !== dials.gamma ||
        current.dials.brightness !== dials.brightness ||
        current.dials.contrast !== dials.contrast ||
        current.vibrance !== vibrance),
  );

  function flash(msg: string, kind: "ok" | "err" = "ok") {
    toast = { msg, kind };
    setTimeout(() => (toast = null), 2200);
  }

  function loadInto(p: Preset) {
    dials = { ...p.dials };
    vibrance = p.vibrance;
  }

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
</script>

<div class="app">
  <Titlebar />

  <div class="body">
    <SlotRail {presets} {active} onselect={onSelect} />

    <main class="panel">
      <header class="hero">
        <div>
          <h1>{current?.name ?? "—"}</h1>
          <p class="sub">
            {#if readOnly}
              Baseline reference — read-only neutral profile
            {:else}
              Adjust color, gamma &amp; vibrance for this slot
            {/if}
          </p>
        </div>
        <div class="status">
          {#if status?.nvidia}
            <span class="chip ok"><Cpu size={13} /> NVIDIA</span>
          {:else}
            <span class="chip warn"><CircleAlert size={13} /> No NVAPI</span>
          {/if}
        </div>
      </header>

      <section class="controls card">
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
              min={0}
              max={vibranceMax}
              step={1}
              disabled={readOnly || !status?.nvidia}
              format={(v) => `${Math.round((v / vibranceMax) * 100)}%`}
              onchange={liveApply}
            />
            <div class="vibrance-note">
              {#if !status?.nvidia}
                Vibrance needs an NVIDIA GPU.
              {:else}
                Level {vibrance} / {vibranceMax} · driver-level, BattlEye-safe.
              {/if}
            </div>
          </div>
        </div>
      </section>

      <footer class="actions">
        <button class="btn danger" onclick={onReset} disabled={busy}>
          <RotateCcw size={14} /> Reset display
        </button>
        <div class="spacer"></div>
        {#if toast}
          <span class="toast" class:err={toast.kind === "err"}>{toast.msg}</span>
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
  .controls { padding: 20px 22px; }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 28px; }
  .col { display: flex; flex-direction: column; }
  .vibrance-note {
    margin-top: 2px;
    font-size: var(--fs-xs);
    color: var(--fg-subtle);
    line-height: 1.5;
  }
  .actions { display: flex; align-items: center; gap: 12px; margin-top: auto; }
  .spacer { flex: 1; }
  .toast {
    font-size: var(--fs-sm);
    color: var(--ok);
    padding: 4px 10px;
    border-radius: var(--radius-sm);
    background: var(--ok-soft);
  }
  .toast.err { color: var(--danger); background: var(--danger-soft); }
</style>
