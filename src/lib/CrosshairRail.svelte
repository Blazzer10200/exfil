<script lang="ts">
  // Crosshair list — same interaction model as the preset rail (SlotRail): a
  // fixed "None" baseline on top (the crosshair counterpart of Normal), user
  // crosshairs below, right-click menu for Rename / Bind / Unbind / Duplicate /
  // Delete, dblclick to rename, and an "Add ▾" popover for blank or
  // from-a-running-game creation. Menu styles are shared via app.css.
  import type { Crosshair } from "./api";
  import { NO_CROSSHAIR } from "./api";
  import {
    Pencil,
    Trash2,
    Lock,
    Link2,
    Unlink,
    Copy,
    Gamepad2,
    Plus,
    FilePlus2,
    ChevronDown,
    CircleOff,
  } from "lucide-svelte";
  import ProgramPicker from "./ProgramPicker.svelte";

  interface Props {
    crosshairs: Crosshair[];
    selected: string;
    thumbs: Record<string, string>;
    onselect: (id: string) => void;
    oncreate: () => void;
    oncreategame: (exe: string, title: string) => void;
    onduplicate: (id: string) => void;
    ondelete: (id: string) => void;
    onrename: (id: string, name: string) => void;
    onbind: (id: string, exe: string | null) => void;
    onerror?: (message: string) => void;
  }
  let {
    crosshairs,
    selected,
    thumbs,
    onselect,
    oncreate,
    oncreategame,
    onduplicate,
    ondelete,
    onrename,
    onbind,
    onerror,
  }: Props = $props();

  const find = (id: string) => crosshairs.find((c) => c.id === id);

  // ── Inline rename ──
  let editing = $state<string | null>(null);
  let draft = $state("");

  function startRename(id: string) {
    const c = find(id);
    if (!c) return;
    editing = id;
    draft = c.name;
  }
  function commitRename() {
    if (editing === null) return;
    const id = editing;
    const name = draft.trim();
    editing = null;
    const c = find(id);
    if (name && c && name !== c.name) onrename(id, name);
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      (e.currentTarget as HTMLInputElement).blur();
    } else if (e.key === "Escape") {
      editing = null;
    }
  }

  // ── Right-click context menu ──
  // Height varies with the item (None = 1 row, unbound = 4, bound = 5), so
  // estimate per-row when clamping against the viewport.
  const MENU_W = 180;
  let menu = $state<{ id: string; x: number; y: number } | null>(null);

  function openMenu(e: MouseEvent, id: string) {
    e.preventDefault();
    const rows = id === NO_CROSSHAIR ? 1 : find(id)?.exe ? 5 : 4;
    const x = Math.min(e.clientX, window.innerWidth - MENU_W - 8);
    const y = Math.min(e.clientY, window.innerHeight - (12 + rows * 36) - 8);
    menu = { id, x: Math.max(8, x), y: Math.max(8, y) };
  }
  function closeMenu() {
    menu = null;
  }
  function menuAction(fn: (id: string) => void) {
    if (menu) fn(menu.id);
    closeMenu();
  }

  // ── Program binding ──
  // Same shared ProgramPicker as presets, in two intents:
  //  - "bind":   attach a game to an existing crosshair (right-click / hero link)
  //  - "create": make a NEW crosshair straight from a running game (Add ▾)
  let binder = $state<{ mode: "bind"; id: string } | { mode: "create" } | null>(null);

  export function openBindFor(id: string) {
    if (id !== NO_CROSSHAIR) binder = { mode: "bind", id };
  }
  function pickProgram(exe: string, title: string) {
    if (!binder) return;
    if (binder.mode === "create") oncreategame(exe, title);
    else onbind(binder.id, exe);
  }

  // ── Add popover ──
  let addOpen = $state(false);
  function closeAdd() {
    addOpen = false;
  }

  function focusOnMount(node: HTMLElement) {
    node.focus();
  }
</script>

<svelte:window
  onkeydown={(e) => e.key === "Escape" && (closeMenu(), closeAdd())}
  onblur={() => (closeMenu(), closeAdd())}
/>

