<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { getVersion } from "@tauri-apps/api/app";
  import { Minus, X, MoreVertical, Download, Upload, Power, Check } from "lucide-svelte";
  import { getAutostart, setAutostart } from "./api";

  interface Props {
    onimport: () => void;
    onexport: () => void;
    onerror?: (message: string) => void;
  }
  let { onimport, onexport, onerror }: Props = $props();

  const appWindow = getCurrentWindow();

  // Start-with-Windows toggle — backed by the persisted store preference.
  let autostart = $state(true);
  // Real app version for the titlebar chip; static "v2" until resolved.
  let ver = $state("v2");
  onMount(async () => {
    try {
      ver = `v${await getVersion()}`;
    } catch {
      // cosmetic — keep the static fallback
    }
    try {
      autostart = await getAutostart();
    } catch (e) {
      onerror?.(String(e));
    }
  });
  async function toggleAutostart() {
    try {
      autostart = await setAutostart(!autostart);
    } catch (e) {
      onerror?.(String(e));
    }
  }

  let menuOpen = $state(false);
  function toggleMenu() {
    menuOpen = !menuOpen;
  }
  function closeMenu() {
    menuOpen = false;
  }
  function doImport() {
    closeMenu();
    onimport();
  }
  function doExport() {
    closeMenu();
    onexport();
  }
  function focusOnMount(node: HTMLElement) {
    node.focus();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && closeMenu()} />

<div class="titlebar drag">
  <div class="brand">
    <img class="mark" src="/favicon.png" alt="EXFIL" />
    <span class="name">EXFIL</span>
    <span class="ver mono">{ver}</span>
  </div>
  <div class="controls no-drag">
    <div class="menu-wrap">
      <button
        class="winbtn"
        title="More options"
        aria-label="More options"
        aria-haspopup="true"
        aria-expanded={menuOpen}
        onclick={toggleMenu}
      >
        <MoreVertical size={15} />
      </button>
      {#if menuOpen}
        <button class="menu-backdrop" aria-label="Close menu" onclick={closeMenu}></button>
        <div class="dropdown" role="menu" aria-label="More options" tabindex="-1" use:focusOnMount>
          <button
            class="dd-item"
            role="menuitemcheckbox"
            aria-checked={autostart}
            onclick={toggleAutostart}
          >
            <Power size={14} />
            <span>Start with Windows</span>
            {#if autostart}<Check size={13} class="on" />{/if}
          </button>
          <div class="dd-sep"></div>
          <button class="dd-item" role="menuitem" onclick={doImport}>
            <Download size={14} />
            <span>Import presets…</span>
          </button>
          <button class="dd-item" role="menuitem" onclick={doExport}>
            <Upload size={14} />
            <span>Export presets…</span>
          </button>
        </div>
      {/if}
    </div>
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

  .menu-wrap { position: relative; height: 100%; }
  .menu-backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: transparent;
    border: none;
    padding: 0;
    cursor: default;
  }
  .dropdown {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    z-index: 50;
    width: 190px;
    padding: 5px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: linear-gradient(180deg, var(--bg-elev-3), var(--bg-elev-2));
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg), inset 0 1px 0 color-mix(in oklab, white 5%, transparent);
    animation: dd-in 110ms var(--ease-soft);
    transform-origin: top right;
  }
  @keyframes dd-in {
    from { opacity: 0; transform: scale(0.95) translateY(-3px); }
    to { opacity: 1; transform: scale(1) translateY(0); }
  }
  .dd-item {
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
    transition: background 110ms ease, color 110ms ease;
  }
  .dd-item:hover { background: var(--surface-hover); color: var(--fg); }
  /* Programmatic container focus shouldn't draw the UA ring; keyboard focus
     on the items themselves gets the app ring token instead. */
  .dropdown:focus { outline: none; }
  .winbtn:focus-visible,
  .dd-item:focus-visible {
    outline: none;
    box-shadow: inset 0 0 0 2px var(--ring);
  }
  .dd-item span { flex: 1; }
  .dd-item :global(.on) { flex-shrink: 0; color: var(--accent); }
  .dd-sep { height: 1px; margin: 3px 6px; background: var(--border); }
</style>
