<script lang="ts">
  // CROSSHAIR // LIBRARY: starter templates grid + preview/info column.
  import { onMount } from "svelte";
  import { renderCrosshair, setCrosshairEnabled, type CrosshairStyle } from "./api";
  import { app } from "./state.svelte";
  import { toast } from "./toast.svelte";
  import { decode } from "./xhair-code";
  import { paintStage, toBitmap, type Bitmap } from "./xhair-canvas";
  import templates from "./crosshair-templates.json";

  type Cat = "cross" | "dot" | "ring" | "shapes";
  type Template = { id: string; name: string; category: Cat; description: string; code: string };
  const ALL = (templates as Template[]).map((t) => ({ ...t, style: decode(t.code)?.style }))
    .filter((t): t is Template & { style: CrosshairStyle } => !!t.style);
  const FILTERS: { id: "all" | Cat; label: string }[] = [
    { id: "all", label: "ALL" },
    { id: "cross", label: "CROSS" },
    { id: "dot", label: "DOT" },
    { id: "ring", label: "RING" },
    { id: "shapes", label: "SHAPES" },
  ];

  let filter = $state<"all" | Cat>("all");
  let picked = $state(ALL[0]?.id ?? "");
  const shown = $derived(filter === "all" ? ALL : ALL.filter((t) => t.category === filter));
  const sel = $derived(ALL.find((t) => t.id === picked) ?? ALL[0]);

  // Card bitmaps rendered once, lazily.
  let bmps = $state<Record<string, Bitmap>>({});
  onMount(() => {
    let alive = true;
    (async () => {
      for (const t of ALL) {
        if (!alive) return;
        try {
          bmps[t.id] = toBitmap(await renderCrosshair(t.style));
        } catch {
          // card stays empty; USE still works off the decoded style
        }
      }
    })();
    return () => {
      alive = false;
    };
  });
  function card(cv: HTMLCanvasElement, id: string) {
    $effect(() => {
      const b = bmps[id];
      if (b) paintStage(cv, b, { cssW: cv.clientWidth, cssH: cv.clientHeight, zoom: 2, grid: true });
    });
  }
  let preview = $state<HTMLCanvasElement>();
  let inset = $state<HTMLCanvasElement>();
  $effect(() => {
    const b = sel ? bmps[sel.id] : undefined;
    if (preview) paintStage(preview, b ?? null, { cssW: preview.clientWidth, cssH: 200, zoom: 4, grid: true, ticks: true });
    if (inset) paintStage(inset, b ?? null, { cssW: 56, cssH: 56, zoom: 1 });
  });

  async function use() {
    if (!sel) return;
    await app.createCrosshair(sel.name, { ...sel.style });
    app.setView("crosshair");
  }

  // 5s on-screen preview: push a temp crosshair, revert everything after.
  let previewing = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  async function previewOnScreen() {
    if (!sel || previewing) return;
    previewing = true;
    const prevSel = app.selected;
    const prevOn = app.overlayOn;
    try {
      const id = (await app.createCrosshair(`preview · ${sel.name}`, { ...sel.style }))?.id;
      if (!prevOn) await app.setOverlay(true);
      toast.info("◌ PREVIEWING ON SCREEN · 5s", { spin: true, sticky: true });
      timer = setTimeout(async () => {
        try {
          await app.pickCrosshair(prevSel);
          if (id) await app.deleteCrosshair(id, { silent: true });
          if (!prevOn) await setCrosshairEnabled(false).then((v) => (app.overlayOn = v));
        } finally {
          toast.clear();
          previewing = false;
        }
      }, 5000);
    } catch {
      previewing = false;
      toast.error("✕ PREVIEW FAILED");
    }
  }
  onMount(() => () => clearTimeout(timer));

  let codeIn = $state("");
  async function importCode() {
    const d = decode(codeIn.trim());
    if (!d) {
      toast.error("✕ INVALID CODE · expected EXFIL, Valorant or CS2 format");
      return;
    }
    await app.createCrosshair(`${d.source.toUpperCase()} import`, d.style);
    codeIn = "";
    app.setView("crosshair");
    toast.ok(`✓ CODE IMPORTED · ${d.source.toUpperCase()}`);
  }
  const hex = (s: string) => s.toUpperCase();
