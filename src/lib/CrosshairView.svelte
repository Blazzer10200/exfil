<script lang="ts">
  // Crosshairs tab: own list of crosshair designs + an editor. The preview is
  // drawn from the SAME Rust renderer the on-screen overlay uses, on a
  // device-pixel canvas, so 1× here is pixel-for-pixel what the overlay draws.
  import { onMount, untrack } from "svelte";
  import { slide } from "svelte/transition";
  import { listen } from "@tauri-apps/api/event";
  import {
    Copy,
    Gamepad2,
    Keyboard,
    CheckCircle2,
    XCircle,
    Palette,
    Crosshair as CrossIcon,
    CircleDot,
    Circle,
    Square,
    Move,
    CircleOff,
  } from "lucide-svelte";
  import Slider from "./Slider.svelte";
  import CrosshairRail from "./CrosshairRail.svelte";
  import {
    NO_CROSSHAIR,
    getCrosshairs,
    createCrosshair,
    updateCrosshair,
    renameCrosshair,
    deleteCrosshair,
    selectCrosshair,
    setCrosshairBinding,
    setCrosshairEnabled,
    renderCrosshair,
    type CrosshairStore,
    type CrosshairStyle,
    type CrosshairImage,
  } from "./api";

  const PREVIEW = 200; // CSS px, main stage
  const INSET = 64; // CSS px, actual-size inset
  const ZOOMS = [1, 2, 4, 8];
  const COLORS = ["#00ff66", "#00e5ff", "#ffee00", "#ff3df2", "#ff3b3b", "#ffffff"];
  const OUTLINE_COLORS = ["#000000", "#ffffff"];
  const SLIDE = { duration: 170 };
  type Part = "arms" | "t_style" | "dot" | "ring" | "outline";

  let store = $state<CrosshairStore | null>(null);
  // Editable copy of the selected crosshair's style; saved back debounced.
  let style = $state<CrosshairStyle | null>(null);
  let thumbs = $state<Record<string, string>>({});
  let zoom = $state(4);
  let canvas = $state<HTMLCanvasElement>();
  let inset = $state<HTMLCanvasElement>();
  // Rail instance — exposes openBindFor so the hero "Bind…" link opens the
  // same picker the rail's right-click menu uses.
  let rail = $state<{ openBindFor: (id: string) => void } | undefined>();
  let editing = $state(false);
  let draft = $state("");
  let toast = $state<{ msg: string; kind: "ok" | "err" } | null>(null);

  const current = $derived(store?.crosshairs.find((c) => c.id === store?.selected));
  const boundOnes = $derived(store?.crosshairs.filter((c) => c.exe) ?? []);
  const key = $derived(style ? JSON.stringify(style) : "");
  const empty = $derived(!!style && !style.arms && !style.dot && !style.ring);
  const nudged = $derived(!!style && (style.offset_x !== 0 || style.offset_y !== 0));

  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  function flash(msg: string, kind: "ok" | "err" = "ok") {
    toast = { msg, kind };
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), 2200);
  }

  function loadStyle() {
    style = current ? $state.snapshot(current.style) : null;
  }

  // ── Rendering ──
  let scratch: HTMLCanvasElement | undefined;
  function toCanvas(img: ImageData) {
    scratch ??= document.createElement("canvas");
    scratch.width = img.width;
    scratch.height = img.height;
    scratch.getContext("2d")?.putImageData(img, 0, 0);
    return scratch;
  }

  async function refreshThumb(id: string, s: CrosshairStyle) {
    try {
      const ci = await renderCrosshair(s);
      thumbs[id] = toCanvas(ci.image).toDataURL();
    } catch (e) {
      flash(String(e), "err");
    }
  }

  // Draw `ci` into a device-pixel-sized canvas at integer zoom `z`. The bitmap's
  // center boundary (`half`) lands exactly on the canvas's center boundary.
  function paint(cv: HTMLCanvasElement | undefined, css: number, z: number, ci: CrosshairImage, s: CrosshairStyle) {
    const ctx = cv?.getContext("2d");
    if (!cv || !ctx) return;
    const dpr = window.devicePixelRatio || 1;
    const px = Math.round(css * dpr);
    if (cv.width !== px) {
      cv.width = px;
      cv.height = px;
    }
    ctx.clearRect(0, 0, px, px);
    const mid = Math.floor(px / 2);
    // Pixel grid on the zoomed view — lines sit on bitmap pixel boundaries.
    if (z >= 4) {
      ctx.fillStyle = "rgba(255,255,255,0.07)";
      for (let x = mid % z; x < px; x += z) ctx.fillRect(x, 0, 1, px);
      for (let y = mid % z; y < px; y += z) ctx.fillRect(0, y, px, 1);
    }
    // Edge ticks straddle the exact center boundary (columns mid-1 | mid).
    const t = Math.max(4, Math.round(css * 0.05 * dpr));
    ctx.fillStyle = "rgba(255,255,255,0.45)";
    ctx.fillRect(mid - 1, 0, 2, t);
    ctx.fillRect(mid - 1, px - t, 2, t);
    ctx.fillRect(0, mid - 1, t, 2);
    ctx.fillRect(px - t, mid - 1, t, 2);
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(
      toCanvas(ci.image),
      mid + (s.offset_x - ci.half) * z,
      mid + (s.offset_y - ci.half) * z,
      ci.image.width * z,
      ci.image.height * z,
    );
  }

  // Coalesced render loop: at most one IPC in flight; slider drags that outrun
  // it collapse to the newest style. Zoom-only changes reuse the last bitmap.
  let want: { key: string; z: number; id: string | undefined } | null = null;
  let rendering = false;
  let last: { key: string; ci: CrosshairImage } | null = null;
  let thumbTimer: ReturnType<typeof setTimeout> | undefined;

  async function renderLoop() {
    if (rendering) return;
    rendering = true;
    try {
      while (want) {
        const job = want;
        want = null;
        const snap: CrosshairStyle = JSON.parse(job.key);
        const ci = last?.key === job.key ? last.ci : await renderCrosshair(snap);
        last = { key: job.key, ci };
        if (want) continue; // a newer request arrived mid-flight
        paint(canvas, PREVIEW, job.z, ci, snap);
        paint(inset, INSET, 1, ci, snap);
        const id = job.id;
        clearTimeout(thumbTimer);
        thumbTimer = setTimeout(() => {
          if (id) thumbs[id] = toCanvas(ci.image).toDataURL();
        }, 200);
      }
    } catch (e) {
      flash(String(e), "err");
    } finally {
      rendering = false;
    }
  }

  $effect(() => {
    if (!key || !canvas) return;
    inset; // repaint once the inset canvas mounts
    want = { key, z: zoom, id: untrack(() => current?.id) };
    renderLoop();
  });

  // ── Debounced save ──
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let pending: { id: string; snap: CrosshairStyle } | null = null;
  async function flushSave() {
    clearTimeout(saveTimer);
    const p = pending;
    pending = null;
    if (p) await updateCrosshair(p.id, p.snap);
  }

  $effect(() => {
    const k = key;
    const c = untrack(() => current);
    if (!k || !c || k === JSON.stringify(c.style)) return;
    const snap: CrosshairStyle = JSON.parse(k);
    // Optimistic: the local store copy tracks edits so re-selecting shows them.
    c.style = snap;
    pending = { id: c.id, snap };
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => flushSave().catch((e) => flash(String(e), "err")), 150);
  });

  onMount(() => {
    let dead = false;
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        store = await getCrosshairs();
        loadStyle();
        for (const c of store.crosshairs) refreshThumb(c.id, $state.snapshot(c.style));
      } catch (e) {
        flash(String(e), "err");
      }
      // Ctrl+Shift+F11 flips the overlay backend-side; mirror it here.
      const un = await listen<boolean>("crosshair-toggled", (e) => {
        if (store) store.enabled = e.payload;
        flash(e.payload ? "Crosshair overlay on" : "Crosshair overlay off");
      });
      if (dead) un();
      else unlisten = un;
    })();
    return () => {
      dead = true;
      unlisten?.();
    };
  });

  // ── Actions ──
  async function pick(id: string) {
    if (!store || id === store.selected) return;
    try {
      await selectCrosshair(id);
      store.selected = id;
      editing = false;
      loadStyle();
    } catch (e) {
      flash(String(e), "err");
    }
  }

  // Create (blank, or a copy of `from`) and select it — the backend selects
  // the new one too.
  async function create(from: string | null) {
    if (!store) return null;
    await flushSave();
    const src = from ? store.crosshairs.find((c) => c.id === from) : undefined;
    const c = await createCrosshair(src ? `${src.name} copy` : "", src ? $state.snapshot(src.style) : null);
    store.crosshairs.push(c);
    store.selected = c.id;
    loadStyle();
    refreshThumb(c.id, c.style);
    return c;
  }

  async function onNew(from: string | null) {
    try {
      const c = await create(from);
      if (c) flash(`Created ${c.name}`);
    } catch (e) {
      flash(String(e), "err");
    }
  }

  // New crosshair straight from a running game: named after its window
  // title, bound to its exe (mirrors the preset rail's "From a running program").
  async function onCreateFromGame(exe: string, title: string) {
    try {
      const c = await create(null);
      if (!c) return;
      const name = title.trim() || exe;
      if (name !== c.name) await renameCrosshair(c.id, name);
      store = await setCrosshairBinding(c.id, exe);
      loadStyle();
      flash(`Created ${name} · bound ${exe}`);
    } catch (e) {
      flash(String(e), "err");
    }
  }

  function startRename() {
    if (!current) return;
    draft = current.name;
    editing = true;
  }
  async function commitRename() {
    if (!editing || !current) return;
    editing = false;
    const name = draft.trim();
    if (!name || name === current.name) return;
    try {
      await renameCrosshair(current.id, name);
      current.name = name;
    } catch (e) {
      flash(String(e), "err");
    }
  }
  function onRenameKey(e: KeyboardEvent) {
    if (e.key === "Enter") commitRename();
    else if (e.key === "Escape") editing = false;
  }

  async function onRename(id: string, name: string) {
    const c = store?.crosshairs.find((x) => x.id === id);
    try {
      await renameCrosshair(id, name);
      if (c) c.name = name;
    } catch (e) {
      flash(String(e), "err");
    }
  }

  // Deleting the picked crosshair falls back to None (like colors → Normal);
  // deleting any other one leaves the pick alone.
  async function onDelete(id: string) {
    if (!store || store.crosshairs.length <= 1) return;
    const name = store.crosshairs.find((c) => c.id === id)?.name ?? "crosshair";
    try {
      await flushSave();
      store = await deleteCrosshair(id);
      editing = false;
      loadStyle();
      flash(`Deleted ${name}`);
    } catch (e) {
      flash(String(e), "err");
    }
  }

  async function bind(id: string, exe: string | null) {
    try {
      await flushSave();
      store = await setCrosshairBinding(id, exe);
      loadStyle();
      flash(exe ? `Bound ${exe}` : "Unbound");
    } catch (e) {
      flash(String(e), "err");
    }
  }

  async function toggleOverlay() {
    if (!store) return;
    try {
      store.enabled = await setCrosshairEnabled(!store.enabled);
    } catch (e) {
      flash(String(e), "err");
    }
  }

  function toggle(k: Part) {
    if (style) style[k] = !style[k];
  }
  function setColor(c: string) {
    if (style) style.color = c;
  }
  function setOutlineColor(c: string) {
    if (style) style.outline_color = c;
  }
  function resetNudge() {
    if (style) {
      style.offset_x = 0;
      style.offset_y = 0;
    }
  }

  const px = (v: number) => `${v}px`;
  const signed = (v: number) => `${v > 0 ? "+" : ""}${v}px`;
