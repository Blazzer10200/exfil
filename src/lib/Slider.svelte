<script lang="ts">
  // A labeled range slider with a live numeric readout and accent fill.
  // The thumb tracks the pointer continuously (input step="any") while the
  // VALUE snaps to `step`; on release / external change the thumb eases onto
  // the snapped value, so coarse integer ranges still feel fluid.
  import { untrack } from "svelte";

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

  // Visual thumb position — NaN until the first sync (no glide on mount).
  let pos = $state(NaN);
  let dragging = false;
  let raf = 0;

  const shown = $derived(Number.isNaN(pos) ? value : pos);
  const pct = $derived(Math.min(100, Math.max(0, ((shown - min) / (max - min)) * 100)));
  const display = $derived(format ? format(value) : `${value}${unit}`);

  function quantize(v: number) {
    const decimals = (String(step).split(".")[1] ?? "").length;
    const q = Math.round((v - min) / step) * step + min;
    return Number(Math.min(max, Math.max(min, q)).toFixed(decimals));
  }

  function commit(v: number) {
    const q = quantize(v);
    if (q !== value) {
      value = q;
      onchange?.(q);
    }
  }

  function glideTo(target: number) {
    cancelAnimationFrame(raf);
    const from = untrack(() => pos);
    if (Number.isNaN(from) || from === target) {
      pos = target;
      return;
    }
    const t0 = performance.now();
    const tick = (now: number) => {
      const k = Math.min(1, (now - t0) / 140);
      pos = from + (target - from) * (1 - (1 - k) ** 3);
      if (k < 1) raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
  }

  // External value changes (preset switch, re-center) glide the thumb too.
  $effect(() => {
    const v = value;
    if (!dragging) glideTo(v);
  });
  $effect(() => () => cancelAnimationFrame(raf));

  function oninput(e: Event) {
    dragging = true;
    cancelAnimationFrame(raf);
    pos = parseFloat((e.target as HTMLInputElement).value);
    commit(pos);
  }

  function release() {
    if (!dragging) return;
    dragging = false;
    glideTo(value);
  }

  // step="any" leaves keyboard steps to the browser — do them explicitly.
  function onkeydown(e: KeyboardEvent) {
    const big = Math.max(step, (max - min) / 10);
    const delta: Record<string, number> = {
      ArrowRight: step,
      ArrowUp: step,
      ArrowLeft: -step,
      ArrowDown: -step,
      PageUp: big,
      PageDown: -big,
    };
    let next: number | undefined;
    if (e.key in delta) next = value + delta[e.key];
    else if (e.key === "Home") next = min;
    else if (e.key === "End") next = max;
    if (next === undefined) return;
    e.preventDefault();
    commit(next);
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
    aria-valuetext={display}
    {min}
    {max}
    step="any"
    value={shown}
    {disabled}
    {oninput}
    onchange={release}
    onpointerup={release}
    {onkeydown}
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
    display: block;
    width: 100%;
    height: 6px;
    margin: 5px 0;
    border-radius: 999px;
    background: linear-gradient(
      to right,
      var(--accent) 0%,
      var(--accent) var(--pct),
      var(--track) var(--pct),
      var(--track) 100%
    );
    box-shadow: inset 0 1px 2px oklch(0 0 0 / 0.35);
    outline: none;
    cursor: pointer;
    touch-action: none;
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
    transition: transform 120ms var(--ease-soft, ease), box-shadow 120ms ease;
  }
  input[type="range"]:hover::-webkit-slider-thumb { transform: scale(1.12); }
  input[type="range"]:active::-webkit-slider-thumb {
    transform: scale(1.22);
    box-shadow: var(--shadow-sm), 0 0 0 6px color-mix(in oklab, var(--accent) 22%, transparent);
  }
  input[type="range"]:focus-visible::-webkit-slider-thumb {
    box-shadow: 0 0 0 3px var(--ring);
  }
</style>
