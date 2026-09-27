<script lang="ts">
  // SETTINGS page: every section rendered at once in two columns; the strip
  // scrolls to a section (and tracks the one in view) instead of swapping.
  import { onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getVersion } from "@tauri-apps/api/app";
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { ExternalLink } from "lucide-svelte";
  import { exportPresets, getAutostart, importPresets, installUpdate, openUrl, setAutostart, setHotkeys, uninstallApp } from "./api";
  import { app } from "./state.svelte";
  import { toast, reason } from "./toast.svelte";
  import Strip, { type StripItem } from "./Strip.svelte";
  import ArmChip from "./ArmChip.svelte";

  type Section = "general" | "hotkeys" | "data" | "about";
  let section = $state<Section>("general");
  const items = $derived.by((): StripItem[] => [
    { id: "general", label: "GENERAL" },
    { id: "hotkeys", label: "HOTKEYS" },
    { id: "data", label: "DATA" },
    { id: "about", label: "ABOUT", dot: app.updateMeta ? "var(--accent)" : undefined },
  ]);
  let body = $state<HTMLDivElement>();
  let lockUntil = 0;
  function goto(id: string) {
    section = id as Section;
    lockUntil = performance.now() + 600;
    body?.querySelector<HTMLElement>(`[data-sec="${id}"]`)?.scrollIntoView({ behavior: "smooth", block: "start" });
  }
  function onscroll() {
    if (!body || performance.now() < lockUntil) return;
    const top = body.getBoundingClientRect().top + 8;
    let best: Section = "general";
    for (const el of body.querySelectorAll<HTMLElement>("[data-sec]")) {
      if (el.getBoundingClientRect().top <= top) best = el.dataset.sec as Section;
    }
    section = best;
  }

  let autostart = $state(false);
  let version = $state("");
  let phase = $state<"idle" | "checking" | "none" | "installing">("idle");
  let progress = $state(0);
  let uninstallErr = $state("");
  let unlisten: UnlistenFn | undefined;

  onMount(() => {
    getAutostart().then((v) => (autostart = v)).catch(() => {});
    getVersion().then((v) => (version = v)).catch(() => {});
    listen<number>("update-progress", (e) => {
      progress = e.payload;
      toast.info(`◌ UPDATING · ${e.payload}%`, { spin: true, sticky: true });
    }).then((u) => (unlisten = u));
    return () => unlisten?.();
  });

  async function toggleAutostart() {
    try {
      autostart = await setAutostart(!autostart);
    } catch (e) {
      toast.error(`✕ AUTOSTART · ${reason(e)}`);
    }
  }
  async function toggleHotkeys() {
    try {
      app.hotkeys = await setHotkeys(!app.hotkeys);
    } catch (e) {
      toast.error(`✕ HOTKEYS · ${reason(e)}`);
    }
  }
  async function doExport() {
    try {
      const path = await saveDialog({ defaultPath: "exfil-presets.json", filters: [{ name: "EXFIL presets", extensions: ["json"] }] });
      if (!path) return;
      await exportPresets(path);
      toast.ok("✓ PRESETS EXPORTED");
    } catch (e) {
      toast.error(`✕ EXPORT FAILED · ${reason(e)}`);
    }
  }
  async function doImport() {
    try {
      const picked = await openDialog({ multiple: false, directory: false, filters: [{ name: "EXFIL presets", extensions: ["json"] }] });
      if (typeof picked !== "string") return;
      const store = await importPresets(picked);
      app.presets = store.presets;
      app.active = store.active;
      toast.ok("✓ PRESETS IMPORTED");
    } catch (e) {
      toast.error(`✕ IMPORT FAILED · ${reason(e)}`);
    }
  }
  async function check() {
    phase = "checking";
    const meta = await app.refreshUpdate();
    if (meta) phase = "idle";
    else {
      phase = "none";
      setTimeout(() => phase === "none" && (phase = "idle"), 3000);
    }
  }
  async function install() {
    phase = "installing";
    progress = 0;
    try {
      await installUpdate(); // only settles on failure — success relaunches
    } catch (e) {
      phase = "idle";
      toast.error(`✕ UPDATE FAILED · ${reason(e)}`, { label: "RETRY", run: () => void install() });
    }
  }
  async function uninstall() {
    uninstallErr = "";
    try {
      await uninstallApp();
    } catch (e) {
      uninstallErr = reason(e);
    }
  }
  const HOTKEYS = [
    ["Cycle presets", "includes Normal, so it doubles as on/off", "F9"],
    ["Snap to Normal", "native gamma + vibrance, instantly", "F10"],
    ["Toggle crosshair", "overlay master switch", "F11"],
    ["Cycle crosshairs", "None → each crosshair → None", "F12"],
  ];
</script>

<Strip {items} active={section} onselect={goto} />

