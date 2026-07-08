<script lang="ts">
  // A labeled range slider with a live numeric readout and accent fill.
  interface Props {
    label: string;
    value: number;
    min: number;
    max: number;
    step: number;
    unit?: string;
    format?: (v: number) => string;
    disabled?: boolean;
    onchange?: (v: number) => void;
  }
  let {
    label,
    value = $bindable(),
    min,
    max,
    step,
    unit = "",
    format,
    disabled = false,
    onchange,
  }: Props = $props();

  const pct = $derived(((value - min) / (max - min)) * 100);
  const display = $derived(format ? format(value) : `${value}${unit}`);

  function handle(e: Event) {
    const v = parseFloat((e.target as HTMLInputElement).value);
    value = v;
    onchange?.(v);
  }
</script>

<div class="slider" class:disabled>
  <div class="row">
    <span class="field-label">{label}</span>
    <span class="readout mono">{display}</span>
  </div>
  <input
    type="range"
    aria-label={label}
    {min}
    {max}
    {step}
    {value}
    {disabled}
    oninput={handle}
    style="--pct: {pct}%"
  />
</div>

<style>
  .slider { margin-bottom: 14px; }
  .slider.disabled { opacity: 0.45; pointer-events: none; }
  .row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: 6px;
  }
  .field-label { margin-bottom: 0; }
  .readout {
    font-size: var(--fs-sm);
    color: var(--fg-2);
    font-variant-numeric: tabular-nums;
  }
  input[type="range"] {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 6px;
    border-radius: 999px;
    background: linear-gradient(
      to right,
      var(--accent) 0%,
      var(--accent) var(--pct),
      var(--track) var(--pct),
      var(--track) 100%
    );
    outline: none;
    cursor: pointer;
  }
  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--fg);
    border: 3px solid var(--accent);
    box-shadow: var(--shadow-sm);
    transition: transform 80ms ease;
  }
  input[type="range"]::-webkit-slider-thumb:hover { transform: scale(1.15); }
  input[type="range"]:focus-visible::-webkit-slider-thumb {
    box-shadow: 0 0 0 3px var(--ring);
  }
</style>
