<script lang="ts">
  import type { Preset } from "./api";
  import { slotAccent } from "./api";
  import { Pencil, Trash2, Lock } from "lucide-svelte";

  interface Props {
    presets: Preset[];
    active: string;
    onselect: (slot: string) => void;
    oncreate: () => void;
    ondelete: (slot: string) => void;
    onrename: (slot: string, name: string) => void;
  }
  let { presets, active, onselect, oncreate, ondelete, onrename }: Props =
    $props();

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
  // Menu width/height are fixed enough to clamp against the viewport so it
  // never spills off-screen near the window edges.
  const MENU_W = 168;
  const MENU_H = 92;
  let menu = $state<{ slot: string; x: number; y: number } | null>(null);

  function openMenu(e: MouseEvent, slot: string) {
    e.preventDefault();
    const x = Math.min(e.clientX, window.innerWidth - MENU_W - 8);
    const y = Math.min(e.clientY, window.innerHeight - MENU_H - 8);
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
</script>

<svelte:window
  onkeydown={(e) => e.key === "Escape" && closeMenu()}
  onblur={closeMenu}
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
          <span class="dot"></span>
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
      </div>
    {/each}
  </div>

  <button class="new no-drag" onclick={oncreate} title="Create a new preset">
    <span class="plus">+</span> New preset
  </button>
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
  <div class="ctxmenu" style="left: {menu.x}px; top: {menu.y}px;" role="menu">
    {#if menu.slot === "Normal"}
      <div class="ctx-item locked" role="menuitem" aria-disabled="true">
        <Lock size={14} />
        <span>Native baseline — locked</span>
      </div>
    {:else}
      <button class="ctx-item" role="menuitem" onclick={menuRename}>
        <Pencil size={14} />
        <span>Rename</span>
      </button>
      <button class="ctx-item danger" role="menuitem" onclick={menuDelete}>
        <Trash2 size={14} />
        <span>Delete</span>
      </button>
    {/if}
  </div>
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
  .rename:focus { border-color: var(--border-focus); }
  .new {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 8px;
    border-radius: var(--radius);
    border: 1px dashed var(--border-strong);
    background: transparent;
    color: var(--fg-muted);
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: 500;
    cursor: pointer;
    flex-shrink: 0;
    transition: background 120ms ease, color 120ms ease, border-color 120ms ease;
  }
  .new:hover {
    background: var(--surface-hover);
    color: var(--fg);
    border-color: var(--fg-faint);
  }
  .plus { font-size: 15px; line-height: 1; }

  /* ── Context menu ── */
  .menu-backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: transparent;
    border: none;
    padding: 0;
    cursor: default;
  }
  .ctxmenu {
    position: fixed;
    z-index: 50;
    width: 168px;
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: var(--bg-elev-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    animation: ctx-in 90ms ease;
    transform-origin: top left;
  }
  @keyframes ctx-in {
    from { opacity: 0; transform: scale(0.96) translateY(-2px); }
    to { opacity: 1; transform: scale(1) translateY(0); }
  }
  .ctx-item {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    padding: 7px 9px;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--fg-2);
    font: inherit;
    font-size: var(--fs-sm);
    text-align: left;
    cursor: pointer;
    transition: background 100ms ease, color 100ms ease;
  }
  .ctx-item:hover {
    background: var(--surface-hover);
    color: var(--fg);
  }
  .ctx-item.danger:hover {
    background: var(--danger-soft);
    color: var(--danger);
  }
  .ctx-item.locked {
    color: var(--fg-faint);
    cursor: default;
    font-size: var(--fs-xs);
  }
  .ctx-item.locked:hover { background: transparent; color: var(--fg-faint); }
</style>
