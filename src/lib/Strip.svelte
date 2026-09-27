<script lang="ts">
  // 44px section tab strip. Items scroll horizontally under fade masks; the
  // `pin` snippet (usually `+ NEW ▾`) stays pinned to the right outside the
  // scroller. At ≥10 items a searchable `▾ ALL` pin joins it.
  import { tick, type Snippet } from "svelte";
  import { ChevronDown, Link2, Search } from "lucide-svelte";
  import type { IconComponent } from "./types";

  export type StripItem = {
    id: string;
    label: string;
    swatch?: string; // 6×6 square in this colour
    thumb?: string; // 14×14 data URL
    icon?: IconComponent;
    bound?: boolean; // shows a link icon
    dot?: string; // small attention dot in this colour (e.g. update available)
    meta?: string; // mono readout shown only while active
    dim?: boolean;
  };

  interface Props {
    items: StripItem[];
    active: string;
    onselect: (id: string) => void;
    oncontext?: (id: string, e: MouseEvent) => void;
    ondblclick?: (id: string) => void;
    pin?: Snippet;
    right?: Snippet; // free-floating right slot (e.g. the overlay toggle)
  }
  let { items, active, onselect, oncontext, ondblclick, pin, right }: Props = $props();

  let scroller = $state<HTMLDivElement>();
  let atStart = $state(true);
  let atEnd = $state(true);
  let allOpen = $state(false);
  let query = $state("");

  const overflow = $derived(items.length >= 10);
  const filtered = $derived(
    query.trim() ? items.filter((i) => i.label.toLowerCase().includes(query.trim().toLowerCase())) : items,
  );

  function measure() {
    const el = scroller;
    if (!el) return;
    atStart = el.scrollLeft <= 1;
    atEnd = el.scrollLeft + el.clientWidth >= el.scrollWidth - 1;
  }

  // The active tab always scrolls into view (new preset, hotkey cycle).
  $effect(() => {
    void active;
    void items.length;
    tick().then(() => {
      scroller?.querySelector<HTMLElement>(".tab.active")?.scrollIntoView({ inline: "nearest", block: "nearest" });
      measure();
    });
  });

  function onwheel(e: WheelEvent) {
    if (!scroller || e.deltaY === 0) return;
    scroller.scrollLeft += e.deltaY;
    e.preventDefault();
  }
  function pickAll(id: string) {
    allOpen = false;
    query = "";
    onselect(id);
  }
  function focus(node: HTMLElement) {
    node.focus();
  }
</script>

<svelte:window onresize={measure} />

