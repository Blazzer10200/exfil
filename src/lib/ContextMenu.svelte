<script lang="ts">
  // Right-click menu anchored at (x, y), clamped to the viewport. Closes on
  // Escape, backdrop click, scroll, or blur.
  import { tick } from "svelte";
  import type { IconComponent } from "./types";

  export type MenuItem = {
    label: string;
    icon?: IconComponent;
    danger?: boolean;
    disabled?: boolean;
    on?: boolean;
    sep?: boolean;
    run?: () => void;
  };

  interface Props {
    x: number;
    y: number;
    items: MenuItem[];
    onclose: () => void;
  }
  let { x, y, items, onclose }: Props = $props();

  let el = $state<HTMLElement>();
  // svelte-ignore state_referenced_locally
  let left = $state(x);
  // svelte-ignore state_referenced_locally
  let top = $state(y);

  $effect(() => {
    void x;
    void y;
    tick().then(() => {
      if (!el) return;
      const r = el.getBoundingClientRect();
      left = Math.min(x, window.innerWidth - r.width - 6);
      top = Math.min(y, window.innerHeight - r.height - 6);
      el.focus();
    });
  });

  function pick(it: MenuItem) {
    if (it.disabled) return;
    onclose();
    it.run?.();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} onscroll={onclose} onresize={onclose} />

<button class="backdrop clear" aria-label="Close menu" onclick={onclose} oncontextmenu={(e) => { e.preventDefault(); onclose(); }}></button>
<div class="menu fixed" role="menu" tabindex="-1" bind:this={el} style="left: {left}px; top: {top}px">
  {#each items as it}
    {#if it.sep}
      <div class="menu-sep"></div>
    {:else}
      <button class="menu-item" class:danger={it.danger} class:on={it.on} role="menuitem" disabled={it.disabled} onclick={() => pick(it)}>
        {#if it.icon}<it.icon size={13} />{/if}
        {it.label}
      </button>
    {/if}
  {/each}
</div>
