<script lang="ts">
  // Tactical HUD shell: icon rail + (titlebar · view · status bar).
  import { onMount } from "svelte";
  import Rail from "$lib/Rail.svelte";
  import Titlebar from "$lib/Titlebar.svelte";
  import StatusBar from "$lib/StatusBar.svelte";
  import ColorView from "$lib/ColorView.svelte";
  import CrosshairView from "$lib/CrosshairView.svelte";
  import GamesView from "$lib/GamesView.svelte";
  import SettingsView from "$lib/SettingsView.svelte";
  import { app } from "$lib/state.svelte";

  let ready = $state(false);
  onMount(() => {
    app.boot().finally(() => (ready = true));
    return () => app.dispose();
  });
</script>

<div class="shell">
  <Rail />
  <div class="column">
    <Titlebar />
    {#if ready}
      {#key app.view === "library" ? "crosshair" : app.view}
        <div class="view">
          {#if app.view === "color"}
            <ColorView />
          {:else if app.view === "crosshair" || app.view === "library"}
            <CrosshairView />
          {:else if app.view === "games"}
            <GamesView />
          {:else}
            <SettingsView />
          {/if}
        </div>
      {/key}
    {:else}
      <div class="view"></div>
    {/if}
    <StatusBar />
  </div>
</div>

<style>
  .shell {
    position: fixed;
    inset: 0;
    display: flex;
    background: var(--hud-bg);
    color: var(--hud-fg);
  }
  .column {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .view {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    animation: hud-fade 160ms var(--ease-hud);
  }
</style>
