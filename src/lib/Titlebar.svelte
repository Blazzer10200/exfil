<script lang="ts" module>
  export type View = "color" | "crosshair";
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { getVersion } from "@tauri-apps/api/app";
  import { Minus, X, Settings, Palette, Crosshair } from "lucide-svelte";

  interface Props {
    view: View;
    onview: (v: View) => void;
    onsettings: () => void;
  }
  let { view, onview, onsettings }: Props = $props();

  const appWindow = getCurrentWindow();

  // Real app version for the titlebar chip; static "v2" until resolved.
  let ver = $state("v2");
  onMount(async () => {
    try {
      ver = `v${await getVersion()}`;
    } catch {
      // cosmetic — keep the static fallback
    }
  });
</script>

<div class="titlebar drag">
  <div class="brand">
    <img class="mark" src="/favicon.png" alt="EXFIL" />
    <span class="name">EXFIL</span>
    <span class="ver mono">{ver}</span>
  </div>
  <div class="tabs no-drag" role="tablist" aria-label="Section">
    <button class="tab" class:on={view === "color"} role="tab" aria-selected={view === "color"} onclick={() => onview("color")}>
      <Palette size={13} /> Color
    </button>
    <button class="tab" class:on={view === "crosshair"} role="tab" aria-selected={view === "crosshair"} onclick={() => onview("crosshair")}>
      <Crosshair size={13} /> Crosshairs
    </button>
  </div>
  <div class="controls no-drag">
    <button class="winbtn" title="Settings" aria-label="Settings" onclick={onsettings}>
      <Settings size={15} />
    </button>
    <button class="winbtn" title="Minimize" aria-label="Minimize" onclick={() => appWindow.minimize()}>
      <Minus size={15} />
    </button>
    <button class="winbtn close" title="Close to tray" aria-label="Close to tray" onclick={() => appWindow.hide()}>
      <X size={15} />
    </button>
  </div>
</div>

<style>
  .titlebar {
    height: var(--titlebar-h);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 4px 0 12px;
    background: var(--bg-inset);
    border-bottom: 1px solid var(--border);
    user-select: none;
    flex-shrink: 0;
  }
  .brand { display: flex; align-items: center; gap: 8px; }
  .mark {
    width: 18px;
    height: 18px;
    border-radius: var(--radius-xs);
    object-fit: contain;
  }
  .name {
    font-size: var(--fs-sm);
    font-weight: 600;
    letter-spacing: 0.08em;
    color: var(--fg-2);
  }
  .ver { font-size: var(--fs-xs); color: var(--fg-faint); }
  .brand, .controls { flex: 1; }
  .controls { justify-content: flex-end; }
  .tabs {
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: var(--radius);
    background: var(--field);
    border: 1px solid var(--border);
  }
  .tab {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 12px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--fg-muted);
    font: inherit;
    font-size: var(--fs-xs);
    font-weight: 500;
    cursor: pointer;
    transition: background 120ms ease, color 120ms ease, box-shadow 120ms ease;
  }
  .tab:hover { color: var(--fg); }
  .tab.on {
    color: var(--fg);
    background: color-mix(in oklab, var(--accent) 16%, var(--bg-elev-2));
    box-shadow: inset 0 1px 0 color-mix(in oklab, white 6%, transparent), 0 0 10px color-mix(in oklab, var(--accent) 18%, transparent);
  }
  .tab:focus-visible { outline: none; box-shadow: 0 0 0 2px var(--ring); }
  .controls { display: flex; align-items: center; gap: 2px; height: 100%; }
  .winbtn {
    display: grid;
    place-items: center;
    width: 38px;
    height: calc(var(--titlebar-h) - 1px);
    border: none;
    background: transparent;
    color: var(--fg-muted);
    cursor: pointer;
    transition: background 100ms ease, color 100ms ease;
  }
  .winbtn:hover { background: var(--surface-hover); color: var(--fg); }
  .winbtn.close:hover { background: var(--danger); color: white; }
  .winbtn:focus-visible {
    outline: none;
    box-shadow: inset 0 0 0 2px var(--ring);
  }
</style>
