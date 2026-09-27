<script lang="ts">
  // Two-click destructive chip: first click arms (fills red, "CLICK TO
  // CONFIRM"), second runs. Disarms after 4s, on Escape, or on blur.
  interface Props {
    label: string;
    confirm?: string;
    disabled?: boolean;
    size?: "xs" | "" | "lg";
    onconfirm: () => void;
  }
  let { label, confirm = "CLICK TO CONFIRM", disabled = false, size = "", onconfirm }: Props = $props();
  let armed = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  function disarm() {
    clearTimeout(timer);
    armed = false;
  }
  function click() {
    if (!armed) {
      armed = true;
      clearTimeout(timer);
      timer = setTimeout(disarm, 4000);
      return;
    }
    disarm();
    onconfirm();
  }
  $effect(() => () => clearTimeout(timer));
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && armed && disarm()} />

<button class="chip danger {size}" class:armed {disabled} onclick={click} onblur={disarm}>
  {armed ? confirm : label}
</button>
