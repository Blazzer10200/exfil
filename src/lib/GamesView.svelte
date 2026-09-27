<script lang="ts">
  // GAMES: one row per bound program, joining the preset + crosshair bindings.
  import { Monitor, ShieldCheck } from "lucide-svelte";
  import { NO_CROSSHAIR, slotAccent } from "./api";
  import { app, type Binding } from "./state.svelte";
  import ProgramPicker from "./ProgramPicker.svelte";
  import ArmChip from "./ArmChip.svelte";

  let picker = $state(false);
  const userPresets = $derived(app.presets.filter((p) => p.slot !== "Normal"));

  const title = (exe: string) => exe.replace(/\.exe$/i, "").toUpperCase();
  const mono = (exe: string) => exe.replace(/[^a-z0-9]/gi, "").slice(0, 2).toUpperCase() || "?";
  // Monogram tint = the bound preset's palette accent (same index rule as the Color strip).
  function accent(b: Binding): string | null {
    if (!b.preset) return null;
    const i = userPresets.findIndex((p) => p.slot === b.preset!.slot);
    return i < 0 ? null : slotAccent(b.preset.slot, i);
  }

  async function setPreset(b: Binding, slot: string) {
    if (slot === "") {
      if (b.preset) await app.bindPreset(b.preset.slot, null);
    } else await app.bindPreset(slot, b.exe);
  }
  async function setCrosshair(b: Binding, id: string) {
    if (id === "") {
      if (b.crosshair) await app.bindCrosshair(b.crosshair.id, null);
    } else await app.bindCrosshair(id, b.exe);
  }
  async function unbind(b: Binding) {
    if (b.preset) await app.bindPreset(b.preset.slot, null);
    if (b.crosshair) await app.bindCrosshair(b.crosshair.id, null);
  }
  async function edit(b: Binding) {
    if (b.preset) {
      await app.pickPreset(b.preset.slot);
      app.setView("color");
    } else if (b.crosshair) {
      await app.pickCrosshair(b.crosshair.id);
      app.setView("crosshair");
    }
  }
  const boundTo = (exe: string) => app.bindings.find((b) => b.exe === exe)?.preset?.name ?? null;
</script>

<div class="head">
  <span class="title display">BINDINGS</span>
  <span class="help mono">which preset + crosshair switch in while a program is in front</span>
  <span class="grow"></span>
  <button class="chip lg accent" onclick={() => (picker = true)}>+ BIND GAME</button>
</div>

