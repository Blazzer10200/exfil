// Status-bar toasts. They replace the IN FRONT slot for a moment: ok/info
// auto-clear after 2.2s, errors linger 5s and may carry a right-side action.

export type ToastKind = "ok" | "info" | "error";
export type Toast = {
  kind: ToastKind;
  msg: string;
  spin?: boolean; // info only — leading ◌ spins (update download)
  action?: { label: string; run: () => void };
};

class ToastStore {
  current = $state<Toast | null>(null);
  #timer: ReturnType<typeof setTimeout> | undefined;

  #show(t: Toast, ms: number | null) {
    clearTimeout(this.#timer);
    this.current = t;
    if (ms !== null) this.#timer = setTimeout(() => (this.current = null), ms);
  }

  ok(msg: string) {
    this.#show({ kind: "ok", msg }, 2200);
  }
  info(msg: string, opts: { spin?: boolean; sticky?: boolean } = {}) {
    this.#show({ kind: "info", msg, spin: opts.spin }, opts.sticky ? null : 2200);
  }
  error(msg: string, action?: Toast["action"]) {
    this.#show({ kind: "error", msg, action }, 5000);
  }
  clear() {
    clearTimeout(this.#timer);
    this.current = null;
  }
}

export const toast = new ToastStore();

/** Tauri command errors arrive as plain strings; keep the reason short. */
export const reason = (e: unknown) => String(e).replace(/^Error:\s*/, "").slice(0, 80);
