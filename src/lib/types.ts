import type { ComponentType, SvelteComponent } from "svelte";

/** A lucide-svelte icon (legacy class component) usable as `<it.icon size={13} />`. */
export type IconComponent = ComponentType<SvelteComponent<{ size?: number | string; strokeWidth?: number | string }>>;
