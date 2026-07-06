<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { getVersion } from "@tauri-apps/api/app";
  import { Minus, X, Settings } from "lucide-svelte";

  interface Props {
    onsettings: () => void;
  }
  let { onsettings }: Props = $props();

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
