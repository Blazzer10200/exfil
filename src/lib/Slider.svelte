<script lang="ts">
  // Bare range track (no label/readout — Dial / SliderRow wrap it).
  // The thumb tracks the pointer continuously (input step="any") while the
  // VALUE snaps to `step`; on release / external change the thumb eases onto
  // the snapped value, so coarse integer ranges still feel fluid.
  import { untrack } from "svelte";

  interface Props {
    value: number;
    min: number;
    max: number;
    step: number;
    label: string; // aria only
    disabled?: boolean;
    tone?: "accent" | "telemetry";
    onchange?: (v: number) => void;
  }
  let { value = $bindable(), min, max, step, label, disabled = false, tone = "accent", onchange }: Props = $props();

  // Visual thumb position — NaN until the first sync (no glide on mount).
  let pos = $state(NaN);
  let dragging = false;
  let raf = 0;

  const shown = $derived(Number.isNaN(pos) ? value : pos);
  const pct = $derived(Math.min(100, Math.max(0, ((shown - min) / (max - min)) * 100)));

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

<input
  type="range"
  class="track {tone}"
  aria-label={label}
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

<style>
  .track {
    -webkit-appearance: none;
    appearance: none;
    display: block;
    width: 100%;
    height: 10px;
    margin: 0;
    --fill: var(--accent);
    background: linear-gradient(to right, var(--fill) 0%, var(--fill) var(--pct), var(--hud-track) var(--pct), var(--hud-track) 100%)
      center / 100% 2px no-repeat;
    outline: none;
    cursor: pointer;
    touch-action: none;
    transition: background-size 120ms var(--ease-hud);
  }
  .track.telemetry {
    --fill: var(--telemetry);
  }
  .track:hover:not(:disabled) {
    background-size: 100% 3px;
  }
  .track:disabled {
    cursor: default;
  }
  .track::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 8px;
    height: 10px;
    border: 0;
    border-radius: 1px;
    background: var(--fill);
    transition: transform 120ms var(--ease-hud), box-shadow 120ms var(--ease-hud);
  }
  .track:hover::-webkit-slider-thumb {
    transform: scaleY(1.25);
  }
  .track:active::-webkit-slider-thumb {
    box-shadow: 0 0 0 4px color-mix(in oklab, var(--fill) 22%, transparent);
  }
  .track:focus-visible::-webkit-slider-thumb {
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--fill) 35%, transparent);
  }
</style>
