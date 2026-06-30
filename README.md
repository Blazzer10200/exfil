# EXFIL v2

Per-game color / gamma / digital-vibrance preset tool. Tauri 2 (Rust) + Svelte 5,
with a Rift-inspired dark UI. A from-scratch port of the .NET EXFIL v1.

## What it does

- **Gamma / brightness / contrast** — driver-level GDI gamma ramps (`Set/GetDeviceGammaRamp`).
- **Digital vibrance** — NVAPI (`SetDVCLevelEx`, 0..=63 ex-scale) via raw `nvapi64.dll` `QueryInterface`.
- **Six preset slots** — Normal (read-only neutral baseline), Day, Night, Custom, Preset4, Preset5.
  Persisted to `%APPDATA%\exfil-v2\presets.json`. Last-active preset re-applied on boot.

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
