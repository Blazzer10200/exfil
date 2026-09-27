<script lang="ts">
  // Styled replacement for the OS-native tray menu. Rendered in its own
  // frameless transparent always-on-top window ("tray"); the backend shows it
  // at the cursor on tray-icon click and hides it on focus loss.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { getVersion } from "@tauri-apps/api/app";
  import { AppWindow, Crosshair, RotateCcw, Power } from "lucide-svelte";
  import { getCrosshairs, getPresets, NO_CROSSHAIR } from "$lib/api";

  const MARGIN = 12; // transparent gutter so the CSS shadow has room to render
  const win = getCurrentWindow();

  let menuEl: HTMLElement | undefined = $state();
  let version = $state("");
  let openSeq = $state(0); // bumped per open so the entrance animation replays
  let preset = $state("—");
  let xhair = $state("NONE");
  let overlayOn = $state(false);

  function act(action: "show" | "reset" | "quit" | "crosshair") {
    invoke("tray_action", { action }).catch(() => win.hide());
  }

  async function refresh() {
    try {
      const [p, c] = await Promise.all([getPresets(), getCrosshairs()]);
      preset = (p.presets.find((x) => x.slot === p.active)?.name ?? "Normal").toUpperCase();
      overlayOn = c.enabled;
      xhair = c.selected === NO_CROSSHAIR ? "NONE" : (c.crosshairs.find((x) => x.id === c.selected)?.name ?? "NONE").toUpperCase();
    } catch {
      // the menu still works without the readout
    }
  }

  function autofocus(node: HTMLElement) {
    node.focus();
  }

  function onMenuKey(e: KeyboardEvent) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const items = [...(menuEl?.querySelectorAll<HTMLButtonElement>(".item") ?? [])];
    if (!items.length) return;
    const i = items.indexOf(document.activeElement as HTMLButtonElement);
    const next = e.key === "ArrowDown" ? (i + 1) % items.length : (i - 1 + items.length) % items.length;
    items[next].focus();
  }

  onMount(() => {
    getVersion().then((v) => (version = v));
    refresh();
    // Fit the transparent window exactly around the rendered menu.
    if (menuEl) {
      const r = menuEl.getBoundingClientRect();
      win.setSize(new LogicalSize(Math.ceil(r.width) + MARGIN * 2, Math.ceil(r.height) + MARGIN * 2));
    }
    const unlisten = win.listen("tray-open", () => {
      openSeq += 1;
      refresh();
    });
    return () => {
      unlisten.then((u) => u());
    };
  });
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && win.hide()} />

<button class="bd" aria-label="Close menu" onclick={() => win.hide()}></button>

{#key openSeq}
  <div class="tray" role="menu" aria-label="EXFIL tray menu" tabindex="-1" use:autofocus bind:this={menuEl} onkeydown={onMenuKey}>
    <div class="brand">
      <span class="sq"></span>
      <span class="name display">EXFIL</span>
      {#if version}<span class="ver mono">v{version}</span>{/if}
    </div>
    <div class="active mono">
      <span class="k">ACTIVE</span>
      <span class="v">{preset} · {xhair}</span>
    </div>
    <button class="item" role="menuitem" onclick={() => act("show")}><AppWindow size={13} /> SHOW EXFIL</button>
    <button class="item" role="menuitem" onclick={() => act("crosshair")}>
      <Crosshair size={13} /> CROSSHAIR <span class="state" class:on={overlayOn}>{overlayOn ? "ON" : "OFF"}</span>
    </button>
    <button class="item" role="menuitem" onclick={() => act("reset")}><RotateCcw size={13} /> RESET DISPLAY</button>
    <div class="sep"></div>
    <button class="item danger" role="menuitem" onclick={() => act("quit")}><Power size={13} /> QUIT</button>
  </div>
{/key}

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
  }
  .bd {
    position: fixed;
    inset: 0;
    background: transparent;
    border: none;
    padding: 0;
    cursor: default;
  }
  .tray {
    position: fixed;
    top: 12px;
    left: 12px;
    width: 200px;
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: oklch(0.14 0.004 250);
    border: 1px solid var(--hud-line-5);
    border-top: 2px solid var(--accent);
    border-radius: var(--hud-r);
    box-shadow: 0 18px 40px oklch(0 0 0 / 0.6);
    animation: tray-in 130ms var(--ease-hud);
    transform-origin: bottom left;
    outline: none;
  }
  @keyframes tray-in {
    from { opacity: 0; transform: scale(0.96) translateY(4px); }
    to { opacity: 1; transform: none; }
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px 4px;
  }
  .sq {
    width: 8px;
    height: 8px;
    border-radius: 1px;
    background: var(--accent);
    box-shadow: 0 0 8px color-mix(in oklab, var(--accent) 70%, transparent);
  }
  .name {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.14em;
    color: var(--hud-fg-hi);
  }
  .ver {
    margin-left: auto;
    font-size: 10px;
    color: var(--hud-fg-faint);
  }
  .active {
    display: flex;
    gap: 8px;
    padding: 4px 8px 8px;
    font-size: 9.5px;
    letter-spacing: 0.06em;
    border-bottom: 1px solid var(--hud-line-2);
    margin-bottom: 4px;
  }
  .active .k {
    color: var(--hud-fg-faint);
  }
  .active .v {
    color: var(--hud-fg-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 8px;
    border: 0;
    border-radius: var(--hud-r);
    background: transparent;
    font-family: var(--font-display);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    color: var(--hud-fg-2);
    text-align: left;
    cursor: pointer;
  }
  .item:hover,
  .item:focus-visible {
    background: var(--hud-hover);
    color: var(--hud-fg-hi);
    outline: none;
  }
  .item.danger {
    color: var(--danger);
  }
  .item.danger:hover {
    background: var(--danger-soft);
  }
  .state {
    margin-left: auto;
    font-family: var(--font-mono);
    font-size: 9.5px;
    color: var(--hud-fg-faint);
  }
  .state.on {
    color: var(--ok);
  }
  .sep {
    height: 1px;
    margin: 3px 4px;
    background: var(--hud-line-2);
  }
</style>
