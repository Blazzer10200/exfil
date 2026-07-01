# EXFIL v2

Per-game color / gamma / digital-vibrance preset tool. Tauri 2 (Rust) + Svelte 5,
with a Rift-inspired dark UI. A from-scratch port of the .NET EXFIL v1.

## What it does

- **Gamma / brightness / contrast** — driver-level GDI gamma ramps (`Set/GetDeviceGammaRamp`).
  Applied to **every gamma-capable monitor** (`\\.\DISPLAY1..N`, probed directly).
- **Digital vibrance** — NVAPI (`SetDVCLevelEx`, 0..=63 ex-scale) via raw `nvapi64.dll` `QueryInterface`.
  Applied to **every connected NVIDIA output** — so a second monitor can't keep a stale value.
- **Your own presets** — a fixed read-only **Normal** baseline plus presets you
  **create, name, rename, and delete** yourself, each tuned live from the main color
  controls. Persisted to `%APPDATA%\exfil-v2\presets.json`; last-active preset
  re-applied on boot. **Normal** restores each monitor's NATIVE color — neutral gamma
  + per-monitor default vibrance — so the display picks up exactly what Windows/the
  driver programmed it to. (User presets get stable internal keys `p{n}` from a
  monotonic counter, so renames touch only the display name and deleted keys never
  collide.)
- **Per-game auto-switch** — bind a preset to a program and EXFIL applies it
  automatically while that program runs, reverting when it exits. The fastest path is
  the **From game** button in the preset rail: it lists your running games (by window
  title), and picking one creates a preset already bound to it — no file browsing.
  (You can also bind from a preset's right-click menu, or browse for an `.exe`.)
  Detection is a read-only window/process enumeration — no injection, no hooks.
- **Lives in the tray** — runs in the background from a system-tray icon. Left-click
  the icon (or **Show EXFIL** in its menu) opens the window; the menu also has
  **Reset display** and **Quit**. Closing the window (X) **hides to tray** instead of
  quitting — the active ramp keeps being re-asserted.
- **Starts with Windows** — registers an autostart entry (HKCU Run key) and launches
  **hidden to the tray** (`--hidden`) on login, so your last look is applied from boot.

No DLL injection — every write goes through the Windows display driver / NVAPI, so it's
BattlEye / EAC-safe. No telemetry.

## Stack

Svelte 5 (runes) · SvelteKit (adapter-static SPA) · Vite · hand-authored OKLCH CSS (no framework) · TypeScript
· Tauri 2.11 · Rust 2021 · `windows` 0.58 (GDI) · raw NVAPI FFI.

## Dev

```bash
npm install
npx @tauri-apps/cli dev      # hot-reload dev
npx @tauri-apps/cli build    # release exe + NSIS installer
```

Build output: `C:\cargo-targets\release\exfil-v2.exe` and
`...\bundle\nsis\EXFIL_2.0.0_x64-setup.exe`.
