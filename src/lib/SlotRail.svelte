<script lang="ts">
  import type { Preset } from "./api";
  import { slotAccent } from "./api";

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

  function startRename(p: Preset) {
    if (p.slot === "Normal") return;
    editing = p.slot;
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
</script>

<nav class="rail">
  <div class="slots">
    {#each presets as p (p.slot)}
      <div
        class="slot"
        class:active={p.slot === active}
        style="--slot-accent: {slotAccent(p.slot, accentIndex(p.slot))}"
      >
        <button
          class="pick"
          onclick={() => onselect(p.slot)}
          ondblclick={() => startRename(p)}
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
        {#if p.slot !== "Normal" && editing !== p.slot}
          <button
            class="trash"
            title="Delete preset"
            onclick={() => ondelete(p.slot)}
            aria-label="Delete {p.name}"
          >
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2m3 0v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
            </svg>
          </button>
        {/if}
      </div>
    {/each}
  </div>

  <button class="new no-drag" onclick={oncreate} title="Create a new preset">
    <span class="plus">+</span> New preset
  </button>
</nav>

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
  .trash {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 30px;
    flex-shrink: 0;
    background: transparent;
    border: none;
    color: var(--fg-faint);
    cursor: pointer;
    opacity: 0;
    transition: opacity 120ms ease, color 120ms ease;
  }
  .slot:hover .trash { opacity: 1; }
  .trash:hover { color: var(--danger); }
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
</style>
