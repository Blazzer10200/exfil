<script lang="ts">
  // Crosshair part-panel row: 70px label · track · 36px mono value.
  import Slider from "./Slider.svelte";

  interface Props {
    label: string;
    value: number;
    min: number;
    max: number;
    step?: number;
    format?: (v: number) => string;
    disabled?: boolean;
    onchange?: (v: number) => void;
  }
  let { label, value = $bindable(), min, max, step = 1, format, disabled = false, onchange }: Props = $props();
</script>

<div class="row" class:off={disabled}>
  <span class="label display">{label}</span>
  <Slider bind:value {min} {max} {step} {label} {disabled} {onchange} />
  <span class="val mono">{format ? format(value) : value}</span>
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: 70px minmax(0, 1fr) 36px;
    align-items: center;
    gap: 8px;
  }
  .row.off {
    opacity: 0.45;
  }
  .label {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.12em;
    color: var(--hud-fg-dim);
    white-space: nowrap;
  }
  .val {
    font-size: 11px;
    color: var(--hud-fg-2);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
</style>
