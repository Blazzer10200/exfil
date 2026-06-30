# EXFIL v2

Per-game color / gamma / digital-vibrance preset tool. Tauri 2 (Rust) + Svelte 5,
with a Rift-inspired dark UI. A from-scratch port of the .NET EXFIL v1.

## What it does

- **Gamma / brightness / contrast** — driver-level GDI gamma ramps (`Set/GetDeviceGammaRamp`).
  Applied to **every gamma-capable monitor** (`\\.\DISPLAY1..N`, probed directly).
- **Digital vibrance** — NVAPI (`SetDVCLevelEx`, 0..=63 ex-scale) via raw `nvapi64.dll` `QueryInterface`.
  Applied to **every connected NVIDIA output** — so a second monitor can't keep a stale value.
- **Four preset slots** — Normal (native baseline), Preset 1, Preset 2, Preset 3.
  Persisted to `%APPDATA%\exfil-v2\presets.json`. Last-active preset re-applied on boot.
  **Normal** restores each monitor's NATIVE color — neutral gamma + per-monitor default vibrance —
  so the display picks up exactly what Windows/the driver programmed it to.

No DLL injection — every write goes through the Windows display driver / NVAPI, so it's
BattlEye / EAC-safe. No telemetry.

## Stack

Svelte 5 (runes) · SvelteKit (adapter-static SPA) · Vite · Tailwind v4 · TypeScript
· Tauri 2.11 · Rust 2021 · `windows` 0.58 (GDI) · raw NVAPI FFI.

## Dev

```bash
npm install
npx @tauri-apps/cli dev      # hot-reload dev
npx @tauri-apps/cli build    # release exe + NSIS installer
```

Build output: `C:\cargo-targets\release\exfil-v2.exe` and
`...\bundle\nsis\EXFIL_2.0.0_x64-setup.exe`.