<div class="strip">
  <div class="scroll-wrap" class:fade-l={!atStart} class:fade-r={!atEnd}>
    <div class="scroller" bind:this={scroller} onscroll={measure} {onwheel} role="tablist">
      {#each items as it (it.id)}
        <button
          class="tab display"
          class:active={it.id === active}
          class:dim={it.dim}
          role="tab"
          aria-selected={it.id === active}
          onclick={() => onselect(it.id)}
          oncontextmenu={(e) => {
            if (!oncontext) return;
            e.preventDefault();
            oncontext(it.id, e);
          }}
          ondblclick={() => ondblclick?.(it.id)}
        >
          {#if it.icon}<it.icon size={13} strokeWidth={2} />{/if}
          {#if it.thumb}<img class="thumb" src={it.thumb} alt="" draggable="false" />{/if}
          {#if it.swatch}<span class="swatch" style="background: {it.swatch}"></span>{/if}
          <span class="label">{it.label}</span>
          {#if it.bound}<span class="link"><Link2 size={11} /></span>{/if}
          {#if it.dot}<span class="dot" style="background: {it.dot}; box-shadow: 0 0 6px {it.dot}"></span>{/if}
          {#if it.meta && it.id === active}<span class="meta mono">{it.meta}</span>{/if}
          {#if it.id === active}<span class="bar"></span>{/if}
        </button>
      {/each}
    </div>
  </div>
  {#if overflow}
    <div class="pin all">
      <button class="pin-btn display" onclick={() => (allOpen = !allOpen)}>
        <ChevronDown size={12} /> ALL
      </button>
      {#if allOpen}
        <button class="backdrop clear" aria-label="Close" onclick={() => (allOpen = false)}></button>
        <div class="menu all-menu" role="menu">
          <label class="search">
            <Search size={12} />
            <input class="field" placeholder="Filter…" bind:value={query} use:focus />
          </label>
          <div class="all-list">
            {#each filtered as it (it.id)}
              <button class="menu-item" class:on={it.id === active} role="menuitem" onclick={() => pickAll(it.id)}>
                {#if it.swatch}<span class="swatch" style="background: {it.swatch}"></span>{/if}
                {it.label}
              </button>
            {:else}
              <div class="none mono">NO MATCH</div>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/if}
  {#if pin}
    <div class="pin">{@render pin()}</div>
  {/if}
  {#if right}
    <div class="right">{@render right()}</div>
  {/if}
</div>

<style>
  .strip {
    position: relative;
    height: 44px;
    flex: 0 0 44px;
    display: flex;
    align-items: stretch;
    padding: 0 16px;
    border-bottom: 1px solid var(--hud-line);
    min-width: 0;
  }
  .scroll-wrap {
    position: relative;
    flex: 1;
    min-width: 0;
    display: flex;
  }
  .scroll-wrap::before,
  .scroll-wrap::after {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    width: 36px;
    pointer-events: none;
    z-index: 1;
    opacity: 0;
    transition: opacity 160ms var(--ease-hud);
  }
  .scroll-wrap::before {
    left: 0;
    background: linear-gradient(90deg, var(--hud-bg), transparent);
  }
  .scroll-wrap::after {
    right: 0;
    background: linear-gradient(270deg, var(--hud-bg), transparent);
  }
  .scroll-wrap.fade-l::before,
  .scroll-wrap.fade-r::after {
    opacity: 1;
  }
  .scroller {
    flex: 1;
    display: flex;
    align-items: stretch;
    gap: 2px;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: none;
  }
  .scroller::-webkit-scrollbar {
    height: 2px;
  }
  .scroller::-webkit-scrollbar-thumb {
    background: oklch(0.35 0.006 250);
  }
  .tab {
    position: relative;
    flex: 0 0 auto;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 0 14px;
    border: 0;
    background: transparent;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--hud-fg-dim);
    white-space: nowrap;
    cursor: pointer;
    transition: color 140ms var(--ease-hud), background 140ms var(--ease-hud);
  }
  .tab:hover {
    color: var(--hud-fg);
  }
  .tab.dim {
    opacity: 0.6;
  }
  .tab.active {
    font-weight: 700;
    color: var(--hud-fg-hi);
    background: linear-gradient(180deg, transparent, color-mix(in oklab, var(--accent) 10%, transparent));
  }
  .bar {
    position: absolute;
    left: 0;
    right: 0;
    bottom: -1px;
    height: 2px;
    background: var(--accent);
    transform-origin: left;
    animation: hud-bar 260ms var(--ease-hud) 120ms both;
  }
  .label {
    max-width: 150px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .swatch {
    width: 6px;
    height: 6px;
    border-radius: 1px;
    flex: 0 0 6px;
  }
  .thumb {
    width: 14px;
    height: 14px;
    image-rendering: pixelated;
    -webkit-user-drag: none;
  }
  .link {
    display: inline-flex;
    color: var(--hud-fg-faint);
  }
  .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
  }
  .tab.active .link {
    color: var(--accent);
  }
  .meta {
    font-size: 10px;
    font-weight: 400;
    letter-spacing: 0.02em;
    text-transform: none;
    color: var(--hud-fg-faint);
    margin-left: 2px;
  }
  .pin {
    position: relative;
    flex: 0 0 auto;
    display: flex;
    align-items: stretch;
    border-left: 1px solid var(--hud-line);
    margin-left: 4px;
  }
  .pin.all {
    border-left: 0;
    margin-left: 0;
  }
  .pin-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 0 12px;
    border: 0;
    background: transparent;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    color: var(--hud-fg-dim);
    cursor: pointer;
  }
  .pin-btn:hover {
    color: var(--hud-fg);
  }
  .all-menu {
    position: absolute;
    top: calc(100% + 2px);
    right: 0;
    width: 220px;
    padding: 6px;
    z-index: 30;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--hud-fg-faint);
    margin-bottom: 4px;
  }
  .search .field {
    flex: 1;
    height: 26px;
  }
  .all-list {
    max-height: 220px;
    overflow-y: auto;
  }
  .none {
    padding: 8px;
    font-size: 10px;
    color: var(--hud-fg-faint);
    text-align: center;
  }
  .right {
    display: flex;
    align-items: center;
    margin-left: 8px;
  }
</style>
