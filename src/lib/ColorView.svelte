<script lang="ts">
  // COLOR: preset strip · live scene preview · four dial cards · bind · save.
  import { ChevronDown, Lock, Pencil, Copy, Trash2, Link2 } from "lucide-svelte";
  import { renderCrosshair, slotAccent } from "./api";
  import { app } from "./state.svelte";
  import Strip, { type StripItem } from "./Strip.svelte";
  import Dial from "./Dial.svelte";
  import InlineRename from "./InlineRename.svelte";
  import ArmChip from "./ArmChip.svelte";
  import ContextMenu, { type MenuItem } from "./ContextMenu.svelte";
  import ProgramPicker from "./ProgramPicker.svelte";
  import { paintStage, toBitmap, type Bitmap } from "./xhair-canvas";

  const SWATCHES = ["#ff4757", "#ffa502", "#fff200", "#2ed573", "#00e5ff", "#1e90ff", "#a55eea"];

  const items = $derived.by((): StripItem[] => {
    let i = 0;
    return app.presets.map((p) => {
      const swatch = slotAccent(p.slot, p.slot === "Normal" ? 0 : i++);
      return {
        id: p.slot,
        label: p.name,
        swatch,
        bound: !!p.exe,
        meta: p.slot === "Normal" ? undefined : `G${p.dials.gamma.toFixed(2)} V${Math.round(p.vibrance)}`,
      };
    });
  });

  let newOpen = $state(false);
  let renaming = $state(false);
  let picker = $state<null | "create" | "bind">(null);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  function openMenu(slot: string, e: MouseEvent) {
    const p = app.presets.find((x) => x.slot === slot);
    if (!p) return;
    if (slot === "Normal") {
      menu = { x: e.clientX, y: e.clientY, items: [{ label: "Native baseline · locked", icon: Lock, disabled: true }] };
      return;
    }
    menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: "Rename", icon: Pencil, run: () => { void app.pickPreset(slot).then(() => (renaming = true)); } },
        { label: "Duplicate", icon: Copy, run: () => void app.duplicatePreset(slot) },
        { label: p.exe ? "Unbind program" : "Bind to program…", icon: Link2, run: () => {
          if (p.exe) void app.bindPreset(slot, null);
          else void app.pickPreset(slot).then(() => (picker = "bind"));
        } },
        { sep: true, label: "" },
        { label: "Delete", icon: Trash2, danger: true, run: () => void app.deletePreset(slot) },
      ],
    };
  }

  // Selected crosshair drawn 1:1 at scene centre.
  let xcv = $state<HTMLCanvasElement>();
  let bmp = $state<Bitmap | null>(null);
  $effect(() => {
    const st = app.selectedCrosshair?.style;
    if (!st || !app.overlayOn) {
      bmp = null;
      return;
    }
    let live = true;
    renderCrosshair($state.snapshot(st)).then((img) => {
      if (live) bmp = toBitmap(img);
    }).catch(() => {});
    return () => {
      live = false;
    };
  });
  $effect(() => {
    if (!xcv) return;
    const b = bmp;
    const dpr = window.devicePixelRatio || 1;
    const css = b ? b.size / dpr : 1;
    xcv.style.width = `${css}px`;
    xcv.style.height = `${css}px`;
    paintStage(xcv, b, { cssW: css, cssH: css, zoom: 1 });
  });

  const boundTo = (exe: string) => app.presets.find((p) => p.exe === exe)?.name ?? null;
</script>

