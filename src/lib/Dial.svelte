<script lang="ts">
  // Color-view dial card: label + big mono readout, 2px track, range labels.
  import Slider from "./Slider.svelte";

  interface Props {
    label: string;
    value: number;
    min: number;
    max: number;
    step: number;
    format: (v: number) => string;
    range: [string, string];
    suffix?: string; // e.g. "/100" rendered small after the value
    tone?: "accent" | "telemetry";
    disabled?: boolean;
    unavailable?: boolean; // vendor missing → dimmed, dash readout, footnote
    footnote?: string;
    onchange?: (v: number) => void;
  }
  let {
    label,
    value = $bindable(),
    min,
    max,
    step,
    format,
    range,
    suffix,
    tone = "accent",
    disabled = false,
    unavailable = false,
    footnote,
    onchange,
  }: Props = $props();
</script>

<div class="dial panel" class:telemetry={tone === "telemetry"} class:off={disabled} class:na={unavailable}>
  <div class="head">
    <span class="label display">{label}</span>
    <span class="value mono">
      {#if unavailable}—{:else}{format(value)}{#if suffix}<span class="suffix">{suffix}</span>{/if}{/if}
    </span>
  </div>
  <Slider bind:value {min} {max} {step} {label} {tone} disabled={disabled || unavailable} {onchange} />
  <div class="range mono">
    <span>{range[0]}</span>
    <span>{range[1]}</span>
  </div>
  {#if unavailable && footnote}
    <div class="foot">{footnote}</div>
  {/if}
</div>

<style>
  .dial {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    transition: opacity 160ms var(--ease-hud);
  }
  .dial.telemetry {
    border-color: color-mix(in oklab, var(--telemetry) 30%, var(--hud-line-2));
  }
  .dial.telemetry .label {
    color: var(--telemetry-text);
  }
  .dial.off {
    opacity: 0.45;
  }
  .dial.na {
    opacity: 0.5;
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  .label {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.14em;
    color: var(--hud-fg-muted);
  }
  .value {
    font-size: 20px;
    font-weight: 500;
    color: var(--hud-fg-hi);
    font-variant-numeric: tabular-nums;
    line-height: 1;
  }
  .suffix {
    font-size: 11px;
    color: var(--hud-fg-faint);
    margin-left: 1px;
  }
  .range {
    display: flex;
    justify-content: space-between;
    font-size: 9px;
    color: var(--hud-fg-faint);
  }
  .foot {
    font-family: var(--font-mono);
    font-size: 9.5px;
    color: var(--hud-fg-low);
    line-height: 1.4;
  }
</style>
