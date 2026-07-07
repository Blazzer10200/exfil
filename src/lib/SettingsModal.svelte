<script lang="ts">
  // Settings modal — opened from the titlebar cogwheel. Owns the app-level
  // preferences (autostart), preset import/export entry points, about info,
  // and the uninstall flow (two-click confirm → NSIS uninstaller).
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { Settings, X, Power, Keyboard, Download, Upload, ExternalLink, Trash2, ShieldCheck } from "lucide-svelte";
  import { getAutostart, setAutostart, getHotkeys, setHotkeys, uninstallApp, openUrl } from "./api";

  interface Props {
    onclose: () => void;
    onimport: () => void;
    onexport: () => void;
    onerror?: (message: string) => void;
  }
  let { onclose, onimport, onexport, onerror }: Props = $props();

  let version = $state("");
  let autostart = $state(true);
  let hotkeys = $state(true);
  let confirmUninstall = $state(false);
  let uninstallError = $state("");
  let confirmTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    getVersion()
      .then((v) => (version = v))
      .catch(() => {});
    getAutostart()
      .then((v) => (autostart = v))
      .catch((e) => onerror?.(String(e)));
    getHotkeys()
      .then((v) => (hotkeys = v))
      .catch((e) => onerror?.(String(e)));
    return () => clearTimeout(confirmTimer);
  });

  async function toggleAutostart() {
    try {
      autostart = await setAutostart(!autostart);
    } catch (e) {
      onerror?.(String(e));
    }
  }

  async function toggleHotkeys() {
    try {
      hotkeys = await setHotkeys(!hotkeys);
    } catch (e) {
      onerror?.(String(e));
    }
  }

  async function doUninstall() {
    if (!confirmUninstall) {
      confirmUninstall = true;
      uninstallError = "";
      clearTimeout(confirmTimer);
      confirmTimer = setTimeout(() => (confirmUninstall = false), 4000);
      return;
    }
    clearTimeout(confirmTimer);
    confirmUninstall = false;
    try {
      // On success the app exits into the uninstall wizard — no code after this runs.
      await uninstallApp();
    } catch (e) {
      uninstallError = String(e);
    }
  }

  function github() {
    openUrl("https://github.com/Blazzer10200/exfil").catch((e) => onerror?.(String(e)));
  }

  function focusOnMount(node: HTMLElement) {
    node.focus();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<button class="backdrop" aria-label="Close settings" onclick={onclose}></button>
<div class="settings" role="dialog" aria-modal="true" aria-label="Settings" tabindex="-1" use:focusOnMount>
  <header>
    <span class="badge"><Settings size={15} /></span>
    <h2>Settings</h2>
    <button class="close" aria-label="Close" onclick={onclose}><X size={15} /></button>
  </header>

  <section>
    <h3>General</h3>
    <div class="row">
      <span class="row-icon"><Power size={15} /></span>
      <div class="row-text">
        <span class="row-title">Start with Windows</span>
        <span class="row-desc">Launches hidden in the tray when you sign in</span>
      </div>
      <button
        class="switch"
        class:on={autostart}
        role="switch"
        aria-checked={autostart}
        aria-label="Start with Windows"
        onclick={toggleAutostart}
      >
        <span class="knob"></span>
      </button>
    </div>
    <div class="row">
      <span class="row-icon"><Keyboard size={15} /></span>
      <div class="row-text">
        <span class="row-title">Global hotkeys</span>
        <span class="row-desc">Ctrl+Shift+F9 cycles presets · Ctrl+Shift+F10 restores Normal</span>
      </div>
      <button
        class="switch"
        class:on={hotkeys}
        role="switch"
        aria-checked={hotkeys}
        aria-label="Global hotkeys"
        onclick={toggleHotkeys}
      >
        <span class="knob"></span>
      </button>
    </div>
  </section>

  <section>
    <h3>Presets</h3>
    <div class="row">
      <span class="row-icon"><Download size={15} /></span>
      <div class="row-text">
        <span class="row-title">Import presets</span>
        <span class="row-desc">Add presets from a shared file</span>
      </div>
      <button class="act" onclick={onimport}>Import…</button>
    </div>
    <div class="row">
      <span class="row-icon"><Upload size={15} /></span>
      <div class="row-text">
        <span class="row-title">Export presets</span>
        <span class="row-desc">Back up or share your presets as a file</span>
      </div>
      <button class="act" onclick={onexport}>Export…</button>
    </div>
  </section>

  <section>
    <h3>About</h3>
    <div class="about">
      <img src="/favicon.png" alt="" />
      <div class="about-text">
        <span class="about-name">
          EXFIL
          {#if version}<span class="mono about-ver">v{version}</span>{/if}
        </span>
        <span class="row-desc">Per-game color, gamma &amp; digital-vibrance presets</span>
        <span class="safe"><ShieldCheck size={12} /> Driver-level — no injection, anti-cheat safe</span>
      </div>
      <button class="act" onclick={github}><ExternalLink size={13} /> GitHub</button>
    </div>
  </section>

  <section class="danger-zone">
    <h3>Danger zone</h3>
    <div class="row">
      <span class="row-icon danger-icon"><Trash2 size={15} /></span>
      <div class="row-text">
        <span class="row-title">Uninstall EXFIL</span>
        <span class="row-desc">Removes EXFIL from this PC — display returns to native</span>
      </div>
      <button class="act danger" class:confirm={confirmUninstall} onclick={doUninstall}>
        {confirmUninstall ? "Click to confirm" : "Uninstall…"}
      </button>
    </div>
    {#if uninstallError}
      <p class="uninstall-error">{uninstallError}</p>
    {/if}
  </section>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    background:
      radial-gradient(900px 500px at 50% 30%, color-mix(in oklab, var(--accent) 6%, transparent), transparent 60%),
      color-mix(in oklab, #000 50%, transparent);
    backdrop-filter: blur(2px);
    border: none;
    padding: 0;
    cursor: default;
    animation: backdrop-in 140ms ease;
  }
  @keyframes backdrop-in {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  .settings {
    position: fixed;
    z-index: 70;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 460px;
    max-height: 88vh;
    overflow-y: auto;
    padding: 14px 16px 16px;
    display: flex;
    flex-direction: column;
    gap: 11px;
    background:
      radial-gradient(500px 260px at 85% -10%, color-mix(in oklab, var(--accent) 10%, transparent), transparent 60%),
      linear-gradient(180deg, var(--bg-elev-3), var(--bg-elev-2));
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-xl);
    box-shadow: var(--shadow-lg), inset 0 1px 0 color-mix(in oklab, white 6%, transparent);
    animation: settings-in 150ms var(--ease-soft);
  }
  .settings:focus {
    outline: none;
  }
  @keyframes settings-in {
    from { opacity: 0; transform: translate(-50%, -50%) scale(0.96); }
    to { opacity: 1; transform: translate(-50%, -50%) scale(1); }
  }

  header {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .badge {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: var(--radius);
    background: var(--accent-soft);
    color: var(--accent);
  }
  h2 {
    margin: 0;
    flex: 1;
    font-size: var(--fs-lg);
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  .close {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--fg-muted);
    cursor: pointer;
    transition: background 100ms ease, color 100ms ease;
  }
  .close:hover {
    background: var(--surface-hover);
    color: var(--fg);
  }

  section {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
  h3 {
    margin: 0 0 1px 2px;
    font-size: var(--fs-xs);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.09em;
    color: var(--fg-subtle);
  }
  .danger-zone h3 {
    color: color-mix(in oklab, var(--danger) 75%, var(--fg-subtle));
  }

  .row {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: color-mix(in oklab, white 2.5%, transparent);
  }
  .danger-zone .row {
    border-color: color-mix(in oklab, var(--danger) 22%, var(--border));
  }
  .row-icon {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    flex-shrink: 0;
    border-radius: var(--radius-sm);
    background: color-mix(in oklab, var(--accent) 9%, transparent);
    color: var(--fg-muted);
  }
  .row-icon.danger-icon {
    background: var(--danger-soft);
    color: var(--danger);
  }
  .row-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .row-title {
    font-size: var(--fs-sm);
    font-weight: 550;
    color: var(--fg);
  }
  .row-desc {
    font-size: var(--fs-xs);
    color: var(--fg-subtle);
    line-height: 1.45;
  }

  .act {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
    height: 27px;
    padding: 0 11px;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--surface);
    color: var(--fg-2);
    font: inherit;
    font-size: var(--fs-xs);
    cursor: pointer;
    transition: background 100ms ease, border-color 100ms ease, color 100ms ease;
  }
  .act:hover {
    background: var(--surface-hover);
    color: var(--fg);
  }
  .act.danger {
    color: var(--danger);
    border-color: color-mix(in oklab, var(--danger) 35%, transparent);
  }
  .act.danger:hover {
    background: var(--danger-soft);
  }
  .act.danger.confirm {
    background: var(--danger);
    border-color: transparent;
    color: white;
    font-weight: 600;
  }
  .uninstall-error {
    margin: 0 2px;
    font-size: var(--fs-xs);
    color: var(--danger);
    line-height: 1.5;
  }

  .switch {
    position: relative;
    flex-shrink: 0;
    width: 36px;
    height: 20px;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    background: var(--track);
    cursor: pointer;
    transition: background 140ms ease, border-color 140ms ease;
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: 999px;
    background: var(--fg-muted);
    transition: transform 140ms var(--ease-soft), background 140ms ease;
  }
  .switch.on {
    background: var(--accent);
    border-color: transparent;
  }
  .switch.on .knob {
    transform: translateX(16px);
    background: var(--accent-fg);
  }

  .about {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 9px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: color-mix(in oklab, white 2.5%, transparent);
  }
  .about img {
    width: 40px;
    height: 40px;
    flex-shrink: 0;
    border-radius: var(--radius);
    object-fit: contain;
  }
  .about-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .about-name {
    font-size: var(--fs-md);
    font-weight: 650;
    letter-spacing: 0.04em;
    color: var(--fg);
  }
  .about-ver {
    margin-left: 6px;
    font-size: var(--fs-xs);
    font-weight: 400;
    color: var(--fg-faint);
    letter-spacing: 0;
  }
  .safe {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    margin-top: 2px;
    font-size: var(--fs-xs);
    color: var(--ok);
  }
  .safe :global(svg) {
    flex-shrink: 0;
  }

  .act:focus-visible,
  .switch:focus-visible,
  .close:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--ring);
  }
</style>