<nav class="rail">
  <div class="list" onscroll={closeMenu}>
    <button
      class="item"
      class:active={selected === NO_CROSSHAIR}
      class:targeted={menu?.id === NO_CROSSHAIR}
      onclick={() => onselect(NO_CROSSHAIR)}
      oncontextmenu={(e) => openMenu(e, NO_CROSSHAIR)}
      title="No crosshair outside bound games"
    >
      <span class="thumb none"><CircleOff size={15} /></span>
      <span class="label">None</span>
    </button>

    {#each crosshairs as c (c.id)}
      <div class="item-wrap" class:active={c.id === selected} class:targeted={menu?.id === c.id}>
        <button
          class="item"
          onclick={() => onselect(c.id)}
          ondblclick={() => startRename(c.id)}
          oncontextmenu={(e) => openMenu(e, c.id)}
          title={c.name}
        >
          <span class="thumb">
            {#if thumbs[c.id]}<img src={thumbs[c.id]} alt="" />{/if}
          </span>
          {#if editing === c.id}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="rename"
              bind:value={draft}
              onblur={commitRename}
              onkeydown={onKey}
              onclick={(e) => e.stopPropagation()}
              autofocus
            />
          {:else}
            <span class="label">{c.name}</span>
          {/if}
        </button>
        {#if c.exe}
          <span class="bound" title="Switches in while {c.exe} is in front"><Link2 size={11} /></span>
        {/if}
      </div>
    {/each}
  </div>

  <div class="add-wrap">
    <button class="new no-drag" onclick={() => (addOpen = !addOpen)} aria-expanded={addOpen} aria-haspopup="true">
      <Plus size={14} /> Add crosshair <ChevronDown size={12} class="chev" />
    </button>
    {#if addOpen}
      <button class="menu-backdrop" aria-label="Close menu" onclick={closeAdd}></button>
      <div class="add-menu" role="menu" aria-label="Add crosshair" tabindex="-1" use:focusOnMount>
        <button class="ctx-item" role="menuitem" onclick={() => (closeAdd(), oncreate())}>
          <FilePlus2 size={14} />
          <span>Blank crosshair</span>
        </button>
        <button class="ctx-item" role="menuitem" onclick={() => (closeAdd(), (binder = { mode: "create" }))}>
          <Gamepad2 size={14} />
          <span>From a running game…</span>
        </button>
      </div>
    {/if}
  </div>
</nav>

{#if menu}
  <button
    class="menu-backdrop"
    aria-label="Close menu"
    onclick={closeMenu}
    oncontextmenu={(e) => {
      e.preventDefault();
      closeMenu();
    }}
  ></button>
  <div
    class="ctxmenu"
    style="left: {menu.x}px; top: {menu.y}px;"
    role="menu"
    aria-label="Crosshair actions"
    tabindex="-1"
    use:focusOnMount
  >
    {#if menu.id === NO_CROSSHAIR}
      <div class="ctx-item locked" role="menuitem" aria-disabled="true">
        <Lock size={14} />
        <span>No crosshair outside games</span>
      </div>
    {:else}
      <button class="ctx-item" role="menuitem" onclick={() => menuAction(startRename)}>
        <Pencil size={14} />
        <span>Rename</span>
      </button>
      <button class="ctx-item" role="menuitem" onclick={() => menuAction(openBindFor)}>
        <Link2 size={14} />
        <span>Bind to game…</span>
      </button>
      {#if find(menu.id)?.exe}
        <button class="ctx-item" role="menuitem" onclick={() => menuAction((id) => onbind(id, null))}>
          <Unlink size={14} />
          <span>Unbind</span>
        </button>
      {/if}
      <button class="ctx-item" role="menuitem" onclick={() => menuAction(onduplicate)}>
        <Copy size={14} />
        <span>Duplicate</span>
      </button>
      <button
        class="ctx-item danger"
        role="menuitem"
        disabled={crosshairs.length <= 1}
        title={crosshairs.length <= 1 ? "Keep at least one crosshair" : undefined}
        onclick={() => menuAction(ondelete)}
      >
        <Trash2 size={14} />
        <span>Delete</span>
      </button>
    {/if}
  </div>
{/if}

{#if binder}
  <ProgramPicker
    heading={binder.mode === "create" ? "Create crosshair from a game" : "Bind crosshair to a game"}
    sub={binder.mode === "create"
      ? "Pick a running game — a new crosshair is made and bound to it."
      : "This crosshair switches in while that game is the active window."}
    onpick={pickProgram}
    onclose={() => (binder = null)}
    {onerror}
  />
{/if}

<style>
  .rail {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 10px;
    width: 168px;
    background: var(--bg-inset);
    border-right: 1px solid var(--border);
    flex-shrink: 0;
    overflow: hidden;
  }
  .list { display: flex; flex-direction: column; gap: 4px; overflow-y: auto; flex: 1; min-height: 0; }
  .item-wrap {
    display: flex;
    align-items: center;
    border-radius: var(--radius);
    border: 1px solid transparent;
    transition: background 120ms ease, border-color 120ms ease;
  }
  .item-wrap:hover { background: var(--surface-hover); }
  .item-wrap .item { flex: 1; min-width: 0; border: none; background: transparent; }
  .item {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 6px 8px;
    border-radius: var(--radius);
    border: 1px solid transparent;
    background: transparent;
    color: var(--fg-muted);
    font: inherit;
    font-size: var(--fs-sm);
    text-align: left;
    cursor: pointer;
    transition: background 120ms ease, border-color 120ms ease, color 120ms ease;
  }
  .item:hover { color: var(--fg-2); }
  .list > .item:hover { background: var(--surface-hover); }
  .item.active,
  .item-wrap.active {
    background: color-mix(in oklab, var(--accent) 12%, transparent);
    border-color: color-mix(in oklab, var(--accent) 38%, transparent);
  }
  .item.active,
  .item-wrap.active .item { color: var(--fg); }
  .item.targeted,
  .item-wrap.targeted { border-color: color-mix(in oklab, var(--accent) 60%, transparent); }
  .thumb {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    flex-shrink: 0;
    border-radius: var(--radius-sm);
    background: radial-gradient(circle at 50% 35%, oklch(0.36 0.008 250), oklch(0.2 0.006 250));
    box-shadow: inset 0 1px 0 color-mix(in oklab, white 10%, transparent), inset 0 0 0 1px oklch(0 0 0 / 0.35);
    overflow: hidden;
  }
  .thumb.none { background: var(--field); color: var(--fg-subtle); }
  .item.active .thumb.none { color: var(--fg-2); }
  .thumb img { width: 100%; height: 100%; }
  .label { flex: 1; min-width: 0; font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rename {
    flex: 1;
    min-width: 0;
    background: var(--field);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-xs);
    color: var(--fg);
    font: inherit;
    font-size: var(--fs-sm);
    padding: 1px 5px;
    outline: none;
  }
  .rename:focus {
    border-color: color-mix(in oklab, var(--accent) 55%, var(--border-strong));
    box-shadow: 0 0 0 2px color-mix(in oklab, var(--accent) 18%, transparent);
  }
  /* Icon-only binding badge (exe lives in the tooltip + the hero line). */
  .bound {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    margin-right: 6px;
    border-radius: 999px;
    background: color-mix(in oklab, var(--accent) 16%, transparent);
    color: color-mix(in oklab, var(--accent) 90%, var(--fg));
    flex-shrink: 0;
  }
  .add-wrap { position: relative; flex-shrink: 0; }
  .new {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 100%;
    padding: 9px 8px;
    border-radius: var(--radius);
    border: 1px solid color-mix(in oklab, var(--accent) 40%, transparent);
    background: color-mix(in oklab, var(--accent) 10%, transparent);
    color: color-mix(in oklab, var(--accent) 92%, var(--fg));
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: 500;
    cursor: pointer;
    transition: background 120ms ease, border-color 120ms ease, color 120ms ease;
  }
  .new:hover {
    background: color-mix(in oklab, var(--accent) 18%, transparent);
    border-color: color-mix(in oklab, var(--accent) 65%, transparent);
    color: var(--fg);
  }
  .new :global(.chev) { margin-left: -1px; opacity: 0.7; }
  .item:focus-visible,
  .new:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--ring);
  }
</style>
