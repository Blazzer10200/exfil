<p align="center">
  <img src="src-tauri/app-icon.png" width="90" alt="EXFIL logo" />
</p>

<h1 align="center">EXFIL</h1>

<p align="center">
  Per-game color, gamma &amp; digital-vibrance presets for Windows.<br/>
  Driver-level only — <strong>no injection, BattlEye / EAC-safe</strong>. No telemetry.
</p>

<p align="center">
  <img src="docs/screenshot.jpg" width="740" alt="EXFIL main window" />
</p>

## Install

1. Download the latest `EXFIL_x.y.z_x64-setup.exe` from **[Releases](../../releases)** and run it.
2. Windows will likely show a SmartScreen warning ("Windows protected your PC") because the
   installer is unsigned — click **More info → Run anyway**.
3. That's it. EXFIL lives in the system tray, starts with Windows (toggleable in
   Settings — the gear at the bottom of the icon rail), and installs per-user (no admin needed). The
   installer embeds the WebView2 bootstrapper, so it works on machines that don't
   have WebView2 yet.

**Requirements:** Windows 10/11 x64. Gamma/brightness/contrast work on any GPU;
**digital vibrance needs an NVIDIA or AMD GPU** (on Intel or anything else the
slider simply disables itself and everything else keeps working).

More detail on running it on a machine that isn't yours — HDR caveats, AV false positives,
sharing presets — in [`DISTRIBUTING.md`](./DISTRIBUTING.md).

## What it does

- **Gamma / brightness / contrast** — driver-level GDI gamma ramps (`Set/GetDeviceGammaRamp`),
  applied to **every gamma-capable monitor** (`\\.\DISPLAY1..N`, probed live).
- **Digital vibrance** — on NVIDIA via NVAPI (`SetDVCLevelEx`, raw `nvapi64.dll`
  `QueryInterface`); on AMD via ADL display saturation (raw `atiadlxx.dll`, the same
  control Radeon software exposes). Either way it's applied to **every connected
  output** so a second monitor never keeps a stale value.
- **Your own presets** — a fixed read-only **Normal** baseline (restores each monitor's
  native color) plus presets you create, rename, and delete, tuned live from the color
  controls. Persisted per-user; the last-active preset re-applies on boot.
- **Per-game auto-switch** — bind a preset to a program and EXFIL applies it automatically
  while that program runs, reverting when it exits. Create a preset straight from a running
  game (**+ NEW → From a running program…**, or **+ BIND GAME** on the Games page), or bind/unbind from the preset's
  right-click menu or the main panel. Detection is a read-only window/process enumeration —
  no injection, no hooks.
- **Global hotkeys** — **Ctrl+Shift+F9** cycles through your presets, **Ctrl+Shift+F10**
  snaps back to Normal — from inside a game, with EXFIL hidden in the tray
  (toggleable in Settings).
- **Lives in the tray** — closing the window hides to tray; the tray menu has
  **Show / Reset display / Quit**. The active ramp is re-asserted on an interval so
  fullscreen games can't permanently steal the gamma.
- **Always exits clean** — every exit path (tray Quit, OS shutdown) restores native
  gamma + vibrance, so the screen is never left tinted with no app to clear it.
- **Import / export** — share presets as JSON (Settings page); import is additive, never
  overwrites, and rescales vibrance when the file came from a machine with the other
  GPU vendor.
- **Auto-update** — checks GitHub Releases on launch and installs from
  **Settings → About**; every update is signature-verified before it runs.

## Anti-cheat safety

EXFIL never touches game processes. Gamma goes through the Windows display driver
(GDI), vibrance through NVIDIA's public NVAPI or AMD's public ADL, and the auto-switch
watcher only *reads* the process list (`CreateToolhelp32Snapshot` / `EnumWindows`) — no
DLL injection, no hooks, no memory access. This is the same class of access the NVIDIA
Control Panel, AMD Radeon software, and Windows itself use, which is what keeps it
BattlEye / EAC-safe.

## Build from source

Needs Windows 10/11, Node 22+ and a stable Rust toolchain.

```bash
npm install
npm run tauri dev      # hot-reload dev
npm run tauri build    # release exe + NSIS installer
```

Outputs land in `src-tauri/target/release/` (or `%CARGO_TARGET_DIR%\release\` if you set
one): `exfil-v2.exe` and `bundle/nsis/EXFIL_x.y.z_x64-setup.exe`.

Tagged pushes (`v*`) build the installer on CI and attach it to a draft GitHub Release
(see [`.github/workflows/release.yml`](.github/workflows/release.yml)).

**Stack:** Svelte 5 (runes) · SvelteKit (adapter-static SPA) · Vite · hand-authored OKLCH
CSS · TypeScript · Tauri 2 · Rust 2021 · `windows` crate (GDI) · raw NVAPI + AMD ADL FFI.

### Checks

The same four checks run in CI ([`ci.yml`](.github/workflows/ci.yml)) on every push to
`main` and every pull request:

| Check | Command |
| --- | --- |
| Frontend typecheck | `npm run check` |
| Rust format | `npm run fmt:rust -- --check` |
| Rust lint | `npm run lint:rust` |
| Rust tests | `npm run test:rust` |

The Rust commands need a built frontend (`npm run build`) because the backend embeds it.

### Project layout

```
src/                     Svelte frontend
  routes/+page.svelte    app shell: icon rail, titlebar, view, status bar
  routes/tray/           the styled tray-menu popup (its own window)
  lib/*View.svelte       the three pages: Color, Games, Settings
  lib/state.svelte.ts    app-wide state and actions
  lib/api.ts             typed wrappers over the backend commands
  lib/prefs.ts           UI-only preferences (localStorage)
  app.css                design tokens and shared classes
src-tauri/src/           Rust backend
  lib.rs                 startup wiring
  state.rs               shared state + the display operations everything uses
  commands.rs            IPC commands (presets, bindings, import/export, system)
  hotkeys.rs             global shortcuts
  tray.rs                tray icon + menu popup
  updater.rs             signed auto-update
  store.rs               preset model + persistence
  gamma.rs               GDI gamma ramps
  vibrance.rs            vendor-neutral vibrance over nvapi.rs / amd.rs
  watcher.rs             per-program auto-switch
  foreground.rs          foreground-program tracker
```

## Data

Presets live at `%APPDATA%\exfil-v2\presets.json` — created fresh on first run,
per-user, nothing else is written anywhere. No telemetry, no analytics, no crash
reporting.

## License

[MIT](./LICENSE)
