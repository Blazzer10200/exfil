<script lang="ts">
  // Crosshair part panel. On: amber title + toggle + body. Off: a single
  // dashed row that expands when toggled. `always` panels have no toggle.
  import type { Snippet } from "svelte";

  interface Props {
    title: string;
    on?: boolean;
    always?: boolean;
    tone?: "accent" | "telemetry";
    ontoggle?: (on: boolean) => void;
    children: Snippet;
  }
  let { title, on = true, always = false, tone = "accent", ontoggle, children }: Props = $props();
  const open = $derived(always || on);
</script>

<section class="xp" class:open class:cyan={tone === "telemetry"}>
  <header class="head" class:clickable={!always}>
    {#if always}
      <span class="title display">{title}</span>
    {:else}
      <button class="title display tbtn" onclick={() => ontoggle?.(!on)}>{title}</button>
      <button class="toggle" class:on aria-label="{title} on/off" onclick={() => ontoggle?.(!on)}></button>
    {/if}
  </header>
  {#if open}
    <div class="body">{@render children()}</div>
  {/if}
</section>

<style>
  .xp {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 9px 12px 10px;
    border: 1px dashed var(--hud-line-4);
    border-radius: var(--hud-r);
    background: transparent;
    transition: background 170ms var(--ease-hud), border-color 170ms var(--ease-hud);
  }
  .xp.open {
    border-style: solid;
    border-color: var(--hud-line-2);
    background: var(--hud-panel);
  }
  .xp.cyan.open {
    background: oklch(0.8 0.12 215 / 0.05);
    border-color: color-mix(in oklab, var(--telemetry) 25%, var(--hud-line-2));
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 14px;
  }
  .title {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.14em;
    color: var(--hud-fg-low);
    transition: color 170ms var(--ease-hud);
  }
  .open .title {
    color: var(--accent);
  }
  .cyan.open .title {
    color: var(--telemetry-text);
  }
  .tbtn {
    padding: 0;
    border: 0;
    background: transparent;
    cursor: pointer;
    text-align: left;
    flex: 1;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 6px;
    animation: hud-fade 170ms var(--ease-hud);
  }
</style>