<div class="body" class:grid={app.grid}>
  {#if app.bindings.length}
    <div class="cols mono">
      <span></span><span>PROGRAM</span><span>COLOR PRESET</span><span>CROSSHAIR</span><span class="st">STATUS</span><span></span>
    </div>
    <div class="row desktop">
      <span class="mono-gram"><Monitor size={11} /></span>
      <span class="prog"><span class="t display">DESKTOP</span><span class="e mono">fallback when no bound program is in front</span></span>
      <span class="pill dim">NORMAL ▾</span>
      <span class="pill dim dashed">NONE ▾</span>
      <span class="status mono" class:on={!app.inFrontBound}>
        <span class="dot"></span><span class="st">{app.inFrontBound ? "STANDBY" : "ACTIVE"}</span>
      </span>
      <span class="acts"><span class="locked mono">LOCKED</span></span>
    </div>
    {#each app.bindings as b, i (b.exe)}
      {@const front = app.inFront === b.exe}
      {@const ac = accent(b)}
      <div class="row" class:front style="--i: {i}">
        <span class="mono-gram" class:tinted={!!ac} style={ac ? `--ac: ${ac}` : ""}>{mono(b.exe)}</span>
        <span class="prog"><span class="t display">{title(b.exe)}</span><span class="e mono">{b.exe}</span></span>
        <select class="pill sel" class:dashed={!b.preset} value={b.preset?.slot ?? ""} onchange={(e) => void setPreset(b, e.currentTarget.value)}>
          <option value="">NONE</option>
          {#each userPresets as p (p.slot)}<option value={p.slot}>{p.name.toUpperCase()}</option>{/each}
        </select>
        <select class="pill sel" class:dashed={!b.crosshair} value={b.crosshair?.id ?? ""} onchange={(e) => void setCrosshair(b, e.currentTarget.value)}>
          <option value="">NONE</option>
          {#each app.crosshairs.filter((c) => c.id !== NO_CROSSHAIR) as c (c.id)}<option value={c.id}>{c.name.toUpperCase()}</option>{/each}
        </select>
        <span class="status mono" class:on={front}>
          <span class="dot" class:blink={front}></span><span class="st">{front ? "RUNNING" : "IDLE"}</span>
        </span>
        <span class="acts">
          <button class="chip xs" onclick={() => void edit(b)}>EDIT</button>
          <ArmChip label="✕" confirm="SURE?" size="xs" onconfirm={() => void unbind(b)} />
        </span>
      </div>
    {/each}
  {:else}
    <div class="empty">
      <span class="display et">NO GAMES BOUND YET</span>
      <span class="mono ed">bind a running game and EXFIL switches your preset + crosshair in when it's in front</span>
      <span class="eb">
        <button class="chip lg accent" onclick={() => (picker = true)}>PICK A RUNNING GAME</button>
        <button class="chip lg" onclick={() => app.setView("color")}>TUNE COLOR FIRST</button>
      </span>
    </div>
  {/if}
  <div class="callout mono">
    <ShieldCheck size={13} />
    DETECTION IS A READ-ONLY WINDOW/PROCESS ENUMERATION · NO INJECTION · NO HOOKS · BATTLEYE / EAC SAFE
  </div>
</div>

{#if picker}
  <ProgramPicker sub="A new preset named after the game, bound to it" onpick={(exe, t) => void app.createPresetFromGame(exe, t)} onclose={() => (picker = false)} {boundTo} />
{/if}

<style>
  .head {
    height: 44px;
    flex: 0 0 44px;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 16px;
    border-bottom: 1px solid var(--hud-line);
  }
  .title {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.12em;
    color: var(--hud-fg-hi);
  }
  .help {
    font-size: 10px;
    color: var(--hud-fg-low);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .grow {
    flex: 1;
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px;
  }
  .body.grid {
    background-image: var(--hud-grid);
    background-size: 24px 24px;
  }
  .cols,
  .row {
    display: grid;
    grid-template-columns: 22px 1.4fr 1fr 1fr 96px 84px;
    gap: 12px;
    align-items: center;
  }
  .cols {
    padding: 0 12px 6px;
    font-family: var(--font-display);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.14em;
    color: var(--hud-fg-low);
    border-bottom: 1px solid var(--hud-line-2);
    margin-bottom: -4px;
  }
  .row {
    padding: 10px 12px;
    border: 1px solid var(--hud-line-2);
    border-radius: var(--hud-r);
    background: var(--hud-panel);
    animation: hud-in 220ms var(--ease-hud) both;
    animation-delay: calc(var(--i, 0) * 40ms);
    transition: background 120ms var(--ease-hud), border-color 160ms var(--ease-hud);
  }
  .row:hover {
    background: var(--hud-hover);
  }
  .row.front {
    border-color: color-mix(in oklab, var(--ok) 35%, var(--hud-line-2));
  }
  .row.desktop {
    background: var(--hud-panel-2);
  }
  .mono-gram {
    width: 16px;
    height: 16px;
    display: grid;
    place-items: center;
    border-radius: 2px;
    background: var(--hud-pill);
    font-family: var(--font-mono);
    font-size: 8px;
    font-weight: 700;
    color: var(--hud-fg-3);
  }
  .mono-gram.tinted {
    background: color-mix(in oklab, var(--ac) 22%, var(--hud-pill));
    border: 1px solid color-mix(in oklab, var(--ac) 55%, transparent);
    color: var(--ac);
  }
  .prog {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .prog .t {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .prog .e {
    font-size: 9.5px;
    color: var(--hud-fg-low);
  }
  .pill {
    height: 24px;
    display: inline-flex;
    align-items: center;
    padding: 0 8px;
    border: 1px solid var(--hud-line-3);
    border-radius: var(--hud-r);
    background: var(--hud-pill);
    font-family: var(--font-display);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
    color: var(--hud-fg-2);
    white-space: nowrap;
  }
  .pill.dim {
    opacity: 0.55;
  }
  .pill.dashed {
    border-style: dashed;
    color: var(--hud-fg-dim);
  }
  .sel {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    cursor: pointer;
    background-image: linear-gradient(45deg, transparent 50%, var(--hud-fg-dim) 50%), linear-gradient(135deg, var(--hud-fg-dim) 50%, transparent 50%);
    background-position: calc(100% - 12px) 10px, calc(100% - 8px) 10px;
    background-size: 4px 4px, 4px 4px;
    background-repeat: no-repeat;
    padding-right: 22px;
  }
  .sel:hover {
    border-color: var(--hud-line-5);
  }
  .sel:focus {
    outline: none;
    border-color: var(--accent);
  }
  .sel option {
    background: var(--hud-panel);
    color: var(--hud-fg);
  }
  .status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    letter-spacing: 0.08em;
    color: var(--hud-fg-faint);
  }
  .status.on {
    color: var(--ok);
  }
  .status .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--hud-fg-faint);
  }
  .status.on .dot {
    background: var(--ok);
    box-shadow: 0 0 6px color-mix(in oklab, var(--ok) 70%, transparent);
  }
  .blink {
    animation: hud-blink 1.6s ease-in-out infinite;
  }
  .locked {
    font-size: 9.5px;
    letter-spacing: 0.1em;
    color: var(--hud-fg-faint);
  }
  .acts {
    display: flex;
    justify-content: flex-end;
    gap: 4px;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 34px 20px;
    border: 1px dashed var(--hud-line-4);
    border-radius: var(--hud-r);
    text-align: center;
  }
  .et {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.14em;
    color: var(--hud-fg-2);
  }
  .ed {
    font-size: 10px;
    color: var(--hud-fg-low);
    max-width: 380px;
    line-height: 1.5;
  }
  .eb {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }
  .callout {
    margin-top: auto;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border: 1px solid color-mix(in oklab, var(--telemetry) 25%, transparent);
    border-radius: var(--hud-r);
    background: color-mix(in oklab, var(--telemetry) 5%, transparent);
    font-size: 9.5px;
    letter-spacing: 0.06em;
    color: var(--telemetry-text);
  }
  @media (max-width: 780px) {
    .st {
      display: none;
    }
  }
</style>