<Strip {items} active={app.active} onselect={(s) => void app.pickPreset(s)} oncontext={openMenu} ondblclick={(s) => s !== "Normal" && app.active === s && (renaming = true)}>
  {#snippet pin()}
    <div class="new-wrap">
      <button class="new display" onclick={() => (newOpen = !newOpen)}>+ NEW <ChevronDown size={12} /></button>
      {#if newOpen}
        <button class="backdrop clear" aria-label="Close" onclick={() => (newOpen = false)}></button>
        <div class="menu new-menu" role="menu">
          <button class="menu-item" role="menuitem" onclick={() => { newOpen = false; void app.createPreset(); }}>Blank</button>
          <button class="menu-item" role="menuitem" onclick={() => { newOpen = false; picker = "create"; }}>From a running program…</button>
        </div>
      {/if}
    </div>
  {/snippet}
</Strip>

<div class="body" class:grid={app.grid}>
  <section class="preview" style="--b: {app.dials.brightness}; --c: {app.dials.contrast}; --v: {app.vibranceNorm}">
    <div class="scene"></div>
    <span class="corner tl"></span><span class="corner tr"></span><span class="corner bl"></span><span class="corner br"></span>
    <span class="lbl mono l">LIVE PREVIEW · SCENE</span>
    <span class="lbl mono r">
      {#if app.monitor}\\.\{app.monitor.name} · {app.monitor.width}×{app.monitor.height}{:else}PRIMARY DISPLAY{/if}
    </span>
    <canvas class="xhair" bind:this={xcv}></canvas>
    {#if bmp && app.selectedCrosshair}
      <span class="lbl mono cap">XHAIR: {app.selectedCrosshair.name.toUpperCase()} · 1:1</span>
    {/if}
    <div class="tray">
      <div class="swatches">
        {#each SWATCHES as c}<span class="sw" style="background: {c}"></span>{/each}
      </div>
      <div class="ramp"></div>
    </div>
  </section>

  <section class="controls">
    <div class="actions">
      {#if renaming && app.current}
        <InlineRename value={app.current.name} onsave={(n) => { renaming = false; void app.renamePreset(app.active, n); }} oncancel={() => (renaming = false)} />
      {:else}
        <span class="sec display">
          PRESET{#if app.readOnly} · LOCKED · NATIVE BASELINE{/if}
        </span>
        <span class="grow"></span>
        {#if !app.readOnly}
          <button class="chip xs" onclick={() => (renaming = true)}>RENAME</button>
        {/if}
        <button class="chip xs" onclick={() => void app.duplicatePreset(app.active)}>DUPLICATE</button>
        {#if !app.readOnly}
          <ArmChip label="DELETE" size="xs" onconfirm={() => void app.deletePreset(app.active)} />
        {/if}
      {/if}
    </div>

    <Dial label="GAMMA" bind:value={app.dials.gamma} min={0.3} max={2.8} step={0.01} format={(v) => v.toFixed(2)} range={["0.30", "2.80"]} disabled={app.readOnly} onchange={() => app.liveApply()} />
    <Dial label="BRIGHTNESS" bind:value={app.dials.brightness} min={-0.5} max={0.5} step={0.01} format={(v) => (v > 0 ? "+" : v < 0 ? "−" : "") + Math.abs(v).toFixed(2)} range={["−0.50", "+0.50"]} disabled={app.readOnly} onchange={() => app.liveApply()} />
    <Dial label="CONTRAST" bind:value={app.dials.contrast} min={0.5} max={2} step={0.01} format={(v) => v.toFixed(2)} range={["0.50", "2.00"]} disabled={app.readOnly} onchange={() => app.liveApply()} />
    <Dial
      label={app.vendor ? `VIBRANCE · ${app.vendor === "amd" ? "ADL" : "NVAPI"}` : "VIBRANCE · N/A"}
      bind:value={app.vibrance}
      min={app.vibranceMin}
      max={app.vibranceMax}
      step={1}
      format={(v) => String(Math.round(v))}
      suffix={`/${app.vibranceMax}`}
      range={[String(app.vibranceMin), String(app.vibranceMax)]}
      tone="telemetry"
      disabled={app.readOnly}
      unavailable={!app.vendor}
      footnote="needs NVIDIA (NVAPI) or AMD (ADL) driver · gamma/brightness/contrast still work"
      onchange={() => app.liveApply()}
    />

    {#if !app.readOnly}
      <div class="bind mono" class:bound={!!app.current?.exe}>
        {#if app.current?.exe}
          <span class="k">BOUND</span><span class="v">· {app.current.exe}</span>
          <span class="grow"></span>
          <button class="lnk" onclick={() => void app.bindPreset(app.active, null)}>UNBIND</button>
        {:else}
          <span class="k">BIND</span><span class="v">· — no program</span>
          <span class="grow"></span>
          <button class="lnk accent" onclick={() => (picker = "bind")}>PICK GAME ▸</button>
        {/if}
      </div>
    {/if}

    <div class="footer">
      <button class="chip lg reset" disabled={app.readOnly || !app.dirty} onclick={() => app.revert()}>RESET</button>
      <button class="chip lg accent save" class:saved={!app.dirty} disabled={app.readOnly || !app.dirty} onclick={() => void app.save()}>
        {app.dirty ? "SAVE" : "SAVED"}
      </button>
    </div>
  </section>
</div>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}
{#if picker === "create"}
  <ProgramPicker sub="A new preset named after the game, bound to it" onpick={(exe, title) => void app.createPresetFromGame(exe, title)} onclose={() => (picker = null)} {boundTo} boundHint="opens it" />
{:else if picker === "bind"}
  <ProgramPicker sub="Auto-applies {app.current?.name ?? ''} while this program runs" onpick={(exe) => void app.bindPreset(app.active, exe)} onclose={() => (picker = null)} {boundTo} />
{/if}

<style>
  .new-wrap {
    position: relative;
    display: flex;
    align-items: stretch;
  }
  .new {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 0 12px;
    border: 0;
    background: transparent;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    color: var(--accent);
    cursor: pointer;
    white-space: nowrap;
  }
  .new:hover {
    color: var(--accent-hover);
  }
  .new-menu {
    position: absolute;
    top: calc(100% + 2px);
    right: 0;
    min-width: 200px;
    z-index: 30;
  }

  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 296px;
    gap: 14px;
    padding: 14px 16px;
  }
  .body.grid {
    background-image: var(--hud-grid);
    background-size: 24px 24px;
  }

  /* ── preview ── */
  .preview {
    position: relative;
    min-height: 0;
    border: 1px solid var(--hud-line-3);
    border-radius: var(--hud-r);
    overflow: hidden;
    background: #000;
  }
  .scene {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, oklch(0.6 0.06 230) 0%, oklch(0.46 0.05 230) 50%, oklch(0.3 0.04 135) 50%, oklch(0.16 0.02 135) 100%);
    filter: brightness(calc(1 + var(--b))) contrast(var(--c)) saturate(max(0, calc(1 + var(--v) * 1.6)));
  }
  .lbl {
    position: absolute;
    font-size: 10px;
    letter-spacing: 0.1em;
    text-shadow: 0 1px 2px #000;
    pointer-events: none;
  }
  .lbl.l {
    top: 10px;
    left: 10px;
    color: rgba(255, 255, 255, 0.8);
  }
  .lbl.r {
    top: 10px;
    right: 10px;
    color: var(--telemetry-text);
  }
  .lbl.cap {
    left: 10px;
    bottom: 84px;
    font-size: 9.5px;
    letter-spacing: 0.08em;
    color: rgba(255, 255, 255, 0.65);
  }
  .corner {
    position: absolute;
    width: 14px;
    height: 14px;
    border: 1.5px solid rgba(255, 255, 255, 0.6);
    pointer-events: none;
  }
  .corner.tl { top: 26px; left: 8px; border-right: 0; border-bottom: 0; }
  .corner.tr { top: 26px; right: 8px; border-left: 0; border-bottom: 0; }
  .corner.bl { bottom: 78px; left: 8px; border-right: 0; border-top: 0; }
  .corner.br { bottom: 78px; right: 8px; border-left: 0; border-top: 0; }
  .xhair {
    position: absolute;
    left: 50%;
    top: calc(50% - 35px);
    transform: translate(-50%, -50%);
    pointer-events: none;
    image-rendering: pixelated;
  }
  .tray {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    background: linear-gradient(180deg, transparent, oklch(0 0 0 / 0.55));
  }
  .swatches {
    display: flex;
    gap: 4px;
    filter: brightness(calc(1 + var(--b))) contrast(var(--c)) saturate(max(0, calc(1 + var(--v) * 1.6)));
  }
  .sw {
    flex: 1;
    height: 38px;
    border-radius: 2px;
  }
  .ramp {
    height: 8px;
    border-radius: 2px;
    background: linear-gradient(90deg, #000, #fff);
    filter: brightness(calc(1 + var(--b))) contrast(var(--c));
  }

  /* ── controls ── */
  .controls {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }
  .actions {
    height: 26px;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .sec {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.12em;
    color: var(--hud-fg-dim);
    white-space: nowrap;
  }
  .grow {
    flex: 1;
  }
  .bind {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 8px 10px;
    border: 1px dashed var(--hud-line-6);
    border-radius: var(--hud-r);
    font-size: 10px;
    color: var(--hud-fg-low);
  }
  .bind.bound {
    border-style: solid;
    border-color: color-mix(in oklab, var(--accent) 30%, transparent);
  }
  .bind .k {
    color: var(--hud-fg-3);
    letter-spacing: 0.08em;
  }
  .bind.bound .v {
    color: var(--hud-fg-2);
  }
  .lnk {
    padding: 0;
    border: 0;
    background: transparent;
    font: inherit;
    letter-spacing: 0.08em;
    color: var(--hud-fg-3);
    cursor: pointer;
  }
  .lnk:hover {
    color: var(--hud-fg);
  }
  .lnk.accent {
    color: var(--accent);
  }
  .lnk.accent:hover {
    color: var(--accent-hover);
  }
  .footer {
    margin-top: auto;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    height: 32px;
  }
  .footer .chip {
    height: 32px;
    justify-content: center;
    font-size: 11px;
    letter-spacing: 0.12em;
  }
  .reset {
    color: var(--danger);
    border-color: color-mix(in oklab, var(--danger) 45%, transparent);
    font-weight: 600;
  }
  .reset:hover:not(:disabled) {
    background: var(--danger-soft);
  }
  .save {
    font-weight: 700;
  }
  .save:hover:not(:disabled) {
    box-shadow: 0 0 18px color-mix(in oklab, var(--accent) 35%, transparent);
  }
  .save:active:not(:disabled) {
    transform: scale(0.97);
  }
  .save.saved {
    opacity: 0.5;
  }
</style>
