<script lang="ts">
  // CROSSHAIR: strip (None · Library · crosshairs · + NEW · overlay toggle),
  // zoomable stage, part panels, share code.
  import { onMount } from "svelte";
  import { ChevronDown, CircleOff, LayoutGrid, Pencil, Copy, Trash2, Link2, Plus, Image } from "lucide-svelte";
  import { DEFAULT_STYLE, NO_CROSSHAIR, SHAPES, renderCrosshair, type CrosshairStyle } from "./api";
  import { app } from "./state.svelte";
  import { toast } from "./toast.svelte";
  import { prefs, savePrefs, type Backdrop } from "./prefs";
  import { encode, decode } from "./xhair-code";
  import { paintStage, thumbUrl, toBitmap, type Bitmap } from "./xhair-canvas";
  import Strip, { type StripItem } from "./Strip.svelte";
  import XPanel from "./XPanel.svelte";
  import SliderRow from "./SliderRow.svelte";
  import InlineRename from "./InlineRename.svelte";
  import ArmChip from "./ArmChip.svelte";
  import ContextMenu, { type MenuItem } from "./ContextMenu.svelte";
  import ProgramPicker from "./ProgramPicker.svelte";
  import LibraryView from "./LibraryView.svelte";

  const SWATCHES = ["#00ff66", "#00e5ff", "#ffee00", "#ff3df2", "#ff3b3b", "#ffffff"];
  const ZOOMS = [1, 2, 4, 8];
  const BACKDROPS: { id: Backdrop; label: string }[] = [
    { id: "scene", label: "Scene" },
    { id: "bright", label: "Bright" },
    { id: "dark", label: "Dark" },
    { id: "noise", label: "Noise" },
  ];

  // ── strip ──
  let thumbs = $state<Record<string, string>>({});
  const thumbKeys = new Map<string, string>();
  let thumbTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const list = app.crosshairs.map((c) => ({ id: c.id, style: $state.snapshot(c.style) }));
    clearTimeout(thumbTimer);
    thumbTimer = setTimeout(async () => {
      for (const { id, style } of list) {
        if (id === NO_CROSSHAIR) continue;
        const key = JSON.stringify(style);
        if (thumbKeys.get(id) === key) continue;
        thumbKeys.set(id, key);
        try {
          thumbs[id] = thumbUrl(toBitmap(await renderCrosshair(style)), 14);
        } catch {
          // thumb stays stale; the stage will surface any render error
        }
      }
    }, 200);
  });
  const items = $derived.by((): StripItem[] => [
    { id: NO_CROSSHAIR, label: "NONE", icon: CircleOff },
    { id: "__library", label: "LIBRARY", icon: LayoutGrid },
    ...app.crosshairs
      .filter((c) => c.id !== NO_CROSSHAIR)
      .map((c) => ({ id: c.id, label: c.name, thumb: thumbs[c.id], bound: !!c.exe })),
  ]);

  let newOpen = $state(false);
  let renaming = $state(false);
  let picker = $state<null | "create" | "bind">(null);
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const sel = $derived(app.selectedCrosshair);
  const isNone = $derived(app.selected === NO_CROSSHAIR || !sel);
  const userCount = $derived(app.crosshairs.filter((c) => c.id !== NO_CROSSHAIR).length);

  const inLibrary = $derived(app.view === "library");
  const stripActive = $derived(inLibrary ? "__library" : app.selected);
  function onselect(id: string) {
    if (id === "__library") return app.setView("library");
    if (inLibrary) app.setView("crosshair");
    void app.pickCrosshair(id);
  }
  function openMenu(id: string, e: MouseEvent) {
    const c = app.crosshairs.find((x) => x.id === id);
    if (!c || id === NO_CROSSHAIR || id === "__library") return;
    menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: "Rename", icon: Pencil, run: () => void app.pickCrosshair(id).then(() => (renaming = true)) },
        { label: c.exe ? "Unbind program" : "Bind to program…", icon: Link2, run: () => {
          if (c.exe) void app.bindCrosshair(id, null);
          else void app.pickCrosshair(id).then(() => (picker = "bind"));
        } },
        { label: "Duplicate", icon: Copy, run: () => void app.duplicateCrosshair(id) },
        { sep: true, label: "" },
        { label: "Delete", icon: Trash2, danger: true, disabled: userCount <= 1, run: () => void app.deleteCrosshair(id) },
      ],
    };
  }
  async function createBlank() {
    await app.createCrosshair(`Crosshair ${userCount + 1}`, null);
  }
  async function pasteCode(intoCurrent: boolean) {
    let text = "";
    try {
      text = await navigator.clipboard.readText();
    } catch {
      toast.error("✕ CLIPBOARD UNAVAILABLE");
      return;
    }
    const d = decode(text);
    if (!d) {
      toast.error("✕ INVALID CODE · expected EXFIL, Valorant or CS2 format");
      return;
    }
    if (intoCurrent && sel && !isNone) {
      style = d.style;
      commit();
    } else {
      await app.createCrosshair(`${d.source.toUpperCase()} import`, d.style);
    }
    toast.ok(`✓ CODE IMPORTED · ${d.source.toUpperCase()}`);
  }
  async function copyCode() {
    try {
      await navigator.clipboard.writeText(code);
      toast.ok("✓ CODE COPIED");
    } catch {
      toast.error("✕ CLIPBOARD UNAVAILABLE");
    }
  }
  const boundTo = (exe: string) => app.crosshairs.find((c) => c.exe === exe)?.name ?? null;

  // ── editing ──
  // svelte-ignore state_referenced_locally
  let style = $state<CrosshairStyle>({ ...(app.selectedCrosshair?.style ?? DEFAULT_STYLE) });
  let loadedFor = "";
  $effect(() => {
    const c = app.selectedCrosshair;
    if (!c) return;
    if (c.id !== loadedFor) {
      loadedFor = c.id;
      style = { ...$state.snapshot(c.style) };
    }
  });
  function commit() {
    if (!sel || isNone) return;
    app.editCrosshair(sel.id, $state.snapshot(style));
  }
  function set<K extends keyof CrosshairStyle>(k: K, v: CrosshairStyle[K]) {
    style[k] = v;
    commit();
  }
  const code = $derived(isNone ? "" : encode(style));
  const validHex = (s: string) => /^#[0-9a-fA-F]{6}$/.test(s);

  // ── rendering (coalesced: one IPC in flight, newest style wins) ──
  let bmp = $state<Bitmap | null>(null);
  let rendering = false;
  let pendingRender = false;
  async function renderLoop() {
    if (rendering) {
      pendingRender = true;
      return;
    }
    rendering = true;
    try {
      do {
        pendingRender = false;
        if (isNone) {
          bmp = null;
          break;
        }
        bmp = toBitmap(await renderCrosshair($state.snapshot(style)));
      } while (pendingRender);
    } catch (e) {
      toast.error(`✕ RENDER · ${String(e).slice(0, 60)}`);
    } finally {
      rendering = false;
    }
  }
  $effect(() => {
    void JSON.stringify(style);
    void isNone;
    void renderLoop();
  });

  // ── stage ──
  let zoom = $state(ZOOMS.includes(prefs.zoom) ? prefs.zoom : 4);
  let backdrop = $state<Backdrop>(prefs.backdrop);
  let backdropImage = $state<string | null>(prefs.backdropImage);
  let stageEl = $state<HTMLDivElement>();
  let cv = $state<HTMLCanvasElement>();
  let inset = $state<HTMLCanvasElement>();
  let size = $state({ w: 0, h: 0 });
  onMount(() => {
    const ro = new ResizeObserver(([e]) => {
      size = { w: e.contentRect.width, h: e.contentRect.height };
    });
    if (stageEl) ro.observe(stageEl);
    return () => ro.disconnect();
  });
  $effect(() => {
    if (!cv) return;
    paintStage(cv, bmp, { cssW: size.w, cssH: size.h, zoom, grid: true, ticks: true, dx: style.offset_x, dy: style.offset_y });
  });
  $effect(() => {
    if (!inset) return;
    paintStage(inset, bmp, { cssW: 72, cssH: 72, zoom: 1 });
  });
  function setZoom(z: number) {
    zoom = z;
    prefs.zoom = z;
    savePrefs();
  }
  function setBackdrop(b: Backdrop) {
    backdrop = b;
    prefs.backdrop = b;
    savePrefs();
  }
  let fileEl = $state<HTMLInputElement>();
  async function onScreenshotFile(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    try {
      const url = URL.createObjectURL(file);
      const img = new window.Image();
      await new Promise<void>((res, rej) => {
        img.onload = () => res();
        img.onerror = () => rej(new Error("decode"));
        img.src = url;
      });
      const k = Math.min(1, 1600 / img.width);
      const c = document.createElement("canvas");
      c.width = Math.round(img.width * k);
      c.height = Math.round(img.height * k);
      c.getContext("2d")!.drawImage(img, 0, 0, c.width, c.height);
      URL.revokeObjectURL(url);
      backdropImage = c.toDataURL("image/jpeg", 0.85);
      prefs.backdropImage = backdropImage;
      setBackdrop("custom");
    } catch {
      toast.error("✕ SCREENSHOT · couldn't read that image");
    }
  }
  const monitorLabel = $derived(app.monitor ? `${app.monitor.name} · ${app.monitor.width}×${app.monitor.height}` : "PRIMARY");
  const bound = $derived(app.crosshairs.filter((c) => c.id !== NO_CROSSHAIR && c.exe));
