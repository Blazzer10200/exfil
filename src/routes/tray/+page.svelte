<script lang="ts">
  // Styled replacement for the OS-native tray menu. Rendered in its own
  // frameless transparent always-on-top window ("tray"); the backend shows it
  // at the cursor on tray-icon click and hides it on focus loss.
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { getVersion } from "@tauri-apps/api/app";
  import { AppWindow, RotateCcw, Power } from "lucide-svelte";

  const MARGIN = 12; // transparent gutter so the CSS shadow has room to render
  const win = getCurrentWindow();

  let menuEl: HTMLElement | undefined = $state();
  let version = $state("");
  let openSeq = $state(0); // bumped per open so the entrance animation replays

  function act(action: "show" | "reset" | "quit") {
    invoke("tray_action", { action }).catch(() => win.hide());
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
    const next =
      e.key === "ArrowDown" ? (i + 1) % items.length : (i - 1 + items.length) % items.length;
    items[next].focus();
  }

  onMount(() => {
    getVersion().then((v) => (version = v));
    // Fit the transparent window exactly around the rendered menu.
    if (menuEl) {
      const r = menuEl.getBoundingClientRect();
      win.setSize(
        new LogicalSize(Math.ceil(r.width) + MARGIN * 2, Math.ceil(r.height) + MARGIN * 2),
      );
    }
    const unlisten = win.listen("tray-open", () => (openSeq += 1));
    return () => {
      unlisten.then((u) => u());
    };
  });
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && win.hide()} />

<button class="backdrop" aria-label="Close menu" onclick={() => win.hide()}></button>

{#key openSeq}
  <div
    class="menu"
    role="menu"
    aria-label="EXFIL tray menu"
    tabindex="-1"
    use:autofocus
    bind:this={menuEl}
    onkeydown={onMenuKey}
  >
    <div class="brand">
      <span class="dot"></span>
      EXFIL
      {#if version}<span class="ver mono">v{version}</span>{/if}
    </div>
    <button class="item" role="menuitem" onclick={() => act("show")}>
      <AppWindow size={15} /> Show EXFIL
    </button>
    <button class="item" role="menuitem" onclick={() => act("reset")}>
      <RotateCcw size={15} /> Reset display
    </button>
    <div class="sep"></div>
    <button class="item danger" role="menuitem" onclick={() => act("quit")}>
      <Power size={15} /> Quit
    </button>
  </div>
{/key}

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
  }

  .backdrop {
    position: fixed;
    inset: 0;
    background: transparent;
    border: none;
    padding: 0;
    cursor: default;
  }

  .menu {
    position: fixed;
    top: 12px;
    left: 12px;
    width: 200px;
    padding: 5px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: linear-gradient(180deg, var(--bg-elev-3), var(--bg-elev-2));
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-lg), inset 0 1px 0 color-mix(in oklab, white 5%, transparent);
    animation: menu-in 130ms var(--ease-soft);
    transform-origin: bottom left;
  }
  .menu:focus {
    outline: none;
  }
  @keyframes menu-in {
    from { opacity: 0; transform: scale(0.95) translateY(4px); }
    to { opacity: 1; transform: scale(1) translateY(0); }
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 9px 7px;
    font-size: var(--fs-xs);
    font-weight: 650;
    letter-spacing: 0.08em;
    color: var(--fg-muted);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 999px;
    background: var(--accent);
    box-shadow: 0 0 8px color-mix(in oklab, var(--accent) 70%, transparent);
  }
  .ver {
    margin-left: auto;
    font-weight: 400;
    letter-spacing: 0;
    color: var(--fg-faint);
  }

  .sep {
    height: 1px;
    margin: 3px 6px;
    background: var(--border);
  }

  .item {
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
    transition: background 110ms ease, color 110ms ease, transform 80ms ease;
  }
  .item :global(svg) {
    flex-shrink: 0;
    opacity: 0.8;
    transition: opacity 110ms ease;
  }
  .item:hover {
    background: var(--surface-hover);
    color: var(--fg);
  }
  .item:hover :global(svg) {
    opacity: 1;
  }
  .item:active {
    transform: scale(0.98);
  }
  .item:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--ring);
  }
  .item.danger:hover {
    background: var(--danger-soft);
    color: var(--danger);
  }
</style>