</script>

<div class="body" class:grid={app.grid}>
  <div class="left">
    <div class="filters">
      {#each FILTERS as f (f.id)}
        <button class="chip xs" class:accent={filter === f.id} onclick={() => (filter = f.id)}>{f.label}</button>
      {/each}
      <span class="grow"></span>
      <span class="hint mono">2× · GRID = PX</span>
    </div>
    <div class="cards">
    {#each shown as t (t.id)}
      <button class="card" class:on={sel?.id === t.id} onclick={() => (picked = t.id)} ondblclick={use}>
        <canvas class="cv" use:card={t.id}></canvas>
        <span class="lbl">
          <span class="cn display">{t.name}</span>
          <span class="cs mono">{t.style.arms ? `${t.style.length}·${t.style.thickness}·${t.style.gap}` : t.style.dot ? `DOT ${t.style.dot_size}` : t.style.ring ? `RING ${t.style.ring_radius}` : "—"} · {hex(t.style.color)}</span>
        </span>
      </button>
    {/each}
    </div>
  </div>

  <aside class="side">
    {#if sel}
      <div class="pv">
        <canvas bind:this={preview} class="pvc"></canvas>
        <div class="inset"><canvas bind:this={inset} style="width:56px;height:56px"></canvas><span class="mono">1:1</span></div>
        <span class="tag mono">{sel.name.toUpperCase()} · 4×</span>
      </div>
      <div class="info panel">
        <span class="ih2"><span class="in display">{sel.name}</span><span class="ic mono">STARTER · {sel.category.toUpperCase()}</span></span>
        <p class="id">{sel.description}</p>
        <div class="stats">
          <span class="st"><span class="k mono">LEN</span><span class="v mono">{sel.style.arms ? sel.style.length : "—"}</span></span>
          <span class="st"><span class="k mono">THICK</span><span class="v mono">{sel.style.arms ? sel.style.thickness : sel.style.dot ? sel.style.dot_size : "—"}</span></span>
          <span class="st"><span class="k mono">GAP</span><span class="v mono">{sel.style.arms ? sel.style.gap : "—"}</span></span>
          <span class="st"><span class="k mono">OUTLINE</span><span class="v mono">{sel.style.outline ? sel.style.outline_thickness : "OFF"}</span></span>
        </div>
        <span class="col"><span class="k mono">COLOR</span><span class="swatch" style="background: {sel.style.color}"></span><span class="v mono">{hex(sel.style.color)}</span></span>
      </div>
      <button class="chip lg accent use" onclick={use}>USE TEMPLATE</button>
      <button class="chip lg use" disabled={previewing} onclick={previewOnScreen}>{previewing ? "PREVIEWING…" : "PREVIEW ON SCREEN"}</button>
    {/if}
    <div class="import">
      <span class="it display">IMPORT A CODE</span>
      <span class="ih mono">EXFIL · Valorant · CS2</span>
      <div class="ir">
        <input class="field mono" placeholder="paste a code…" bind:value={codeIn} onkeydown={(e) => e.key === "Enter" && void importCode()} />
        <button class="chip xs" disabled={!codeIn.trim()} onclick={importCode}>IMPORT</button>
      </div>
    </div>
  </aside>
</div>

<style>
  .grow { flex: 1; }
  .left { display: flex; flex-direction: column; gap: 10px; min-height: 0; }
  .filters { display: flex; align-items: center; gap: 4px; height: 22px; flex: 0 0 22px; }
  .hint { font-size: 10px; letter-spacing: 0.1em; color: var(--hud-fg-low); }
  .body { flex: 1; min-height: 0; display: grid; grid-template-columns: minmax(0, 1fr) 296px; gap: 14px; padding: 14px 16px; animation: hud-in 200ms var(--ease-hud) both; }
  .body.grid { background-image: var(--hud-grid); background-size: 24px 24px; }
  .cards { display: grid; grid-template-columns: repeat(4, 1fr); grid-auto-rows: max-content; gap: 10px; overflow-y: auto; min-height: 0; padding-right: 2px; align-content: start; }
  .card {
    display: flex; flex-direction: column; padding: 0; border: 1px solid var(--hud-line-2); border-radius: var(--hud-r); overflow: hidden;
    background: var(--hud-panel); cursor: pointer; text-align: left; color: var(--hud-fg);
    transition: border-color 120ms var(--ease-hud), background 120ms var(--ease-hud);
  }
  .card:hover { border-color: var(--hud-line-5); }
  .card.on { border-color: var(--accent); box-shadow: 0 0 0 1px color-mix(in oklab, var(--accent) 30%, transparent); }
  .cv { width: 100%; aspect-ratio: 1.25; background: linear-gradient(180deg, oklch(0.6 0.06 230) 0%, oklch(0.46 0.05 230) 50%, oklch(0.3 0.04 135) 50%, oklch(0.16 0.02 135) 100%); }
  .lbl { display: flex; flex-direction: column; gap: 2px; padding: 5px 8px 6px; min-width: 0; }
  .cn { font-size: 10.5px; font-weight: 700; letter-spacing: 0.08em; text-transform: uppercase; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .cs { font-size: 9px; color: var(--hud-fg-low); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .ih2 { display: flex; align-items: baseline; justify-content: space-between; gap: 8px; }
  .side { display: flex; flex-direction: column; gap: 8px; min-height: 0; overflow-y: auto; }
  .pv { position: relative; height: 200px; flex: 0 0 200px; border: 1px solid var(--hud-line-3); border-radius: var(--hud-r); background: #1a1a1a; overflow: hidden; }
  .pvc { position: absolute; inset: 0; width: 100%; height: 100%; }
  .inset { position: absolute; right: 8px; bottom: 8px; width: 56px; height: 56px; border: 1px solid rgba(255,255,255,0.25); background: #000; }
  .inset canvas { display: block; }
  .inset span { position: absolute; left: 3px; bottom: 1px; font-size: 8px; color: rgba(255,255,255,0.6); }
  .tag { position: absolute; left: 8px; top: 8px; font-size: 10px; letter-spacing: 0.1em; color: rgba(255,255,255,0.8); }
  .info { display: flex; flex-direction: column; gap: 6px; padding: 10px 12px; }
  .in { font-size: 13px; font-weight: 700; letter-spacing: 0.1em; text-transform: uppercase; color: var(--hud-fg-hi); }
  .ic { font-size: 9.5px; letter-spacing: 0.1em; color: var(--hud-fg-low); white-space: nowrap; }
  .id { margin: 2px 0 4px; font-size: 11.5px; line-height: 1.5; color: var(--hud-fg-2); }
  .stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 4px; }
  .st { display: flex; flex-direction: column; gap: 2px; padding: 5px 6px; border-radius: 2px; background: var(--hud-pill); }
  .k { font-size: 8.5px; letter-spacing: 0.1em; color: var(--hud-fg-faint); }
  .v { font-size: 11px; color: var(--hud-fg-hi); }
  .col { display: flex; align-items: center; gap: 6px; margin-top: 2px; }
  .swatch { width: 12px; height: 12px; border-radius: 2px; border: 1px solid rgba(255,255,255,0.2); }
  .use { justify-content: center; width: 100%; height: 32px; }
  .import { margin-top: auto; display: flex; flex-direction: column; gap: 4px; padding: 10px 12px; border: 1px dashed var(--hud-line-4); border-radius: var(--hud-r); }
  .it { font-size: 10px; font-weight: 600; letter-spacing: 0.14em; color: var(--hud-fg-dim); }
  .ih { font-size: 9px; color: var(--hud-fg-faint); }
  .ir { display: flex; gap: 6px; margin-top: 4px; }
  .ir .field { flex: 1; height: 26px; font-size: 10px; min-width: 0; }
</style>