<div class="body" class:grid={app.grid} bind:this={body} {onscroll}>
  <div class="col">
    <span class="sec-label display" data-sec="general">GENERAL</span>
    <div class="row panel">
      <span class="txt"><span class="t display">Start with Windows</span><span class="d mono">launch hidden in the tray at sign-in</span></span>
      <button class="toggle" class:on={autostart} aria-label="Start with Windows" onclick={toggleAutostart}></button>
    </div>
    <div class="row panel">
      <span class="txt"><span class="t display">Close hides to tray</span><span class="d mono">the titlebar ✕ hides · Quit lives in the tray menu</span></span>
      <button class="toggle on" disabled aria-label="Close hides to tray"></button>
    </div>
    <div class="row panel">
      <span class="txt"><span class="t display">Grid texture</span><span class="d mono">faint 24px grid behind the views</span></span>
      <button class="toggle" class:on={app.grid} aria-label="Grid texture" onclick={() => app.setGrid(!app.grid)}></button>
    </div>

    <span class="sec-label display" data-sec="data">DATA</span>
    <div class="row panel">
      <span class="txt"><span class="t display">Presets & crosshairs</span><span class="d mono">%APPDATA%\exfil-v2\ · import is additive, never overwrites</span></span>
      <span class="acts">
        <button class="chip" onclick={doImport}>IMPORT</button>
        <button class="chip" onclick={doExport}>EXPORT</button>
      </span>
    </div>
  </div>

  <div class="col">
    <span class="sec-label display hk" data-sec="hotkeys">
      GLOBAL HOTKEYS
      <button class="toggle" class:on={app.hotkeys} aria-label="Global hotkeys" onclick={toggleHotkeys}></button>
    </span>
    <div class="panel list" class:off={!app.hotkeys}>
      {#each HOTKEYS as [t, d, k]}
        <div class="hrow">
          <span class="txt"><span class="t display">{t}</span><span class="d mono">{d}</span></span>
          <span class="keys mono"><kbd>Ctrl</kbd><kbd>Shift</kbd><kbd class="f">{k}</kbd></span>
        </div>
      {/each}
    </div>

    <span class="sec-label display" data-sec="about">ABOUT</span>
    <div class="row panel about">
      <img class="icon" src="/favicon.png" alt="" draggable="false" />
      <span class="txt">
        <span class="t display big">EXFIL {#if version}v{version}{/if}</span>
        <span class="d mono">MIT · github.com/Blazzer10200/exfil</span>
      </span>
      <button class="chip" onclick={() => void openUrl("https://github.com/Blazzer10200/exfil")}><ExternalLink size={12} /> GITHUB</button>
    </div>
    <div class="row panel" class:ready={!!app.updateMeta}>
      <span class="txt">
        <span class="t display" class:amber={!!app.updateMeta}>{app.updateMeta ? `UPDATE READY · v${app.updateMeta.version}` : "Updates"}</span>
        <span class="d mono">{app.updateMeta ? "signature-verified · restarts when done" : "signed GitHub release · installs + relaunches"}</span>
      </span>
      {#if phase === "installing"}
        <span class="pct mono">{progress}%</span>
      {:else if app.updateMeta}
        <button class="chip accent" onclick={install}>INSTALL</button>
      {:else if phase === "checking"}
        <span class="pct mono">CHECKING…</span>
      {:else if phase === "none"}
        <span class="pct mono ok">UP TO DATE</span>
      {:else}
        <button class="chip" onclick={check}>CHECK FOR UPDATES</button>
      {/if}
    </div>
    <div class="row panel danger">
      <span class="txt">
        <span class="t display red">Uninstall EXFIL</span>
        <span class="d mono">{uninstallErr || "restores native color first · removes the installed copy"}</span>
      </span>
      <ArmChip label="UNINSTALL…" onconfirm={uninstall} />
    </div>
  </div>
</div>

<style>
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 14px;
    padding: 14px 16px;
    align-content: start;
  }
  .body.grid {
    background-image: var(--hud-grid);
    background-size: 24px 24px;
  }
  @media (max-width: 760px) {
    .body {
      grid-template-columns: 1fr;
    }
  }
  .col {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .sec-label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 6px;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.14em;
    color: var(--hud-fg-low);
  }
  .col > .sec-label:first-child {
    margin-top: 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    animation: hud-in 200ms var(--ease-hud) both;
  }
  .list {
    display: flex;
    flex-direction: column;
    animation: hud-in 200ms var(--ease-hud) both;
  }
  .list.off {
    opacity: 0.5;
  }
  .hrow {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 9px 12px;
  }
  .hrow + .hrow {
    border-top: 1px solid var(--hud-line);
  }
  .row.ready {
    border-color: color-mix(in oklab, var(--accent) 35%, var(--hud-line-2));
    background: oklch(0.8 0.15 75 / 0.06);
  }
  .row.danger {
    border-color: color-mix(in oklab, var(--danger) 30%, var(--hud-line-2));
  }
  .txt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .t {
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--hud-fg-hi);
  }
  .t.big {
    font-size: 13px;
    font-weight: 700;
  }
  .t.amber {
    color: oklch(0.88 0.06 75);
  }
  .t.red {
    color: var(--danger);
  }
  .d {
    font-size: 10px;
    color: var(--hud-fg-low);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .keys {
    display: inline-flex;
    gap: 4px;
  }
  kbd {
    height: 20px;
    padding: 0 6px;
    display: inline-grid;
    place-items: center;
    border: 1px solid var(--hud-line-4);
    border-radius: 2px;
    background: var(--hud-pill);
    font-family: var(--font-mono);
    font-size: 9.5px;
    color: var(--hud-fg-3);
  }
  kbd.f {
    color: var(--accent);
    border-color: color-mix(in oklab, var(--accent) 40%, transparent);
  }
  .acts {
    display: flex;
    gap: 6px;
  }
  .icon {
    width: 36px;
    height: 36px;
    border-radius: var(--hud-r);
    -webkit-user-drag: none;
  }
  .pct {
    font-size: 11px;
    color: var(--accent);
    font-variant-numeric: tabular-nums;
  }
  .pct.ok {
    color: var(--ok);
  }
</style>
