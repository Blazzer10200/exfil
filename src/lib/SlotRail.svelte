<script lang="ts">
  import type { Preset } from "./api";
  import { slotAccent } from "./api";

  interface Props {
    presets: Preset[];
    active: string;
    onselect: (slot: string) => void;
  }
  let { presets, active, onselect }: Props = $props();
</script>

<nav class="rail">
  {#each presets as p (p.slot)}
    <button
      class="slot"
      class:active={p.slot === active}
      style="--slot-accent: {slotAccent(p.slot)}"
      onclick={() => onselect(p.slot)}
      title={p.name}
    >
      <span class="dot"></span>
      <span class="label">{p.name}</span>
    </button>
  {/each}
</nav>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px 10px;
    width: 168px;
    background: var(--bg-inset);
    border-right: 1px solid var(--border);
    flex-shrink: 0;
    overflow-y: auto;
  }
  .slot {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 11px;
    border-radius: var(--radius);
    border: 1px solid transparent;
    background: transparent;
    color: var(--fg-muted);
    font: inherit;
    font-size: var(--fs-sm);
    cursor: pointer;
    text-align: left;
    transition: background 120ms ease, color 120ms ease, border-color 120ms ease;
  }
  .slot:hover { background: var(--surface-hover); color: var(--fg-2); }
  .slot.active {
    background: color-mix(in oklab, var(--slot-accent) 14%, transparent);
    border-color: color-mix(in oklab, var(--slot-accent) 40%, transparent);
    color: var(--fg);
  }
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
  .label { font-weight: 500; letter-spacing: 0.01em; }
</style>
