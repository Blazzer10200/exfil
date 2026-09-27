<script lang="ts">
  // Inline rename: 24px amber-ringed input, uppercase display, n / max
  // counter, SAVE ↵ / ESC chips. Enter saves, Escape cancels, blur cancels.
  interface Props {
    value: string;
    max?: number;
    onsave: (name: string) => void;
    oncancel: () => void;
  }
  let { value, max = 24, onsave, oncancel }: Props = $props();
  // svelte-ignore state_referenced_locally
  let draft = $state(value);
  const clean = $derived(draft.trim().slice(0, max));

  function save() {
    if (!clean || clean === value) return oncancel();
    onsave(clean);
  }
  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      save();
    } else if (e.key === "Escape") {
      e.preventDefault();
      oncancel();
    }
  }
  function focus(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
  // Blur cancels unless focus moved to one of our chips.
  let root = $state<HTMLElement>();
  function onfocusout(e: FocusEvent) {
    if (root && e.relatedTarget instanceof Node && root.contains(e.relatedTarget)) return;
    oncancel();
  }
</script>

<div class="rename" bind:this={root} {onfocusout}>
  <input class="field name" bind:value={draft} maxlength={max} spellcheck="false" use:focus {onkeydown} />
  <span class="count mono">{draft.length} / {max}</span>
  <button class="chip xs accent" onmousedown={(e) => e.preventDefault()} onclick={save}>SAVE ↵</button>
  <button class="chip xs" onmousedown={(e) => e.preventDefault()} onclick={oncancel}>ESC</button>
</div>

<style>
  .rename {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .name {
    height: 24px;
    width: 180px;
    font-family: var(--font-display);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--accent) 15%, transparent);
  }
  .count {
    font-size: 9px;
    color: var(--hud-fg-faint);
    white-space: nowrap;
  }
</style>