</script>

<Strip {items} active={stripActive} {onselect} oncontext={openMenu} ondblclick={(id) => id === app.selected && !isNone && (renaming = true)}>
  {#snippet pin()}
    <div class="new-wrap">
      <button class="new display" onclick={() => (newOpen = !newOpen)}>+ NEW <ChevronDown size={12} /></button>
      {#if newOpen}
        <button class="backdrop clear" aria-label="Close" onclick={() => (newOpen = false)}></button>
        <div class="menu new-menu" role="menu">
          <button class="menu-item" role="menuitem" onclick={() => { newOpen = false; void createBlank(); }}>Blank</button>
          <button class="menu-item" role="menuitem" onclick={() => { newOpen = false; app.setView("library"); }}>From library</button>
          <button class="menu-item" role="menuitem" onclick={() => { newOpen = false; void pasteCode(false); }}>Paste code</button>
          <button class="menu-item" role="menuitem" onclick={() => { newOpen = false; picker = "create"; }}>From running program…</button>
        </div>
      {/if}
    </div>
  {/snippet}
  {#snippet right()}
    <button class="ov display" class:on={app.overlayOn} onclick={() => void app.setOverlay(!app.overlayOn)}>
      OVERLAY <span class="ov-state mono">{app.overlayOn ? "ON" : "OFF"}</span>
      <span class="toggle" class:on={app.overlayOn}></span>
    </button>
  {/snippet}
</Strip>

{#if inLibrary}
  <LibraryView />
{:else}
<div class="body" class:grid={app.grid}>
  <section class="stage-col">
    <div class="stage bd-{backdrop}" bind:this={stageEl} style={backdrop === "custom" && backdropImage ? `background-image: url(${backdropImage})` : ""}>
      <canvas class="cv" bind:this={cv}></canvas>
      {#if isNone}
        <span class="none-pill mono">NO CROSSHAIR · OVERLAY IDLE</span>
      {/if}
      <span class="tag mono tl">{zoom === 1 ? "1:1 · ACTUAL SIZE" : `${zoom}× · GRID = SCREEN PX`}</span>
      <span class="tag mono tr cyan" title="Primary monitor only">{monitorLabel} ▾</span>
      {#if zoom > 1 && !isNone}
        <div class="inset"><canvas bind:this={inset} style="width:72px;height:72px"></canvas><span class="mono">1:1</span></div>
      {/if}
      <div class="zoom">
        {#each ZOOMS as z}
          <button class="zc mono" class:on={z === zoom} onclick={() => setZoom(z)}>{z}×</button>
        {/each}
      </div>
      <div class="picker">
        {#each BACKDROPS as b (b.id)}
          <button class="bp bd-{b.id}" class:on={backdrop === b.id} title={b.label} aria-label={b.label} onclick={() => setBackdrop(b.id)}></button>
        {/each}
        {#if backdropImage}
          <button class="bp bd-custom" class:on={backdrop === "custom"} title="Your screenshot" aria-label="Your screenshot" style="background-image: url({backdropImage})" onclick={() => setBackdrop("custom")}></button>
        {/if}
        <input class="hidden-file" type="file" accept="image/png,image/jpeg,image/webp" bind:this={fileEl} onchange={onScreenshotFile} />
        <button class="bp add" title="Use a screenshot…" aria-label="Use a screenshot" onclick={() => fileEl?.click()}>
          {#if backdropImage}<Image size={11} />{:else}<Plus size={11} />{/if}
        </button>
      </div>
    </div>
  </section>

  <section class="side">
    {#if isNone}
      <div class="actions"><span class="sec display">NONE · NOTHING OUTSIDE BOUND GAMES</span></div>
      {#if bound.length}
        <div class="bound-list">
          {#each bound as c (c.id)}
            <button class="bound-row panel" onclick={() => void app.pickCrosshair(c.id)}>
              {#if thumbs[c.id]}<img src={thumbs[c.id]} alt="" />{/if}
              <span class="display bn">{c.name}</span>
              <span class="mono be">· {c.exe}</span>
            </button>
          {/each}
        </div>
      {:else}
        <div class="empty">
          <span class="display et">NO CROSSHAIRS</span>
          <span class="mono ed">pick a starter from the library, start blank, or paste a code</span>
          <button class="chip lg accent" onclick={() => app.setView("library")}>OPEN LIBRARY</button>
          <button class="chip lg" onclick={createBlank}>BLANK CROSSHAIR</button>
          <button class="chip lg" onclick={() => void pasteCode(false)}>PASTE A CODE</button>
        </div>
      {/if}
    {:else if sel}
      <div class="actions">
        {#if renaming}
          <InlineRename value={sel.name} onsave={(n) => { renaming = false; void app.renameCrosshair(sel.id, n); }} oncancel={() => (renaming = false)} />
        {:else}
          <span class="sec display name">{sel.name}{#if sel.exe}<span class="mono exe"> · {sel.exe}</span>{/if}</span>
          <span class="grow"></span>
          <button class="chip xs" onclick={() => (renaming = true)}>RENAME</button>
          <button class="chip xs" onclick={() => (picker = "bind")}>{sel.exe ? "REBIND" : "BIND"}</button>
          <button class="chip xs" onclick={() => void app.duplicateCrosshair(sel.id)}>DUPLICATE</button>
          <ArmChip label="DELETE" size="xs" disabled={userCount <= 1} onconfirm={() => void app.deleteCrosshair(sel.id)} />
        {/if}
      </div>

      <div class="panels">
        <XPanel title="SHAPE" always>
          <div class="shapes">
            {#each SHAPES as s, i}
              <button class="shape" class:on={style.shape === i} title={s} aria-label={s} onclick={() => set("shape", i)}>
                <svg viewBox="0 0 16 16" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="square">
                  {#if i === 0}<path d="M8 1v4M8 11v4M1 8h4M11 8h4" />
                  {:else if i === 1}<path d="M2 2l4 4M14 2l-4 4M2 14l4-4M14 14l-4-4" />
                  {:else if i === 2}<path d="M8 11v4M1 8h4M11 8h4" />
                  {:else if i === 3}<path d="M3 12l5-5 5 5" />
                  {:else}<path d="M2 6V2h4M14 6V2h-4M2 10v4h4M14 10v4h-4" />{/if}
                </svg>
              </button>
            {/each}
          </div>
        </XPanel>

        <XPanel title="COLOR" always>
          <div class="color-row">
            <input class="field hex mono" value={style.color} maxlength={7} spellcheck="false" onchange={(e) => { const v = e.currentTarget.value.trim(); if (validHex(v)) set("color", v.toLowerCase()); else e.currentTarget.value = style.color; }} />
            <div class="swatches">
              {#each SWATCHES as c}
                <button class="sw" class:on={style.color === c} style="background: {c}" aria-label={c} onclick={() => set("color", c)}></button>
              {/each}
              <label class="sw custom" class:on={!SWATCHES.includes(style.color)} title="Custom">
                <input type="color" value={style.color} oninput={(e) => set("color", e.currentTarget.value)} />
              </label>
            </div>
          </div>
          <SliderRow label="OPACITY" value={Math.round(style.opacity * 100)} min={10} max={100} format={(v) => `${v}%`} onchange={(v) => set("opacity", v / 100)} />
          <div class="trow">
            <span class="display tl2">GLOW</span>
            <button class="toggle" class:on={style.glow} aria-label="Glow" onclick={() => set("glow", !style.glow)}></button>
          </div>
          {#if style.glow}
            <SliderRow label="RADIUS" value={style.glow_radius} min={0} max={8} onchange={(v) => set("glow_radius", v)} />
          {/if}
        </XPanel>

        <XPanel title="INNER LINES" on={style.arms} ontoggle={(v) => set("arms", v)}>
          <SliderRow label="LENGTH" value={style.length} min={1} max={40} onchange={(v) => set("length", v)} />
          <SliderRow label="THICKNESS" value={style.thickness} min={1} max={10} onchange={(v) => set("thickness", v)} />
          <SliderRow label="GAP" value={style.gap} min={0} max={30} onchange={(v) => set("gap", v)} />
        </XPanel>

        <XPanel title="OUTER LINES" on={style.outer} ontoggle={(v) => set("outer", v)}>
          <SliderRow label="LENGTH" value={style.outer_length} min={1} max={40} onchange={(v) => set("outer_length", v)} />
          <SliderRow label="THICKNESS" value={style.outer_thickness} min={1} max={10} onchange={(v) => set("outer_thickness", v)} />
          <SliderRow label="GAP" value={style.outer_gap} min={0} max={60} onchange={(v) => set("outer_gap", v)} />
          <SliderRow label="OPACITY" value={Math.round(style.outer_opacity * 100)} min={10} max={100} format={(v) => `${v}%`} onchange={(v) => set("outer_opacity", v / 100)} />
        </XPanel>

        <XPanel title="CENTER DOT" on={style.dot} ontoggle={(v) => set("dot", v)}>
          <SliderRow label="SIZE" value={style.dot_size} min={1} max={10} onchange={(v) => set("dot_size", v)} />
        </XPanel>

        <XPanel title="RING" on={style.ring} ontoggle={(v) => set("ring", v)}>
          <SliderRow label="RADIUS" value={style.ring_radius} min={2} max={60} onchange={(v) => set("ring_radius", v)} />
          <SliderRow label="THICKNESS" value={style.ring_thickness} min={1} max={6} onchange={(v) => set("ring_thickness", v)} />
        </XPanel>

        <XPanel title="OUTLINE" on={style.outline} ontoggle={(v) => set("outline", v)}>
          <SliderRow label="THICKNESS" value={style.outline_thickness} min={1} max={3} onchange={(v) => set("outline_thickness", v)} />
          <div class="trow">
            <span class="display tl2">COLOR</span>
            <div class="swatches">
              {#each ["#000000", "#ffffff"] as c}
                <button class="sw" class:on={style.outline_color === c} style="background: {c}" aria-label={c} onclick={() => set("outline_color", c)}></button>
              {/each}
              <label class="sw custom" class:on={!["#000000", "#ffffff"].includes(style.outline_color)} title="Custom">
                <input type="color" value={style.outline_color} oninput={(e) => set("outline_color", e.currentTarget.value)} />
              </label>
            </div>
          </div>
        </XPanel>

        <XPanel title="POSITION" always>
          <div class="trow">
            {#if style.offset_x === 0 && style.offset_y === 0}
              <span class="mono pos">DEAD CENTER</span>
            {:else}
              <span class="mono pos">{style.offset_x > 0 ? "+" : ""}{style.offset_x} · {style.offset_y > 0 ? "+" : ""}{style.offset_y}</span>
              <button class="lnk" onclick={() => { style.offset_x = 0; style.offset_y = 0; commit(); }}>RE-CENTER</button>
            {/if}
          </div>
          <SliderRow label="NUDGE X" value={style.offset_x} min={-50} max={50} onchange={(v) => set("offset_x", v)} />
          <SliderRow label="NUDGE Y" value={style.offset_y} min={-50} max={50} onchange={(v) => set("offset_y", v)} />
          <div class="trow">
            <span class="display tl2">SCALE WITH RESOLUTION</span>
            <button class="toggle" class:on={style.scale_with_resolution} aria-label="Scale with resolution" onclick={() => set("scale_with_resolution", !style.scale_with_resolution)}></button>
          </div>
        </XPanel>

        <XPanel title="SHARE CODE" always tone="telemetry">
          <div class="code-row">
            <span class="tag2 mono">EXFIL-1</span>
            <input class="field code mono" readonly value={code} onfocus={(e) => e.currentTarget.select()} />
          </div>
          <div class="code-acts">
            <button class="chip xs" onclick={copyCode}>COPY</button>
            <button class="chip xs" onclick={() => void pasteCode(true)}>PASTE</button>
          </div>
        </XPanel>
      </div>
    {/if}
  </section>
</div>
{/if}

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}
{#if picker === "create"}
  <ProgramPicker sub="A new crosshair named after the game, bound to it" onpick={(exe, title) => void app.createCrosshair(title.trim() || exe, null, exe)} onclose={() => (picker = null)} {boundTo} />
{:else if picker === "bind" && sel}
  <ProgramPicker sub="Switches {sel.name} in while this program is in front" onpick={(exe) => void app.bindCrosshair(sel.id, exe)} onclose={() => (picker = null)} {boundTo} />
{/if}

<style>
  .new-wrap { position: relative; display: flex; align-items: stretch; }
  .new {
    display: inline-flex; align-items: center; gap: 4px; padding: 0 12px; border: 0; background: transparent;
    font-size: 11px; font-weight: 600; letter-spacing: 0.08em; color: var(--accent); cursor: pointer; white-space: nowrap;
  }
  .new:hover { color: var(--accent-hover); }
  .new-menu { position: absolute; top: calc(100% + 2px); right: 0; min-width: 210px; z-index: 30; }
  .ov {
    display: inline-flex; align-items: center; gap: 8px; height: 26px; padding: 0 10px; border: 1px solid var(--hud-line-3);
    border-radius: var(--hud-r); background: var(--hud-panel); font-size: 10px; font-weight: 600; letter-spacing: 0.1em;
    color: var(--hud-fg-dim); cursor: pointer; white-space: nowrap;
  }
  .ov.on { color: var(--hud-fg-hi); border-color: color-mix(in oklab, var(--accent) 35%, var(--hud-line-3)); }
  .ov-state { font-size: 10px; color: var(--hud-fg-faint); }
  .ov.on .ov-state { color: var(--accent); }

  .body {
    flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(0, 1fr) 296px; gap: 14px; padding: 14px 16px;
  }
  .body.grid { background-image: var(--hud-grid); background-size: 24px 24px; }
  .stage-col { display: flex; flex-direction: column; min-height: 0; }
  .stage {
    position: relative; flex: 1; min-height: 0; border: 1px solid var(--hud-line-3); border-radius: var(--hud-r); overflow: hidden;
    background-size: cover; background-position: center;
  }
  .bd-scene { background-image: linear-gradient(180deg, oklch(0.6 0.06 230) 0%, oklch(0.46 0.05 230) 50%, oklch(0.3 0.04 135) 50%, oklch(0.16 0.02 135) 100%); }
  .bd-bright { background-color: #e8e8e8; }
  .bd-dark { background-color: #1a1a1a; }
  .bd-noise { background-image: repeating-conic-gradient(#2a2a2a 0 25%, #3a3a3a 0 50%); background-size: 8px 8px; }
  .cv { position: absolute; inset: 0; width: 100%; height: 100%; }
  .none-pill {
    position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%); padding: 6px 12px; border-radius: var(--hud-r);
    background: oklch(0 0 0 / 0.6); border: 1px dashed var(--hud-line-6); font-size: 10px; letter-spacing: 0.12em; color: var(--hud-fg-3);
  }
  .tag { position: absolute; top: 10px; font-size: 10px; letter-spacing: 0.1em; color: rgba(255,255,255,0.8); text-shadow: 0 1px 2px #000; pointer-events: none; }
  .tag.tl { left: 10px; }
  .tag.tr { right: 10px; padding: 2px 6px; border-radius: 2px; background: oklch(0 0 0 / 0.45); }
  .tag.cyan { color: var(--telemetry-text); }
  .inset {
    position: absolute; right: 10px; bottom: 10px; width: 72px; height: 72px; border: 1px solid rgba(255,255,255,0.25); border-radius: 2px;
    background: #000 url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='8' height='8'%3E%3Crect width='4' height='4' fill='%23222'/%3E%3Crect x='4' y='4' width='4' height='4' fill='%23222'/%3E%3C/svg%3E");
  }
  .inset canvas { display: block; }
  .inset span { position: absolute; left: 3px; bottom: 1px; font-size: 8px; color: rgba(255,255,255,0.6); }
  .zoom { position: absolute; left: 10px; bottom: 10px; display: flex; border: 1px solid var(--hud-line-5); border-radius: var(--hud-r); overflow: hidden; background: oklch(0 0 0 / 0.55); }
  .zc { width: 30px; height: 20px; border: 0; background: transparent; font-size: 10px; color: var(--hud-fg-3); cursor: pointer; }
  .zc + .zc { border-left: 1px solid var(--hud-line-4); }
  .zc.on { background: var(--accent); color: var(--accent-fg); font-weight: 600; }
  .picker { position: absolute; left: 50%; bottom: 10px; transform: translateX(-50%); display: flex; gap: 4px; padding: 3px; border-radius: var(--hud-r); background: oklch(0 0 0 / 0.55); }
  .bp { width: 20px; height: 20px; border: 1px solid rgba(255,255,255,0.15); border-radius: 2px; padding: 0; cursor: pointer; background-size: cover; background-position: center; }
  .bp.on { outline: 2px solid var(--accent); outline-offset: -2px; }
  .bp.add { display: grid; place-items: center; background: transparent; color: var(--hud-fg-3); }
  .bp.add:hover { color: var(--hud-fg-hi); }
  .hidden-file { display: none; }

  .side { display: flex; flex-direction: column; gap: 6px; min-height: 0; }
  .actions { height: 26px; flex: 0 0 26px; display: flex; align-items: center; gap: 6px; min-width: 0; }
  .sec { font-size: 10px; font-weight: 600; letter-spacing: 0.12em; color: var(--hud-fg-dim); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sec.name { color: var(--hud-fg-hi); font-size: 11px; text-transform: uppercase; }
  .exe { font-size: 10px; font-weight: 400; letter-spacing: 0; text-transform: none; color: var(--hud-fg-low); }
  .grow { flex: 1; }
  .panels { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: 6px; padding-right: 2px; }
  .shapes { display: grid; grid-template-columns: repeat(5, 1fr); gap: 4px; }
  .shape { height: 28px; display: grid; place-items: center; border: 1px solid var(--hud-line-3); border-radius: var(--hud-r); background: var(--hud-pill); color: var(--hud-fg-dim); cursor: pointer; }
  .shape:hover { color: var(--hud-fg); }
  .shape.on { color: var(--accent); border-color: var(--accent); background: var(--accent-soft); }
  .color-row { display: flex; align-items: center; gap: 8px; }
  .hex { width: 76px; height: 24px; font-size: 11px; }
  .swatches { display: flex; gap: 6px; }
  .sw { width: 16px; height: 16px; border: 0; border-radius: 2px; padding: 0; cursor: pointer; }
  .sw.on { outline: 2px solid var(--hud-fg); outline-offset: 1px; }
  .sw.custom { position: relative; background: conic-gradient(#f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00); overflow: hidden; }
  .sw.custom input { position: absolute; inset: -4px; width: 200%; height: 200%; opacity: 0; cursor: pointer; }
  .trow { display: flex; align-items: center; justify-content: space-between; gap: 8px; min-height: 16px; }
  .tl2 { font-size: 10px; font-weight: 600; letter-spacing: 0.12em; color: var(--hud-fg-dim); }
  .pos { font-size: 10px; letter-spacing: 0.08em; color: var(--hud-fg-3); }
  .lnk { padding: 0; border: 0; background: transparent; font-family: var(--font-mono); font-size: 10px; letter-spacing: 0.08em; color: var(--accent); cursor: pointer; }
  .lnk:hover { color: var(--accent-hover); }
  .code-row { display: flex; align-items: center; gap: 6px; }
  .tag2 { font-size: 9px; letter-spacing: 0.1em; padding: 2px 5px; border-radius: 2px; background: color-mix(in oklab, var(--telemetry) 15%, transparent); color: var(--telemetry-text); }
  .code { flex: 1; height: 26px; font-size: 10px; color: var(--hud-fg-2); }
  .code-acts { display: flex; gap: 6px; }

  .bound-list { display: flex; flex-direction: column; gap: 6px; }
  .bound-row { display: flex; align-items: center; gap: 8px; padding: 8px 10px; cursor: pointer; text-align: left; color: var(--hud-fg); }
  .bound-row:hover { background: var(--hud-hover); }
  .bound-row img { width: 14px; height: 14px; image-rendering: pixelated; }
  .bn { font-size: 11px; font-weight: 600; letter-spacing: 0.08em; text-transform: uppercase; }
  .be { font-size: 10px; color: var(--hud-fg-low); }
  .empty { display: flex; flex-direction: column; align-items: stretch; gap: 8px; padding: 20px 16px; border: 1px dashed var(--hud-line-4); border-radius: var(--hud-r); text-align: center; }
  .empty .chip { justify-content: center; }
  .et { font-size: 12px; font-weight: 700; letter-spacing: 0.14em; color: var(--hud-fg-2); }
  .ed { font-size: 10px; color: var(--hud-fg-low); line-height: 1.5; margin-bottom: 6px; }
</style>
