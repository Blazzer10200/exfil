<script lang="ts">
  // 26px telemetry bar: vendor · ramp · in-front (or the current toast) · hotkeys.
  import { app } from "./state.svelte";
  import { toast } from "./toast.svelte";

  const vendorText = $derived(
    app.vendor === "nvidia" ? "NVIDIA · DVC READY" : app.vendor === "amd" ? "AMD · ADL READY" : "GAMMA ONLY · NO DVC",
  );
  const t = $derived(toast.current);
</script>

<footer class="status mono" class:error={t?.kind === "error"}>
  <span class="item">
    <span class="dot" class:cyan={!!app.vendor} class:grey={!app.vendor}></span>
    {vendorText}
  </span>
  <span class="item">
    <span class="dot green"></span>
    RAMP ASSERTED
  </span>
  {#if t}
    {#key t}
      <span class="item toast {t.kind}">
        {#if t.spin}<span class="spin">◌</span>{/if}
        {t.spin ? t.msg.replace(/^◌\s*/, "") : t.msg}
        {#if t.action}
          <button class="action" onclick={t.action.run}>{t.action.label}</button>
        {/if}
      </span>
    {/key}
  {:else}
    <span class="item" class:ok={app.inFrontBound}>
      IN FRONT: {app.inFront ? app.inFront.toUpperCase() : "DESKTOP"}
    </span>
  {/if}
  <span class="grow"></span>
  {#if app.hotkeys}
    <span class="item keys">
      HOTKEYS <b>F9</b> · <b>F10</b> · <b>F11</b> · <b>F12</b>
    </span>
  {/if}
</footer>

<style>
  .status {
    height: 26px;
    flex: 0 0 26px;
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 0 16px;
    border-top: 1px solid var(--hud-line);
    font-size: 10px;
    letter-spacing: 0.04em;
    color: var(--hud-fg-low);
    white-space: nowrap;
    overflow: hidden;
    transition: background 200ms var(--ease-hud);
  }
  .status.error {
    background: color-mix(in oklab, var(--danger) 6%, transparent);
  }
  .item {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .item.ok {
    color: var(--ok);
  }
  .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
  }
  .dot.cyan {
    background: var(--telemetry);
    box-shadow: 0 0 6px color-mix(in oklab, var(--telemetry) 70%, transparent);
  }
  .dot.green {
    background: var(--ok);
    box-shadow: 0 0 6px color-mix(in oklab, var(--ok) 60%, transparent);
  }
  .dot.grey {
    background: var(--hud-fg-faint);
  }
  .toast {
    animation: hud-fade 160ms var(--ease-hud);
  }
  .toast.ok {
    color: var(--ok);
  }
  .toast.info {
    color: var(--accent);
  }
  .toast.error {
    color: var(--danger);
  }
  .spin {
    display: inline-block;
    animation: hud-spin 900ms linear infinite;
  }
  .action {
    margin-left: 6px;
    padding: 0;
    border: 0;
    background: transparent;
    font: inherit;
    letter-spacing: 0.06em;
    color: var(--danger);
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
  }
  .grow {
    flex: 1;
  }
  .keys b {
    font-weight: 500;
    color: var(--hud-fg-3);
  }
</style>
