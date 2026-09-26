<script lang="ts">
  // Modal "pick a program" chooser shared by preset binding/creation and
  // crosshair binding: browse for an .exe via the OS dialog, OR pick from the
  // live list of programs with a visible window. Hands back the lowercased exe
  // basename (+ a display title) — callers decide what binding means.
  import { onMount } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { Gamepad2, Link2, RotateCw, X } from "lucide-svelte";
  import { listWindowPrograms, type WindowProc } from "./api";

  interface Props {
    heading: string;
    sub: string;
    onpick: (exe: string, title: string) => void;
    onclose: () => void;
    onerror?: (message: string) => void;
  }
  let { heading, sub, onpick, onclose, onerror }: Props = $props();

  let procs = $state<WindowProc[]>([]);
  let procFilter = $state("");
  let procLoading = $state(false);

  let filteredProcs = $derived(
    procFilter.trim()
      ? procs.filter((p) => {
          const q = procFilter.trim().toLowerCase();
          return p.title.toLowerCase().includes(q) || p.exe.includes(q);
        })
      : procs,
  );

  async function loadProcs() {
    procLoading = true;
    try {
      procs = await listWindowPrograms();
    } catch (e) {
      procs = [];
      onerror?.(`Failed to list running programs: ${String(e)}`);
    } finally {
      procLoading = false;
    }
  }

  onMount(loadProcs);

  async function browseExe() {
    const picked = await openDialog({
      multiple: false,
      directory: false,
      filters: [{ name: "Programs", extensions: ["exe"] }],
    });
    if (typeof picked !== "string") return;
    const base = picked.split(/[\\/]/).pop()?.toLowerCase() ?? "";
    if (!base) return;
    // No window title from a file pick — derive one from the basename.
    onpick(base, base.replace(/\.exe$/i, ""));
    onclose();
  }

  function pickProc(proc: WindowProc) {
    onpick(proc.exe, proc.title);
    onclose();
  }

  function focusOnMount(node: HTMLElement) {
    node.focus();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<button class="menu-backdrop modal" aria-label="Close picker" onclick={onclose}></button>
<div class="binder" role="dialog" aria-modal="true" aria-label={heading} tabindex="-1" use:focusOnMount>
  <div class="binder-glow"></div>
  <button class="binder-close" aria-label="Close" title="Close" onclick={onclose}>
    <X size={14} />
  </button>
  <header class="binder-head">
    <div class="binder-icon"><Gamepad2 size={18} /></div>
    <div class="binder-head-text">
      <span>{heading}</span>
      <span class="binder-sub">{sub}</span>
    </div>
  </header>
  <button class="browse" onclick={browseExe}>
    <Link2 size={14} />
    Browse for .exe…
  </button>
  <div class="binder-or"><span></span>or pick a running program<span></span></div>
  <div class="proc-filter-row">
    <input class="proc-filter" placeholder="Filter…" bind:value={procFilter} />
    <button class="proc-refresh" title="Refresh list" aria-label="Refresh list" onclick={loadProcs}>
      <RotateCw size={14} class={procLoading ? "spin" : ""} />
    </button>
  </div>
  <div class="proc-list">
    {#each filteredProcs as proc (proc.exe)}
      <button class="proc" onclick={() => pickProc(proc)}>
        <span class="proc-dot"></span>
        <span class="proc-text">
          <span class="proc-title">{proc.title}</span>
          <span class="proc-exe">{proc.exe}</span>
        </span>
      </button>
    {:else}
      <div class="proc-empty">
        {procLoading ? "Loading…" : "No matching programs"}
      </div>
    {/each}
  </div>
</div>

<style>
  .menu-backdrop.modal {
    position: fixed;
    inset: 0;
    z-index: 60;
    padding: 0;
    border: none;
    cursor: default;
    background:
      radial-gradient(900px 500px at 50% 30%, color-mix(in oklab, var(--accent) 6%, transparent), transparent 60%),
      color-mix(in oklab, #000 50%, transparent);
    backdrop-filter: blur(2px);
    animation: backdrop-in 140ms ease;
  }
  @keyframes backdrop-in { from { opacity: 0; } to { opacity: 1; } }
  .binder {
    position: fixed;
    z-index: 70;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 340px;
    max-height: 70vh;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 18px;
    overflow: hidden;
    background:
      radial-gradient(180px 120px at 16% -10%, color-mix(in oklab, var(--accent) 16%, transparent), transparent 70%),
      linear-gradient(180deg, var(--bg-elev-3) 0%, var(--bg-elev-2) 100%);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-xl, 14px);
    box-shadow: var(--shadow-lg), inset 0 1px 0 color-mix(in oklab, white 6%, transparent);
    animation: binder-in 180ms var(--ease-soft);
  }
  .binder:focus { outline: none; }
  @keyframes binder-in {
    from { opacity: 0; transform: translate(-50%, -46%) scale(0.96); }
    to { opacity: 1; transform: translate(-50%, -50%) scale(1); }
  }
  .binder-glow {
    position: absolute;
    inset: 0;
    pointer-events: none;
    box-shadow: inset 0 0 60px color-mix(in oklab, var(--accent) 5%, transparent);
  }
  .binder-close {
    position: absolute;
    top: 10px;
    right: 10px;
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
  .binder-close:hover { background: var(--surface-hover); color: var(--fg); }
  .binder-head {
    display: flex;
    align-items: flex-start;
    gap: 10px;
  }
  .binder-icon {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    border-radius: var(--radius);
    background: linear-gradient(155deg, color-mix(in oklab, var(--accent) 22%, transparent), color-mix(in oklab, var(--accent) 8%, transparent));
    border: 1px solid color-mix(in oklab, var(--accent) 30%, transparent);
    color: var(--accent);
    box-shadow: 0 0 16px color-mix(in oklab, var(--accent) 25%, transparent);
  }
  .binder-head-text { display: flex; flex-direction: column; gap: 2px; padding-top: 2px; }
  .binder-head-text > span:first-child { font-size: var(--fs-md); font-weight: 600; color: var(--fg); }
  .binder-sub { font-size: var(--fs-xs); color: var(--fg-muted); line-height: 1.4; }
  .browse {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    padding: 10px;
    border-radius: var(--radius);
    border: 1px solid var(--border-strong);
    background: linear-gradient(180deg, var(--surface-hover), var(--field));
    color: var(--fg-2);
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: 500;
    cursor: pointer;
    transition: background 100ms ease, color 100ms ease, border-color 100ms ease, box-shadow 120ms ease, transform 80ms ease;
  }
  .browse:hover {
    background: linear-gradient(180deg, var(--surface-active), var(--surface-hover));
    color: var(--fg);
    border-color: color-mix(in oklab, var(--accent) 40%, var(--border-strong));
    box-shadow: 0 0 0 1px color-mix(in oklab, var(--accent) 15%, transparent);
  }
  .browse:active { transform: translateY(1px); }
  .binder-or {
    display: flex;
    align-items: center;
    gap: 8px;
    text-align: center;
    font-size: var(--fs-xs);
    color: var(--fg-subtle);
  }
  .binder-or > span {
    flex: 1;
    height: 1px;
    background: linear-gradient(90deg, transparent, var(--border-strong), transparent);
  }
  .proc-filter-row {
    display: flex;
    gap: 6px;
    align-items: stretch;
  }
  .proc-filter {
    flex: 1;
    min-width: 0;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--field);
    color: var(--fg);
    font: inherit;
    font-size: var(--fs-sm);
    outline: none;
    transition: border-color 100ms ease, box-shadow 100ms ease;
  }
  .proc-filter:focus { border-color: var(--border-focus); box-shadow: 0 0 0 2px var(--ring); }
  .proc-refresh {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 33px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-strong);
    background: var(--field);
    color: var(--fg-2);
    cursor: pointer;
    transition: background 100ms ease, color 100ms ease, border-color 100ms ease;
  }
  .proc-refresh:hover { background: var(--surface-hover); color: var(--accent); border-color: color-mix(in oklab, var(--accent) 35%, var(--border-strong)); }
  .proc-refresh :global(.spin) { animation: proc-spin 700ms linear infinite; }
  @keyframes proc-spin { to { transform: rotate(360deg); } }
  .proc-list {
    display: flex;
    flex-direction: column;
    gap: 3px;
    overflow-y: auto;
    min-height: 0;
    flex: 1;
  }
  .proc {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 7px 9px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    background: transparent;
    color: var(--fg-2);
    font: inherit;
    text-align: left;
    cursor: pointer;
    overflow: hidden;
    transition: background 120ms ease, border-color 120ms ease, transform 80ms ease;
  }
  .proc:hover {
    background: var(--surface-hover);
    border-color: var(--border);
    color: var(--fg);
    transform: translateX(1px);
  }
  .proc-dot {
    flex-shrink: 0;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--fg-faint);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--fg-faint) 12%, transparent);
    transition: background 120ms ease, box-shadow 120ms ease;
  }
  .proc:hover .proc-dot {
    background: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--accent) 22%, transparent);
  }
  .proc-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .proc-title {
    font-size: var(--fs-sm);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .proc-exe {
    font-size: var(--fs-xs);
    color: var(--fg-subtle);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .proc-empty {
    padding: 14px;
    text-align: center;
    font-size: var(--fs-xs);
    color: var(--fg-subtle);
  }
  .proc:focus-visible,
  .browse:focus-visible,
  .proc-refresh:focus-visible,
  .proc-filter:focus-visible,
  .binder-close:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px var(--ring);
  }
</style>
