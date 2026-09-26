<script lang="ts">
  import type { Preset } from "./api";
  import { slotAccent } from "./api";
  import { Pencil, Trash2, Lock, Link2, Unlink, Gamepad2, Plus, FilePlus2, ChevronDown } from "lucide-svelte";
  import ProgramPicker from "./ProgramPicker.svelte";

  interface Props {
    presets: Preset[];
    active: string;
    dirty: boolean;
    onselect: (slot: string) => void;
    oncreate: () => void;
    ondelete: (slot: string) => void;
    onrename: (slot: string, name: string) => void;
    onbind: (slot: string, exe: string | null) => void;
    oncreategame: (exe: string, title: string) => void;
    onerror?: (message: string) => void;
  }
  let {
    presets,
    active,
    dirty,
    onselect,
    oncreate,
    ondelete,
    onrename,
    onbind,
    oncreategame,
    onerror,
  }: Props = $props();

  // Accent index = position among non-Normal presets (Normal is fixed grey).
  function accentIndex(slot: string): number {
    let i = 0;
    for (const p of presets) {
      if (p.slot === "Normal") continue;
      if (p.slot === slot) return i;
      i++;
    }
    return 0;
  }

  let editing = $state<string | null>(null);
  let draft = $state("");

  function startRename(slot: string) {
    if (slot === "Normal") return;
    const p = presets.find((x) => x.slot === slot);
    if (!p) return;
    editing = slot;
    draft = p.name;
  }
  function commitRename() {
    if (editing === null) return;
    const slot = editing;
    const name = draft.trim();
    editing = null;
    const p = presets.find((x) => x.slot === slot);
    if (name && p && name !== p.name) onrename(slot, name);
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
  // Width is fixed; height varies with the slot's items (locked = 1,
  // unbound = 3, bound = 4), so estimate per-item when clamping against the
  // viewport so the tallest variant never spills off-screen.
  const MENU_W = 180;
  let menu = $state<{ slot: string; x: number; y: number } | null>(null);

  function menuHeight(slot: string): number {
    const items = slot === "Normal" ? 1 : boundExe(slot) ? 4 : 3;
    return 12 + items * 36;
  }
  function openMenu(e: MouseEvent, slot: string) {
    e.preventDefault();
    const x = Math.min(e.clientX, window.innerWidth - MENU_W - 8);
    const y = Math.min(e.clientY, window.innerHeight - menuHeight(slot) - 8);
    menu = { slot, x: Math.max(8, x), y: Math.max(8, y) };
  }
  function closeMenu() {
    menu = null;
  }
  function menuRename() {
    if (menu) startRename(menu.slot);
    closeMenu();
  }
  function menuDelete() {
    if (menu) ondelete(menu.slot);
    closeMenu();
  }

  // ── Program binding ──
  // "Bind to program" opens a chooser: browse for an .exe via the OS file
  // dialog, OR pick from the live running-process list. Binding stores the exe
  // basename; the backend watcher auto-applies this preset when it runs.
  // Binder modal serves two intents:
  //  - "bind":   attach a program to an existing slot (right-click → Bind)
  //  - "create": make a NEW preset straight from a running game (rail button)
  // (The chooser UI itself is the shared ProgramPicker modal.)
  let binder = $state<
    { mode: "bind"; slot: string } | { mode: "create" } | null
  >(null);

  function boundExe(slot: string): string | null {
    return presets.find((p) => p.slot === slot)?.exe ?? null;
  }

  function openBinder() {
    if (!menu) return;
    const slot = menu.slot;
    closeMenu();
    binder = { mode: "bind", slot };
  }

  function openCreateFromGame() {
    binder = { mode: "create" };
  }
  // Open the binder for a slot from outside the rail (main-panel "Bind…" link).
  export async function openBindFor(slot: string) {
    if (slot === "Normal") return;
    binder = { mode: "bind", slot };
  }
  function closeBinder() {
    binder = null;
  }
  function pickProgram(exe: string, title: string) {
    if (!binder) return;
    if (binder.mode === "create") {
      oncreategame(exe, title);
    } else {
      onbind(binder.slot, exe);
    }
  }
  function menuUnbind() {
    if (menu) onbind(menu.slot, null);
    closeMenu();
  }

  // ── Add-preset popover ──
  // Single entry point for both preset-creation flows: a blank preset, or one
  // seeded from a running program. Replaces two separate rail buttons.
  let addOpen = $state(false);
  function toggleAdd() {
    addOpen = !addOpen;
  }
  function closeAdd() {
    addOpen = false;
  }
  function addBlank() {
    closeAdd();
    oncreate();
  }
  function addFromProgram() {
    closeAdd();
    openCreateFromGame();
  }

  // Moves focus into a just-opened menu/dialog so Escape/keyboard nav work
  // without requiring a prior click inside it.
  function focusOnMount(node: HTMLElement) {
    node.focus();
  }
</script>

<svelte:window
  onkeydown={(e) => e.key === "Escape" && (closeMenu(), closeAdd(), closeBinder())}
  onblur={() => (closeMenu(), closeAdd())}
/>

<nav class="rail">
  <div class="slots" onscroll={closeMenu}>
    {#each presets as p (p.slot)}
      <div
        class="slot"
        class:active={p.slot === active}
        class:targeted={menu?.slot === p.slot}
        style="--slot-accent: {slotAccent(p.slot, accentIndex(p.slot))}"
      >
        <button
          class="pick"
          onclick={() => onselect(p.slot)}
          ondblclick={() => startRename(p.slot)}
          oncontextmenu={(e) => openMenu(e, p.slot)}
          title={p.slot === "Normal" ? "Native baseline" : p.name}
        >
          <span class="dot" class:dirty={p.slot === active && dirty}></span>
          {#if editing === p.slot}
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
            <span class="label">{p.name}</span>
          {/if}
        </button>
        {#if p.slot !== "Normal" && p.exe}
          <span class="bound" title="Auto-switches when {p.exe} is running">
            <Link2 size={11} />
          </span>
        {/if}
      </div>
    {/each}
  </div>

  <div class="add-wrap">
    <button class="new no-drag" onclick={toggleAdd} aria-expanded={addOpen} aria-haspopup="true">
      <Plus size={14} /> Add preset <ChevronDown size={12} class="chev" />
    </button>
    {#if addOpen}
      <button class="menu-backdrop" aria-label="Close menu" onclick={closeAdd}></button>
      <div class="add-menu" role="menu" aria-label="Add preset" tabindex="-1" use:focusOnMount>
        <button class="ctx-item" role="menuitem" onclick={addBlank}>
          <FilePlus2 size={14} />
          <span>Blank preset</span>
        </button>
        <button class="ctx-item" role="menuitem" onclick={addFromProgram}>
          <Gamepad2 size={14} />
          <span>From a running program…</span>
        </button>
      </div>
    {/if}
  </div>
</nav>

{#if menu}
  <!-- Backdrop swallows the next click so the menu dismisses cleanly. -->
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
    aria-label="Preset actions"
    tabindex="-1"
    use:focusOnMount
  >
    {#if menu.slot === "Normal"}
      <div class="ctx-item locked" role="menuitem" aria-disabled="true">
        <Lock size={14} />
        <span>Native baseline</span>
      </div>
    {:else}
      <button class="ctx-item" role="menuitem" onclick={menuRename}>
        <Pencil size={14} />
        <span>Rename</span>
      </button>
      <button class="ctx-item" role="menuitem" onclick={openBinder}>
        <Link2 size={14} />
        <span>Bind to program…</span>
      </button>
      {#if boundExe(menu.slot)}
        <button class="ctx-item" role="menuitem" onclick={menuUnbind}>
          <Unlink size={14} />
          <span>Unbind</span>
        </button>
      {/if}
      <button class="ctx-item danger" role="menuitem" onclick={menuDelete}>
        <Trash2 size={14} />
        <span>Delete</span>
      </button>
    {/if}
  </div>
{/if}

{#if binder}
  <ProgramPicker
    heading={binder.mode === "create" ? "Create preset from a game" : "Bind a program"}
    sub={binder.mode === "create"
      ? "Pick a running game — a new preset is made and auto-bound to it."
      : "Auto-applies this preset while the program runs."}
    onpick={pickProgram}
    onclose={closeBinder}
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
  .slots {
    display: flex;
    flex-direction: column;
    gap: 4px;
    overflow-y: auto;
    flex: 1;
    min-height: 0;
  }
  .slot {
    display: flex;
    align-items: center;
    border-radius: var(--radius);
    border: 1px solid transparent;
    transition: background 120ms ease, border-color 120ms ease;
  }
  .slot:hover { background: var(--surface-hover); }
  .slot.active {
    background: color-mix(in oklab, var(--slot-accent) 14%, transparent);
    border-color: color-mix(in oklab, var(--slot-accent) 40%, transparent);
  }
  .slot.targeted {
    border-color: color-mix(in oklab, var(--slot-accent) 60%, transparent);
  }
  .pick {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
    min-width: 0;
    padding: 9px 11px;
    background: transparent;
    border: none;
    color: var(--fg-muted);
    font: inherit;
    font-size: var(--fs-sm);
    cursor: pointer;
    text-align: left;
  }
  .slot.active .pick { color: var(--fg); }
  .slot:hover .pick { color: var(--fg-2); }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--slot-accent);
    flex-shrink: 0;
    box-shadow: 0 0 0 0 var(--slot-accent);
    transition: box-shadow 200ms ease;
  }
  .slot.active .dot {
    box-shadow: 0 0 8px 1px color-mix(in oklab, var(--slot-accent) 70%, transparent);
  }
  .dot.dirty {
    background: var(--warn);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--warn) 25%, transparent);
    animation: dirty-pulse 1600ms ease-in-out infinite;
  }
  @keyframes dirty-pulse {
    0%, 100% { box-shadow: 0 0 0 3px color-mix(in oklab, var(--warn) 25%, transparent); }
    50% { box-shadow: 0 0 0 5px color-mix(in oklab, var(--warn) 12%, transparent); }
  }
  .label {
    font-weight: 500;
    letter-spacing: 0.01em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
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
  .add-wrap { position: relative; flex-shrink: 0; }
  .new {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 100%;
    padding: 9px 8px;
    border-radius: var(--radius);
    border: 1px solid color-mix(in oklab, var(--accent, var(--slot-a)) 40%, transparent);
    background: color-mix(in oklab, var(--accent, var(--slot-a)) 10%, transparent);
    color: color-mix(in oklab, var(--accent, var(--slot-a)) 92%, var(--fg));
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: 500;
    cursor: pointer;
    transition: background 120ms ease, color 120ms ease, border-color 120ms ease;
  }
  .new:hover {
    background: color-mix(in oklab, var(--accent, var(--slot-a)) 18%, transparent);
    border-color: color-mix(in oklab, var(--accent, var(--slot-a)) 65%, transparent);
    color: var(--fg);
  }
  .new :global(.chev) { margin-left: -1px; opacity: 0.7; }
  .pick:focus-visible,
  .new:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--ring);
  }

  /* ── Bound-program badge ──
     Icon-only so it never crowds out the preset name (the exe lives in the
     tooltip and the main panel's auto-switch block). */
  .bound {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    margin-right: 8px;
    border-radius: 999px;
    background: color-mix(in oklab, var(--slot-accent) 16%, transparent);
    color: color-mix(in oklab, var(--slot-accent) 90%, var(--fg));
    flex-shrink: 0;
  }
</style>
