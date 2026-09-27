<script lang="ts">
  // 32px drag bar: `EXFIL // SECTION` breadcrumb + version, minimize / close
  // (close hides to tray — the backend's CloseRequested handler owns that).
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { getVersion } from "@tauri-apps/api/app";
  import { Minus, X } from "lucide-svelte";
  import { app } from "./state.svelte";

  const win = getCurrentWindow();
  let version = $state("");
  onMount(() => {
    getVersion().then((v) => (version = v)).catch(() => {});
  });

  const crumbs = $derived.by(() => {
    switch (app.view) {
      case "color":
        return ["COLOR"];
      case "crosshair":
        return ["CROSSHAIR"];
      case "library":
        return ["CROSSHAIR", "LIBRARY"];
      case "games":
        return ["GAMES"];
      case "settings":
        return ["SETTINGS"];
    }
  });
</script>

<header class="titlebar drag" data-tauri-drag-region>
  <span class="crumb display" data-tauri-drag-region>
    EXFIL
    {#each crumbs as c}
      <span class="sep">//</span>{c}
    {/each}
    {#if version}<span class="ver mono">v{version}</span>{/if}
  </span>
  <span class="grow" data-tauri-drag-region></span>
  <button class="wc no-drag" aria-label="Minimize" title="Minimize" onclick={() => win.minimize()}>
    <Minus size={14} />
  </button>
  <button class="wc close no-drag" aria-label="Close" title="Hide to tray" onclick={() => win.hide()}>
    <X size={14} />
  </button>
</header>

<style>
  .titlebar {
    height: 32px;
    flex: 0 0 32px;
    display: flex;
    align-items: stretch;
    padding-left: 16px;
    border-bottom: 1px solid var(--hud-line);
  }
  .crumb {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.18em;
    color: var(--hud-fg-muted);
    white-space: nowrap;
  }
  .sep {
    color: var(--hud-fg-ghost);
  }
  .ver {
    margin-left: 10px;
    font-size: 11px;
    font-weight: 400;
    letter-spacing: 0;
    color: var(--hud-fg-faint);
  }
  .grow {
    flex: 1;
  }
  .wc {
    width: 40px;
    display: grid;
    place-items: center;
    border: 0;
    background: transparent;
    color: oklch(0.58 0.012 258);
    cursor: pointer;
    transition: background 120ms var(--ease-hud), color 120ms var(--ease-hud);
  }
  .wc:hover {
    background: var(--hud-titlebar-hover);
    color: var(--hud-fg);
  }
  .wc.close:hover {
    background: var(--danger);
    color: #fff;
  }
</style>
