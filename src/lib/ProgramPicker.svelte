<script lang="ts">
  // BIND PROGRAM modal: browse for an .exe, or pick a running window. Hands
  // back the lowercased exe basename + a display title; callers decide what
  // binding means. The list is a read-only window enumeration (no injection).
  import { onMount } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { FolderOpen, RotateCw, X } from "lucide-svelte";
  import { listWindowPrograms, type WindowProc } from "./api";
  import { app } from "./state.svelte";
  import { toast, reason } from "./toast.svelte";

  interface Props {
    heading?: string;
    sub: string;
    onpick: (exe: string, title: string) => void;
    onclose: () => void;
    /** Name of whatever this exe is already bound to (shows "→ X · <boundHint>"). */
    boundTo?: (exe: string) => string | null;
    /** What picking an already-bound exe does. */
    boundHint?: string;
  }
  let { heading = "BIND PROGRAM", sub, onpick, onclose, boundTo, boundHint = "will rebind" }: Props = $props();

  let procs = $state<WindowProc[]>([]);
  let filter = $state("");
  let loading = $state(true);

  const filtered = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    const list = q ? procs.filter((p) => p.title.toLowerCase().includes(q) || p.exe.includes(q)) : procs;
    // In-front program first.
    return [...list].sort((a, b) => Number(b.exe === app.inFront) - Number(a.exe === app.inFront));
  });

  async function load() {
    loading = true;
    try {
      procs = await listWindowPrograms();
    } catch (e) {
      procs = [];
      toast.error(`✕ WINDOW LIST FAILED · ${reason(e)}`);
    } finally {
      loading = false;
    }
  }
  onMount(load);

  async function browse() {
    const picked = await openDialog({ multiple: false, directory: false, filters: [{ name: "Programs", extensions: ["exe"] }] });
    if (typeof picked !== "string") return;
    const base = picked.split(/[\\/]/).pop()?.toLowerCase() ?? "";
    if (!base) return;
    onpick(base, base.replace(/\.exe$/i, ""));
    onclose();
  }
  function pick(p: WindowProc) {
    onpick(p.exe, p.title);
    onclose();
  }
  function focus(node: HTMLElement) {
    node.focus();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<button class="backdrop" aria-label="Close" onclick={onclose}></button>
<div class="dialog" role="dialog" aria-modal="true" aria-label={heading} tabindex="-1" use:focus>
  <button class="close" aria-label="Close" onclick={onclose}><X size={14} /></button>
  <div class="head">
    <div class="title display">{heading}</div>
    <div class="sub mono">{sub}</div>
  </div>
  <button class="chip tall browse" onclick={browse}><FolderOpen size={13} /> BROWSE FOR .EXE…</button>
  <div class="or mono"><span></span>OR A RUNNING WINDOW<span></span></div>
  <div class="filter-row">
    <input class="field" placeholder="Filter windows…" bind:value={filter} />
    <button class="chip refresh" title="Refresh" aria-label="Refresh" onclick={load}>
      <span class:spin={loading}><RotateCw size={12} /></span>
    </button>
  </div>
  <div class="list">
    {#if loading && !procs.length}
      {#each [0, 1, 2] as i}
        <div class="skel" style="--i: {i}"></div>
      {/each}
    {:else}
      {#each filtered as p (p.exe)}
        {@const bound = boundTo?.(p.exe)}
        <button class="row" onclick={() => pick(p)}>
          <span class="dot" class:front={p.exe === app.inFront}></span>
          <span class="text">
            <span class="t">{p.title}</span>
            <span class="e mono">{p.exe}{#if bound}<span class="rebind"> → {bound.toUpperCase()} · {boundHint}</span>{/if}</span>
          </span>
        </button>
      {:else}
        <div class="empty mono">NO WINDOWS FOUND</div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .dialog {
    position: fixed;
    z-index: 70;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 380px;
    max-height: 460px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    background: oklch(0.14 0.004 250);
    border: 1px solid var(--hud-line-5);
    border-top: 2px solid var(--accent);
    border-radius: var(--hud-r);
    box-shadow: 0 30px 70px oklch(0 0 0 / 0.7);
    animation: hud-pop 220ms var(--ease-hud);
    outline: none;
  }
  .close {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: var(--hud-r);
    background: transparent;
    color: var(--hud-fg-dim);
    cursor: pointer;
  }
  .close:hover {
    background: var(--hud-hover);
    color: var(--hud-fg);
  }
  .title {
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--hud-fg-hi);
  }
  .sub {
    margin-top: 3px;
    font-size: 10px;
    color: var(--hud-fg-low);
  }
  .browse {
    width: 100%;
    justify-content: center;
  }
  .or {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 9px;
    letter-spacing: 0.12em;
    color: var(--hud-fg-faint);
  }
  .or span {
    flex: 1;
    height: 1px;
    background: var(--hud-line-2);
  }
  .filter-row {
    display: flex;
    gap: 6px;
  }
  .filter-row .field {
    flex: 1;
    height: 28px;
  }
  .refresh {
    width: 28px;
    height: 28px;
    padding: 0;
    justify-content: center;
  }
  .spin {
    display: inline-flex;
    animation: hud-spin 800ms linear infinite;
  }
  .list {
    flex: 1;
    min-height: 120px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin: 0 -4px;
    padding: 0 4px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 8px;
    border: 0;
    border-radius: var(--hud-r);
    background: transparent;
    color: var(--hud-fg);
    text-align: left;
    cursor: pointer;
    transition: background 120ms var(--ease-hud), transform 120ms var(--ease-hud);
  }
  .row:hover {
    background: var(--hud-hover);
    transform: translateX(2px);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--hud-fg-faint);
    flex: 0 0 6px;
  }
  .dot.front {
    background: var(--ok);
    box-shadow: 0 0 6px color-mix(in oklab, var(--ok) 70%, transparent);
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .t {
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .e {
    font-size: 10px;
    color: var(--hud-fg-low);
  }
  .rebind {
    color: var(--accent);
  }
  .skel {
    height: 34px;
    border-radius: var(--hud-r);
    background: linear-gradient(90deg, var(--hud-panel) 25%, var(--hud-hover) 50%, var(--hud-panel) 75%);
    background-size: 200% 100%;
    animation: hud-shimmer 1.2s linear infinite;
    animation-delay: calc(var(--i) * 120ms);
  }
  .empty {
    margin: 8px 0;
    padding: 18px;
    border: 1px dashed var(--hud-line-4);
    border-radius: var(--hud-r);
    font-size: 10px;
    letter-spacing: 0.1em;
    color: var(--hud-fg-faint);
    text-align: center;
  }
</style>
