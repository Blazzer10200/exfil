<script lang="ts">
  import type { Preset, WindowProc } from "./api";
  import { slotAccent, listWindowPrograms } from "./api";
  import { Pencil, Trash2, Lock, Link2, Unlink, RotateCw, Gamepad2, Plus, FilePlus2, ChevronDown } from "lucide-svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";

  interface Props {
    presets: Preset[];
    active: string;
    dirty: boolean;
    onselect: (slot: string) => void;
    oncreate: () => void;
    ondelete: (slot: string) => void;
    onrename: (slot: string, name: string) => void;
    onbind: (slot: string, exe: string | null) => void;
    oncreategame: (exe: string, title: string) => void;
    onerror?: (message: string) => void;
  }
  let {
    presets,
    active,
    dirty,
    onselect,
    oncreate,
    ondelete,
    onrename,
    onbind,
    oncreategame,
    onerror,
  }: Props = $props();

  // Accent index = position among non-Normal presets (Normal is fixed grey).
  function accentIndex(slot: string): number {
    let i = 0;
    for (const p of presets) {
      if (p.slot === "Normal") continue;
      if (p.slot === slot) return i;
      i++;
    }
    return 0;
  }

  let editing = $state<string | null>(null);
  let draft = $state("");

  function startRename(slot: string) {
    if (slot === "Normal") return;
    const p = presets.find((x) => x.slot === slot);
    if (!p) return;
    editing = slot;
    draft = p.name;
  }
  function commitRename() {
    if (editing === null) return;
    const slot = editing;
    const name = draft.trim();
    editing = null;
    const p = presets.find((x) => x.slot === slot);
    if (name && p && name !== p.name) onrename(slot, name);
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      (e.currentTarget as HTMLInputElement).blur();
    } else if (e.key === "Escape") {
      editing = null;
    }
  }

  // ── Right-click context menu ──
  // Menu width/height are fixed enough to clamp against the viewport so it
  // never spills off-screen near the window edges.
  const MENU_W = 180;
  const MENU_H = 132;
  let menu = $state<{ slot: string; x: number; y: number } | null>(null);

  function openMenu(e: MouseEvent, slot: string) {
    e.preventDefault();
    const x = Math.min(e.clientX, window.innerWidth - MENU_W - 8);
    const y = Math.min(e.clientY, window.innerHeight - MENU_H - 8);
    menu = { slot, x: Math.max(8, x), y: Math.max(8, y) };
  }
  function closeMenu() {
    menu = null;
  }
  function menuRename() {
    if (menu) startRename(menu.slot);
    closeMenu();
  }
  function menuDelete() {
    if (menu) ondelete(menu.slot);
    closeMenu();
  }

  // ── Program binding ──
  // "Bind to program" opens a chooser: browse for an .exe via the OS file
  // dialog, OR pick from the live running-process list. Binding stores the exe
  // basename; the backend watcher auto-applies this preset when it runs.
  // Binder modal serves two intents:
  //  - "bind":   attach a program to an existing slot (right-click → Bind)
  //  - "create": make a NEW preset straight from a running game (rail button)
  let binder = $state<
    { mode: "bind"; slot: string } | { mode: "create" } | null
  >(null);
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

  function boundExe(slot: string): string | null {
    return presets.find((p) => p.slot === slot)?.exe ?? null;
  }

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

  async function openBinderFor(b: { mode: "bind"; slot: string } | { mode: "create" }) {
    binder = b;
    procFilter = "";
    procs = [];
    await loadProcs();
  }

  async function openBinder() {
    if (!menu) return;
    const slot = menu.slot;
    closeMenu();
    await openBinderFor({ mode: "bind", slot });
  }

  async function openCreateFromGame() {
    await openBinderFor({ mode: "create" });
  }
  function closeBinder() {
    binder = null;
  }
  async function browseExe() {
    if (!binder) return;
    const b = binder;
    const picked = await openDialog({
      multiple: false,
      directory: false,
      filters: [{ name: "Programs", extensions: ["exe"] }],
    });
    if (typeof picked === "string") {
      const base = picked.split(/[\\/]/).pop()?.toLowerCase() ?? "";
      if (base) {
        if (b.mode === "create") {
          // No window title from a file pick — derive a name from the basename.
          const title = base.replace(/\.exe$/i, "");
          oncreategame(base, title);
        } else {
          onbind(b.slot, base);
        }
        closeBinder();
      }
    }
  }
  function pickProc(proc: WindowProc) {
    if (!binder) return;
    if (binder.mode === "create") {
      oncreategame(proc.exe, proc.title);
    } else {
      onbind(binder.slot, proc.exe);
    }
    closeBinder();
  }
  function menuUnbind() {
    if (menu) onbind(menu.slot, null);
    closeMenu();
  }

  // ── Add-preset popover ──
  // Single entry point for both preset-creation flows: a blank preset, or one
  // seeded from a running program. Replaces two separate rail buttons.
  let addOpen = $state(false);
  function toggleAdd() {
    addOpen = !addOpen;
  }
  function closeAdd() {
    addOpen = false;
  }
  function addBlank() {
    closeAdd();
    oncreate();
  }
  async function addFromProgram() {
    closeAdd();
    await openCreateFromGame();
  }

  // Moves focus into a just-opened menu/dialog so Escape/keyboard nav work
  // without requiring a prior click inside it.
  function focusOnMount(node: HTMLElement) {
    node.focus();
  }