</script>

{#snippet partSwitch(on: boolean, label: string, part: Part)}
  <button class="mini-switch" class:on role="switch" aria-checked={on} aria-label={label} onclick={() => toggle(part)}>
    <span class="knob"></span>
  </button>
{/snippet}

<div class="xview">
  <CrosshairRail
    bind:this={rail}
    crosshairs={store?.crosshairs ?? []}
    selected={store?.selected ?? NO_CROSSHAIR}
    {thumbs}
    onselect={pick}
    oncreate={() => onNew(null)}
    oncreategame={onCreateFromGame}
    onduplicate={onNew}
    ondelete={onDelete}
    onrename={onRename}
    onbind={bind}
    onerror={(m) => flash(m, "err")}
  />

  <main class="panel">
    {#if store && store.selected === NO_CROSSHAIR}
      <header class="hero">
        <div class="hero-text">
          <h1>None</h1>
          <p class="sub">
            <CircleOff size={13} />
            <span>No crosshair outside bound games</span>
          </p>
        </div>
        <div class="hero-actions">
          {@render overlaySwitch(store.enabled)}
        </div>
      </header>

      <section class="card games">
        <div class="games-head">
          <span class="sec-title"><Gamepad2 size={14} /> Per-game crosshairs</span>
          <span class="games-note">Switch in while their game is the window in front</span>
        </div>
        {#each boundOnes as c (c.id)}
          <button class="game-row" onclick={() => pick(c.id)} title="Edit {c.name}">
            <span class="game-thumb">
              {#if thumbs[c.id]}<img src={thumbs[c.id]} alt="" />{/if}
            </span>
            <span class="game-name">{c.name}</span>
            <span class="game-exe mono">{c.exe}</span>
          </button>
        {:else}
          <p class="games-empty">
            No games bound yet. Right-click a crosshair → <strong>Bind to game…</strong>, or
            <strong>Add crosshair → From a running game…</strong>
          </p>
        {/each}
      </section>

      <footer class="foot">
        {@render hotkeyHint(store.enabled)}
        <div class="spacer"></div>
        {@render toastChip()}
      </footer>
    {:else if store && current && style}
      <header class="hero">
        <div class="hero-text">
          {#if editing}
            <!-- svelte-ignore a11y_autofocus -->
            <input class="rename" bind:value={draft} onblur={commitRename} onkeydown={onRenameKey} autofocus />
          {:else}
            <h1>
              <button class="title" ondblclick={startRename} title="Double-click to rename">{current.name}</button>
            </h1>
          {/if}
          <p class="sub">
            <Gamepad2 size={13} />
            {#if current.exe}
              <span>Switches in while <strong>{current.exe}</strong> is in front</span>
              <button class="link" onclick={() => bind(current.id, null)}>Unbind</button>
            {:else}
              <span>Not bound to a game</span>
              <button class="link" onclick={() => rail?.openBindFor(current.id)}>Bind…</button>
            {/if}
          </p>
        </div>
        <div class="hero-actions">
          <button class="icon-btn" title="Duplicate" aria-label="Duplicate" onclick={() => onNew(current.id)}>
            <Copy size={14} />
          </button>
          {@render overlaySwitch(store.enabled)}
        </div>
      </header>

      <section class="card editor">
        <div class="left">
          <div class="stage">
            <canvas bind:this={canvas} class="main" style="width: {PREVIEW}px; height: {PREVIEW}px;"></canvas>
            {#if zoom > 1}
              <div class="inset" title="Actual size">
                <canvas bind:this={inset} style="width: {INSET}px; height: {INSET}px;"></canvas>
                <span>1:1</span>
              </div>
            {/if}
            {#if empty}<div class="stage-empty">Turn on a part to draw</div>{/if}
          </div>
          <div class="zoom" role="group" aria-label="Preview zoom">
            {#each ZOOMS as z (z)}
              <button class:on={zoom === z} aria-pressed={zoom === z} onclick={() => (zoom = z)}>{z}×</button>
            {/each}
          </div>
          <p class="legend">
            {zoom === 1 ? "Actual size — exact screen pixels" : `${zoom}× zoom · grid = screen pixels`}
          </p>
        </div>

        <div class="right">
          <div class="sec">
            <div class="sec-head">
              <span class="sec-title"><Palette size={14} /> Color</span>
            </div>
            <div class="sec-body">
              <div class="swatches span">
                {#each COLORS as c (c)}
                  <button
                    class="sw"
                    class:on={style.color.toLowerCase() === c}
                    style="--c: {c}"
                    aria-label="Color {c}"
                    onclick={() => setColor(c)}
                  ></button>
                {/each}
                <label class="sw custom" title="Custom color">
                  <input type="color" bind:value={style.color} aria-label="Custom color" />
                </label>
                <span class="hex mono">{style.color.toUpperCase()}</span>
              </div>
              <div class="span">
                <Slider label="Opacity" bind:value={style.opacity} min={0.1} max={1} step={0.05} format={(v) => `${Math.round(v * 100)}%`} />
              </div>
            </div>
          </div>

          <div class="sec" class:off={!style.arms}>
            <div class="sec-head">
              <span class="sec-title"><CrossIcon size={14} /> Cross</span>
              {@render partSwitch(style.arms, "Cross", "arms")}
            </div>
            {#if style.arms}
              <div class="sec-body" transition:slide={SLIDE}>
                <Slider label="Length" bind:value={style.length} min={1} max={40} step={1} format={px} />
                <Slider label="Thickness" bind:value={style.thickness} min={1} max={10} step={1} format={px} />
                <Slider label="Gap" bind:value={style.gap} min={0} max={30} step={1} format={px} />
                <div class="inline-opt">
                  <span class="field-label">T-style</span>
                  {@render partSwitch(style.t_style, "T-style (no top arm)", "t_style")}
                </div>
              </div>
            {/if}
          </div>

          <div class="sec" class:off={!style.dot}>
            <div class="sec-head">
              <span class="sec-title"><CircleDot size={14} /> Center dot</span>
              {@render partSwitch(style.dot, "Center dot", "dot")}
            </div>
            {#if style.dot}
              <div class="sec-body" transition:slide={SLIDE}>
                <Slider label="Size" bind:value={style.dot_size} min={1} max={10} step={1} format={px} />
              </div>
            {/if}
          </div>

          <div class="sec" class:off={!style.ring}>
            <div class="sec-head">
              <span class="sec-title"><Circle size={14} /> Ring</span>
              {@render partSwitch(style.ring, "Ring", "ring")}
            </div>
            {#if style.ring}
              <div class="sec-body" transition:slide={SLIDE}>
                <Slider label="Radius" bind:value={style.ring_radius} min={2} max={60} step={1} format={px} />
                <Slider label="Thickness" bind:value={style.ring_thickness} min={1} max={6} step={1} format={px} />
              </div>
            {/if}
          </div>

          <div class="sec" class:off={!style.outline}>
            <div class="sec-head">
              <span class="sec-title"><Square size={14} /> Outline</span>
              {@render partSwitch(style.outline, "Outline", "outline")}
            </div>
            {#if style.outline}
              <div class="sec-body" transition:slide={SLIDE}>
                <Slider label="Thickness" bind:value={style.outline_thickness} min={1} max={3} step={1} format={px} />
                <div class="swatches">
                  {#each OUTLINE_COLORS as c (c)}
                    <button
                      class="sw sm"
                      class:on={style.outline_color.toLowerCase() === c}
                      style="--c: {c}"
                      aria-label="Outline {c}"
                      onclick={() => setOutlineColor(c)}
                    ></button>
                  {/each}
                  <label class="sw sm custom" title="Custom outline color">
                    <input type="color" bind:value={style.outline_color} aria-label="Custom outline color" />
                  </label>
                </div>
              </div>
            {/if}
          </div>

          <div class="sec">
            <div class="sec-head">
              <span class="sec-title"><Move size={14} /> Position</span>
              {#if nudged}
                <button class="link" onclick={resetNudge}>Re-center</button>
              {:else}
                <span class="centered">Dead center</span>
              {/if}
            </div>
            <div class="sec-body">
              <Slider label="Nudge X" bind:value={style.offset_x} min={-50} max={50} step={1} format={signed} />
              <Slider label="Nudge Y" bind:value={style.offset_y} min={-50} max={50} step={1} format={signed} />
            </div>
          </div>
        </div>
      </section>

      <footer class="foot">
        {@render hotkeyHint(store.enabled)}
        <div class="spacer"></div>
        {@render toastChip()}
      </footer>
    {:else}
      {@render toastChip()}
    {/if}
  </main>
</div>

{#snippet overlaySwitch(on: boolean)}
  <button class="overlay-switch" class:on role="switch" aria-checked={on} onclick={toggleOverlay}>
    <span class="track"><span class="knob"></span></span>
    Overlay {on ? "on" : "off"}
  </button>
{/snippet}

{#snippet hotkeyHint(on: boolean)}
  <span class="hint" class:off={!on}>
    <Keyboard size={13} />
    <span class="mono">Ctrl+Shift+F11</span> toggles · primary monitor · borderless/windowed games
  </span>
{/snippet}

{#snippet toastChip()}
  {#if toast}
    <span class="toast" class:err={toast.kind === "err"}>
      {#if toast.kind === "err"}<XCircle size={13} />{:else}<CheckCircle2 size={13} />{/if}
      {toast.msg}
    </span>
  {/if}
{/snippet}

<style>
  .xview { flex: 1; display: flex; min-height: 0; }

  /* ── None panel: per-game overview ── */
  .games { flex-shrink: 0; padding: 14px 16px; display: flex; flex-direction: column; gap: 6px; }
  .games-head { display: flex; align-items: baseline; justify-content: space-between; gap: 12px; margin-bottom: 4px; }
  .games-note { font-size: var(--fs-xs); color: var(--fg-faint); }
  .game-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border-radius: var(--radius);
    border: 1px solid transparent;
    background: transparent;
    color: var(--fg-2);
    font: inherit;
    font-size: var(--fs-sm);
    text-align: left;
    cursor: pointer;
    transition: background 120ms ease, border-color 120ms ease;
  }
  .game-row:hover { background: var(--surface-hover); border-color: var(--border); }
  .game-thumb {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    flex-shrink: 0;
    border-radius: var(--radius-sm);
    background: linear-gradient(180deg, oklch(0.52 0.05 230) 50%, oklch(0.24 0.03 135) 50%);
    box-shadow: inset 0 1px 0 color-mix(in oklab, white 10%, transparent), inset 0 0 0 1px oklch(0 0 0 / 0.35);
    overflow: hidden;
  }
  .game-thumb img { max-width: 100%; max-height: 100%; object-fit: scale-down; image-rendering: pixelated; }
  .game-name { font-weight: 500; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .game-exe { margin-left: auto; font-size: var(--fs-xs); color: var(--fg-subtle); flex-shrink: 0; }
  .games-empty { margin: 4px 0 2px; font-size: var(--fs-xs); color: var(--fg-subtle); line-height: 1.6; }
  .games-empty strong { color: var(--fg-2); font-weight: 600; }

  /* ── Panel: hero + footer fixed, only the controls column scrolls ── */
  .panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px 22px;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  .hero { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; flex-shrink: 0; }
  .hero-text { min-width: 0; }
  h1 { margin: 0; font-size: var(--fs-hero); font-weight: 650; letter-spacing: -0.02em; line-height: 1.15; }
  .title {
    padding: 0;
    border: none;
    background: none;
    color: var(--fg);
    font: inherit;
    letter-spacing: inherit;
    cursor: text;
    text-align: left;
  }
  .rename {
    width: 100%;
    max-width: 320px;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    border: 1px solid color-mix(in oklab, var(--accent) 55%, var(--border-strong));
    background: var(--field);
    color: var(--fg);
    font: inherit;
    font-size: var(--fs-hero);
    font-weight: 650;
    outline: none;
    box-shadow: 0 0 0 2px color-mix(in oklab, var(--accent) 18%, transparent);
  }
  .sub {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 5px 0 0;
    font-size: var(--fs-sm);
    color: var(--fg-muted);
  }
  .sub :global(svg) { flex-shrink: 0; opacity: 0.75; }
  .sub span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sub strong { color: var(--fg-2); font-weight: 600; }
  .hero-actions { display: flex; align-items: center; gap: 6px; flex-shrink: 0; }
  .icon-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 30px;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--field);
    color: var(--fg-muted);
    font: inherit;
    font-size: var(--fs-xs);
    cursor: pointer;
    transition: background 100ms ease, color 100ms ease, border-color 100ms ease;
  }
  .icon-btn:hover:not(:disabled) { background: var(--surface-hover); color: var(--fg); }
  .overlay-switch {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 30px;
    padding: 0 11px 0 7px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--field);
    color: var(--fg-muted);
    font: inherit;
    font-size: var(--fs-xs);
    font-weight: 500;
    cursor: pointer;
    transition: background 140ms ease, color 140ms ease, border-color 140ms ease, box-shadow 140ms ease;
  }
  .overlay-switch.on {
    color: var(--fg);
    border-color: color-mix(in oklab, var(--accent) 55%, transparent);
    background: color-mix(in oklab, var(--accent) 12%, var(--field));
    box-shadow: 0 0 14px color-mix(in oklab, var(--accent) 22%, transparent);
  }
  .track {
    position: relative;
    width: 26px;
    height: 15px;
    border-radius: 999px;
    background: var(--border-strong);
    transition: background 140ms ease;
  }
  .overlay-switch.on .track { background: var(--accent); }
  .track .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: white;
    transition: transform 160ms var(--ease-soft, ease);
  }
  .overlay-switch.on .knob { transform: translateX(11px); }

  /* ── Editor card ── */
  .editor {
    flex: 1;
    min-height: 0;
    padding: 16px 6px 16px 18px;
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 20px;
  }
  .left { display: flex; flex-direction: column; gap: 9px; width: 200px; }
  .right {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
    padding-right: 12px;
    scrollbar-gutter: stable;
  }
  .stage {
    position: relative;
    width: 200px;
    height: 200px;
    border-radius: var(--radius-lg, 12px);
    overflow: hidden;
    /* Sky/ground split at dead center so light and dark crosshairs both read. */
    background: linear-gradient(
      180deg,
      oklch(0.68 0.06 230) 0%,
      oklch(0.5 0.05 230) 50%,
      oklch(0.3 0.04 135) 50%,
      oklch(0.17 0.02 135) 100%
    );
    border: 1px solid var(--border);
    box-shadow: inset 0 1px 0 oklch(1 0 0 / 0.08), var(--shadow-lg);
  }
  canvas { display: block; image-rendering: pixelated; }
  .inset {
    position: absolute;
    right: 6px;
    bottom: 6px;
    border-radius: var(--radius-sm);
    overflow: hidden;
    background: linear-gradient(180deg, oklch(0.5 0.05 230) 50%, oklch(0.3 0.04 135) 50%);
    border: 1px solid oklch(0 0 0 / 0.45);
    box-shadow: 0 4px 12px oklch(0 0 0 / 0.45), inset 0 1px 0 oklch(1 0 0 / 0.1);
  }
  .inset span {
    position: absolute;
    top: 2px;
    left: 4px;
    font-size: 9px;
    font-weight: 600;
    color: white;
    opacity: 0.75;
    text-shadow: 0 1px 2px black;
  }
  .stage-empty {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    font-size: var(--fs-xs);
    color: white;
    text-shadow: 0 1px 2px black;
  }
  .zoom {
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: var(--radius-sm);
    background: var(--field);
    border: 1px solid var(--border);
  }
  .zoom button {
    flex: 1;
    padding: 4px 0;
    border: none;
    border-radius: var(--radius-xs);
    background: transparent;
    color: var(--fg-muted);
    font: inherit;
    font-size: var(--fs-xs);
    cursor: pointer;
    transition: background 120ms ease, color 120ms ease;
  }
  .zoom button:hover { color: var(--fg); }
  .zoom button.on { background: color-mix(in oklab, var(--accent) 18%, transparent); color: var(--fg); }
  .legend { margin: 0; font-size: 10px; color: var(--fg-faint); text-align: center; }

  /* ── Sections ── */
  .sec {
    flex-shrink: 0;
    padding: 10px 12px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: linear-gradient(180deg, color-mix(in oklab, white 2.5%, transparent), transparent);
    transition: border-color 160ms ease, background 160ms ease;
  }
  .sec.off { background: transparent; border-style: dashed; }
  .sec-head { display: flex; align-items: center; justify-content: space-between; gap: 8px; min-height: 22px; }
  .sec-title { display: inline-flex; align-items: center; gap: 7px; font-size: var(--fs-sm); font-weight: 600; color: var(--fg-2); }
  .sec-title :global(svg) { color: var(--accent); }
  .sec.off .sec-title { color: var(--fg-subtle); }
  .sec.off .sec-title :global(svg) { color: var(--fg-faint); }
  .sec-body {
    display: grid;
    grid-template-columns: 1fr 1fr;
    column-gap: 16px;
    padding-top: 12px;
    align-items: start;
  }
  .span { grid-column: 1 / -1; }
  .sec-body :global(.slider) { margin-bottom: 10px; }
  .inline-opt { display: flex; align-items: center; justify-content: space-between; padding-top: 18px; }
  .centered { font-size: var(--fs-xs); color: var(--fg-faint); }
  .mini-switch {
    position: relative;
    flex-shrink: 0;
    width: 30px;
    height: 17px;
    padding: 0;
    border: none;
    border-radius: 999px;
    background: var(--border-strong);
    cursor: pointer;
    transition: background 150ms ease, box-shadow 150ms ease;
  }
  .mini-switch .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 13px;
    height: 13px;
    border-radius: 50%;
    background: white;
    box-shadow: 0 1px 2px oklch(0 0 0 / 0.4);
    transition: transform 170ms var(--ease-soft, ease);
  }
  .mini-switch.on { background: var(--accent); box-shadow: 0 0 10px color-mix(in oklab, var(--accent) 35%, transparent); }
  .mini-switch.on .knob { transform: translateX(13px); }
  .swatches { display: flex; flex-wrap: wrap; align-items: center; gap: 7px; margin-bottom: 12px; }
  .sw {
    position: relative;
    width: 22px;
    height: 22px;
    padding: 0;
    border-radius: 50%;
    border: 1px solid oklch(0 0 0 / 0.4);
    background: var(--c);
    cursor: pointer;
    box-shadow: inset 0 1px 0 oklch(1 0 0 / 0.25);
    transition: transform 100ms ease, box-shadow 120ms ease;
  }
  .sw.sm { width: 18px; height: 18px; }
  .sw:hover { transform: scale(1.1); }
  .sw.on { box-shadow: 0 0 0 2px var(--bg-elev-2), 0 0 0 4px color-mix(in oklab, var(--c) 80%, white); }
  .sw.custom { background: conic-gradient(red, yellow, lime, cyan, blue, magenta, red); overflow: hidden; }
  .sw.custom input { position: absolute; inset: 0; width: 100%; height: 100%; opacity: 0; cursor: pointer; }
  .hex { margin-left: auto; font-size: var(--fs-xs); color: var(--fg-subtle); }
  .link {
    flex-shrink: 0;
    padding: 0;
    border: none;
    background: none;
    color: var(--accent);
    font: inherit;
    font-size: var(--fs-xs);
    cursor: pointer;
  }
  .link:hover { text-decoration: underline; }

  /* ── Footer ── */
  .foot { display: flex; align-items: center; gap: 12px; flex-shrink: 0; min-height: 28px; margin-top: auto; }
  .spacer { flex: 1; }
  .hint { display: inline-flex; align-items: center; gap: 6px; font-size: var(--fs-xs); color: var(--fg-subtle); }
  .hint.off { color: var(--fg-faint); }
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
  .game-row:focus-visible,
  .title:focus-visible,
  .icon-btn:focus-visible,
  .overlay-switch:focus-visible,
  .mini-switch:focus-visible,
  .zoom button:focus-visible,
  .sw:focus-visible,
  .sw:focus-within,
  .link:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--ring);
  }
</style>
