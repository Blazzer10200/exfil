<script lang="ts">
  // 52px icon rail: Color / Crosshair / Games · spacer · Settings · vendor dot.
  import { Palette, Crosshair, Gamepad2, Settings } from "lucide-svelte";
  import { app } from "./state.svelte";
  import type { View } from "./prefs";

  const items: { id: View; label: string; icon: typeof Palette }[] = [
    { id: "color", label: "Color", icon: Palette },
    { id: "crosshair", label: "Crosshair", icon: Crosshair },
    { id: "games", label: "Games", icon: Gamepad2 },
  ];

  const vendorTip = $derived(
    app.vendor === "nvidia"
      ? "NVIDIA · NVAPI digital vibrance ready"
      : app.vendor === "amd"
        ? "AMD · ADL saturation ready"
        : "No vibrance driver detected",
  );
  const isActive = (id: View) => app.view === id || (id === "crosshair" && app.view === "library");
</script>

<nav class="rail" aria-label="Sections">
  <img class="logo" src="/favicon.png" alt="EXFIL" draggable="false" />
  {#each items as it (it.id)}
    <button
      class="rb"
      class:active={isActive(it.id)}
      title={it.label}
      aria-label={it.label}
      aria-current={isActive(it.id) ? "page" : undefined}
      onclick={() => app.setView(it.id)}
    >
      <it.icon size={18} strokeWidth={1.75} />
      {#if it.id === "games" && app.inFrontBound}<span class="badge ok"></span>{/if}
    </button>
  {/each}
  <span class="spacer"></span>
  <button
    class="rb"
    class:active={app.view === "settings"}
    title="Settings"
    aria-label="Settings"
    onclick={() => app.setView("settings")}
  >
    <Settings size={18} strokeWidth={1.75} />
    {#if app.updateMeta}<span class="badge accent"></span>{/if}
  </button>
  <span class="vendor" class:off={!app.vendor} title={vendorTip}></span>
</nav>

<style>
  .rail {
    width: 52px;
    flex: 0 0 52px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 10px 0 12px;
    background: var(--hud-rail);
    border-right: 1px solid var(--hud-line);
  }
  .logo {
    width: 26px;
    height: 26px;
    border-radius: var(--hud-r);
    margin-bottom: 12px;
    -webkit-user-drag: none;
  }
  .rb {
    position: relative;
    width: 40px;
    height: 38px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: var(--hud-r);
    background: transparent;
    color: var(--hud-fg-dim);
    cursor: pointer;
    transition: color 160ms var(--ease-hud), background 160ms var(--ease-hud);
  }
  .rb:hover {
    color: var(--hud-fg);
  }
  .rb.active {
    color: var(--accent);
    background: var(--accent-soft);
  }
  .rb.active::before {
    content: "";
    position: absolute;
    left: -6px;
    top: 8px;
    width: 2px;
    height: 22px;
    background: var(--accent);
    border-radius: 1px;
  }
  .badge {
    position: absolute;
    right: 7px;
    top: 7px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }
  .badge.ok {
    background: var(--ok);
    box-shadow: 0 0 6px color-mix(in oklab, var(--ok) 70%, transparent);
  }
  .badge.accent {
    background: var(--accent);
    box-shadow: 0 0 6px color-mix(in oklab, var(--accent) 70%, transparent);
  }
  .spacer {
    flex: 1;
  }
  .vendor {
    width: 6px;
    height: 6px;
    margin-top: 6px;
    border-radius: 50%;
    background: var(--telemetry);
    box-shadow: 0 0 8px color-mix(in oklab, var(--telemetry) 80%, transparent);
  }
  .vendor.off {
    background: var(--hud-fg-faint);
    box-shadow: none;
  }
</style>
