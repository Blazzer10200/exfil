//! Per-program auto-switch watcher. Polls the running-process set on an interval
//! (read-only `CreateToolhelp32Snapshot` — NO injection, NO hooks; just an
//! enumeration the OS already exposes) and, when a bound program starts or stops,
//! tells the app which preset slot should be active.
//!
//! Priority is MOST-RECENT-LAUNCH: among all bound programs currently running,
//! the one that started most recently wins. When none are running, the app
//! reverts to the user's last manual pick (resolved by the callback, not here).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use crate::process;

static WATCHER_RUNNING: AtomicBool = AtomicBool::new(false);

/// Start the watcher thread. `bindings()` returns the live (exe→slot) map each
/// tick (read from the store). `on_change(Some(slot))` fires when a bound program
/// becomes the most-recent-running winner; `on_change(None)` fires when the last
/// bound program exits (caller reverts to its manual pick). Idempotent.
pub fn start<B, C>(interval_ms: u64, bindings: B, on_change: C)
where
    B: Fn() -> Vec<(String, String)> + Send + 'static,
    C: Fn(Option<String>) + Send + 'static,
{
    if WATCHER_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    thread::spawn(move || {
        // launch_seq orders bound exes by first-seen-running, so "most recent
        // launch" = highest seq among the currently-running bound set.
        let mut seq: u64 = 0;
        let mut launch_seq: HashMap<String, u64> = HashMap::new();
        let mut last_winner: Option<String> = None;
        let mut announced_any = false;

        loop {
            thread::sleep(Duration::from_millis(interval_ms));

            let binds = bindings();
            // exe -> slot, only for bound exes
            let bind_map: HashMap<String, String> = binds.into_iter().collect();

            // Nothing bound → skip the full process snapshot entirely. Enumerating
            // every process on the machine every tick is pure waste when there's
            // no exe to match; for a background tray utility with no bindings (the
            // common case) this keeps the watcher thread effectively idle. If a
            // bound winner was active when the last binding was cleared, revert to
            // the manual pick once. (`!announced_any` preserves the original
            // first-tick announce that establishes the baseline on boot.)
            if bind_map.is_empty() {
                launch_seq.clear();
                if last_winner.is_some() || !announced_any {
                    announced_any = true;
                    last_winner = None;
                    on_change(None);
                }
                continue;
            }

            let running = process::running_exes();

            // Update launch ordering. Forget any exe that stopped or was
            // unbound, so a relaunch (or a re-bind) orders it as new; then
            // assign a seq the first tick a bound exe is seen running.
            launch_seq.retain(|exe, _| bind_map.contains_key(exe) && running.contains(exe));
            for exe in bind_map.keys() {
                if running.contains(exe) && !launch_seq.contains_key(exe) {
                    seq += 1;
                    launch_seq.insert(exe.clone(), seq);
                }
            }

            // Winner = bound+running exe with the largest launch seq.
            let winner_slot = launch_seq
                .iter()
                .max_by_key(|(_, s)| **s)
                .and_then(|(exe, _)| bind_map.get(exe).cloned());

            if winner_slot != last_winner || !announced_any {
                announced_any = true;
                last_winner = winner_slot.clone();
                on_change(winner_slot);
            }
        }
    });
}