</script>

<svelte:window
  onkeydown={(e) => e.key === "Escape" && (closeMenu(), closeAdd())}
  onblur={() => (closeMenu(), closeAdd())}
/>

<nav class="rail">
  <div class="slots" onscroll={closeMenu}>
    {#each presets as p (p.slot)}
      <div
        class="slot"
        class:active={p.slot === active}
        class:targeted={menu?.slot === p.slot}
        style="--slot-accent: {slotAccent(p.slot, accentIndex(p.slot))}"
      >
        <button
          class="pick"
          onclick={() => onselect(p.slot)}
          ondblclick={() => startRename(p.slot)}
          oncontextmenu={(e) => openMenu(e, p.slot)}
          title={p.slot === "Normal" ? "Native baseline" : p.name}
        >
          <span class="dot" class:dirty={p.slot === active && dirty}></span>
          {#if editing === p.slot}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="rename"
              bind:value={draft}
              onblur={commitRename}
              onkeydown={onKey}
              onclick={(e) => e.stopPropagation()}
              autofocus
            />
          {:else}
            <span class="label">{p.name}</span>
          {/if}
        </button>
        {#if p.slot !== "Normal" && p.exe}
          <span class="bound" title="Auto-switches when {p.exe} is running">
            <Link2 size={10} />
            {p.exe}
          </span>
        {/if}
      </div>
    {/each}
  </div>

  <div class="add-wrap">
    <button class="new no-drag" onclick={toggleAdd} aria-expanded={addOpen} aria-haspopup="true">
      <Plus size={14} /> Add preset <ChevronDown size={12} class="chev" />
    </button>
    {#if addOpen}
      <button class="menu-backdrop" aria-label="Close menu" onclick={closeAdd}></button>
      <div class="add-menu" role="menu" aria-label="Add preset" tabindex="-1" use:focusOnMount>
        <button class="ctx-item" role="menuitem" onclick={addBlank}>
          <FilePlus2 size={14} />
          <span>Blank preset</span>
        </button>
        <button class="ctx-item" role="menuitem" onclick={addFromProgram}>
          <Gamepad2 size={14} />
          <span>From a running program…</span>
        </button>
      </div>
    {/if}
  </div>
</nav>

{#if menu}
  <!-- Backdrop swallows the next click so the menu dismisses cleanly. -->
  <button
    class="menu-backdrop"
    aria-label="Close menu"
    onclick={closeMenu}
    oncontextmenu={(e) => {
      e.preventDefault();
      closeMenu();
    }}
  ></button>
  <div
    class="ctxmenu"
    style="left: {menu.x}px; top: {menu.y}px;"
    role="menu"
    aria-label="Preset actions"
    tabindex="-1"
    use:focusOnMount
  >
    {#if menu.slot === "Normal"}
      <div class="ctx-item locked" role="menuitem" aria-disabled="true">
        <Lock size={14} />
        <span>Native baseline — locked</span>
      </div>
    {:else}
      <button class="ctx-item" role="menuitem" onclick={menuRename}>
        <Pencil size={14} />
        <span>Rename</span>
      </button>
      <button class="ctx-item" role="menuitem" onclick={openBinder}>
        <Link2 size={14} />
        <span>Bind to program…</span>
      </button>
      {#if boundExe(menu.slot)}
        <button class="ctx-item" role="menuitem" onclick={menuUnbind}>
          <Unlink size={14} />
          <span>Unbind</span>
        </button>
      {/if}
      <button class="ctx-item danger" role="menuitem" onclick={menuDelete}>
        <Trash2 size={14} />
        <span>Delete</span>
      </button>
    {/if}
  </div>
{/if}

{#if binder}
  <button
    class="menu-backdrop modal"
    aria-label="Close binder"
    onclick={closeBinder}
  ></button>
  <div class="binder" role="dialog" aria-modal="true" aria-label="Bind program to preset" tabindex="-1" use:focusOnMount>
    <div class="binder-glow"></div>
    <header class="binder-head">
      <div class="binder-icon"><Gamepad2 size={18} /></div>
      <div class="binder-head-text">
        {#if binder.mode === "create"}
          <span>Create preset from a game</span>
          <span class="binder-sub">
            Pick a running game — a new preset is made and auto-bound to it.
          </span>
        {:else}
          <span>Bind a program</span>
          <span class="binder-sub">
            Auto-applies this preset while the program runs.
          </span>
        {/if}
      </div>
    </header>
    <button class="browse" onclick={browseExe}>
      <Link2 size={14} />
      Browse for .exe…
    </button>
    <div class="binder-or"><span></span>or pick a running program<span></span></div>
    <div class="proc-filter-row">
      <input
        class="proc-filter"
        placeholder="Filter…"
        bind:value={procFilter}
      />
      <button
        class="proc-refresh"
        title="Refresh list"
        aria-label="Refresh list"
        onclick={loadProcs}
      >
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
{/if}

<style>
  .rail {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 10px;
    width: 168px;
    background: var(--bg-inset);
    border-right: 1px solid var(--border);
    flex-shrink: 0;
    overflow: hidden;
  }
  .slots {
    display: flex;
    flex-direction: column;
    gap: 4px;
    overflow-y: auto;
    flex: 1;
    min-height: 0;
  }
  .slot {
    display: flex;
    align-items: center;
    border-radius: var(--radius);
    border: 1px solid transparent;
    transition: background 120ms ease, border-color 120ms ease;
  }
  .slot:hover { background: var(--surface-hover); }
  .slot.active {
    background: color-mix(in oklab, var(--slot-accent) 14%, transparent);
    border-color: color-mix(in oklab, var(--slot-accent) 40%, transparent);
  }
  .slot.targeted {
    border-color: color-mix(in oklab, var(--slot-accent) 60%, transparent);
  }
  .pick {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
    min-width: 0;
    padding: 9px 11px;
    background: transparent;
    border: none;
    color: var(--fg-muted);
    font: inherit;
    font-size: var(--fs-sm);
    cursor: pointer;
    text-align: left;
  }
  .slot.active .pick { color: var(--fg); }
  .slot:hover .pick { color: var(--fg-2); }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--slot-accent);
    flex-shrink: 0;
    box-shadow: 0 0 0 0 var(--slot-accent);
    transition: box-shadow 200ms ease;
  }
  .slot.active .dot {
    box-shadow: 0 0 8px 1px color-mix(in oklab, var(--slot-accent) 70%, transparent);
  }
  .dot.dirty {
    background: var(--warn);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--warn) 25%, transparent);
    animation: dirty-pulse 1600ms ease-in-out infinite;
  }
  @keyframes dirty-pulse {
    0%, 100% { box-shadow: 0 0 0 3px color-mix(in oklab, var(--warn) 25%, transparent); }
    50% { box-shadow: 0 0 0 5px color-mix(in oklab, var(--warn) 12%, transparent); }
  }
  .label {
    font-weight: 500;
    letter-spacing: 0.01em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rename {
    flex: 1;
    min-width: 0;
    background: var(--field);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-xs);
    color: var(--fg);
    font: inherit;
    font-size: var(--fs-sm);
    padding: 1px 5px;
    outline: none;
  }
  .rename:focus { border-color: var(--border-focus); }
  .add-wrap { position: relative; flex-shrink: 0; }
  .new {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 100%;
    padding: 9px 8px;
    border-radius: var(--radius);
    border: 1px solid color-mix(in oklab, var(--accent, var(--slot-a)) 40%, transparent);
    background: color-mix(in oklab, var(--accent, var(--slot-a)) 10%, transparent);
    color: color-mix(in oklab, var(--accent, var(--slot-a)) 92%, var(--fg));
    font: inherit;
    font-size: var(--fs-sm);
    font-weight: 500;
    cursor: pointer;
    transition: background 120ms ease, color 120ms ease, border-color 120ms ease;
  }
  .new:hover {
    background: color-mix(in oklab, var(--accent, var(--slot-a)) 18%, transparent);
    border-color: color-mix(in oklab, var(--accent, var(--slot-a)) 65%, transparent);
    color: var(--fg);
  }
  .new :global(.chev) { margin-left: -1px; opacity: 0.7; }
  .add-menu {
    position: absolute;
    bottom: calc(100% + 6px);
    left: 0;
    right: 0;
    z-index: 50;
    padding: 5px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: linear-gradient(180deg, var(--bg-elev-3), var(--bg-elev-2));
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg), inset 0 1px 0 color-mix(in oklab, white 5%, transparent);
    animation: ctx-in 110ms var(--ease-soft);
  }

  /* ── Context menu ── */
  .menu-backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: transparent;
    border: none;
    padding: 0;
    cursor: default;
  }
  .ctxmenu {
    position: fixed;
    z-index: 50;
    width: 180px;
    padding: 5px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: linear-gradient(180deg, var(--bg-elev-3), var(--bg-elev-2));
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg), inset 0 1px 0 color-mix(in oklab, white 5%, transparent);
    animation: ctx-in 110ms var(--ease-soft);
    transform-origin: top left;
  }
  @keyframes ctx-in {
    from { opacity: 0; transform: scale(0.95) translateY(-3px); }
    to { opacity: 1; transform: scale(1) translateY(0); }
  }
  .ctx-item {
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
  .ctx-item :global(svg) { flex-shrink: 0; opacity: 0.8; transition: opacity 110ms ease; }
  .ctx-item:hover {
    background: var(--surface-hover);
    color: var(--fg);
  }
  .ctx-item:hover :global(svg) { opacity: 1; }
  .ctx-item:active { transform: scale(0.98); }
  .ctx-item.danger:hover {
    background: var(--danger-soft);
    color: var(--danger);
  }
  .ctx-item.locked {
    color: var(--fg-faint);
    cursor: default;
    font-size: var(--fs-xs);
  }
  .ctx-item.locked:hover { background: transparent; color: var(--fg-faint); }

  /* ── Bound-program badge ── */
  .bound {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    max-width: 64px;
    margin-right: 8px;
    padding: 2px 5px;
    border-radius: var(--radius-xs);
    background: color-mix(in oklab, var(--slot-accent) 16%, transparent);
    color: color-mix(in oklab, var(--slot-accent) 90%, var(--fg));
    font-size: 10px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex-shrink: 0;
  }

  /* ── Binder modal ── */
  .menu-backdrop.modal {
    z-index: 60;
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
</style>
