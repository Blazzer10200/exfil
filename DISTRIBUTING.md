# Distributing EXFIL to other machines

Notes for sharing the installer with friends. The app was already built
defensively (optional NVAPI, live monitor probing, per-user paths), so there's
no code that assumes *your* specific machine — this is a heads-up on what to
expect, not a list of blockers.

## What to send

Point them at the **GitHub Releases page**, or hand them the
`EXFIL_x.y.z_x64-setup.exe` from your local build output
(`...\release\bundle\nsis\`). Either way it's the same self-contained NSIS
installer: it embeds the WebView2 bootstrapper, so it'll silently install
WebView2 on first run if the target machine doesn't already have it (Windows 11
ships it; most Windows 10 installs get it via Windows Update, but a
debloated/offline machine might not).

## What already Just Works on another PC

- **Non-NVIDIA GPUs (AMD/Intel).** `Nvapi::load()` tries to load `nvapi64.dll`
  and returns `None` on failure instead of erroring — the app runs fine,
  vibrance just shows "No NVAPI" and the slider disables itself. Gamma/
  brightness/contrast work on any GPU.
- **Different monitor counts/layouts.** `gamma::display_dcs()` probes
  `\\.\DISPLAY1..16` live and only keeps ones that actually support gamma
  ramps — no assumption baked in about a specific setup.
- **Presets are per-user.** Stored at `%APPDATA%\exfil-v2\presets.json`,
  created fresh on first run.

## What to tell your friends before they run it

1. **SmartScreen / AV warning is expected.** The installer and exe are
   unsigned (no code-signing cert) — Windows will likely show "Windows
   protected your PC." Click **More info → Run anyway**. Some AV heuristics
   may also flag it because the app does raw NVAPI DLL calls + read-only
   process enumeration (for the per-game auto-switch) — both are legitimate
   here (see `CLAUDE.md`'s no-injection constraint) but look similar to
   patterns malware uses, so a false positive isn't surprising.
2. **HDR displays may see odd gamma behavior.** `SetDeviceGammaRamp` has
   documented undefined behavior on some GPU/driver combos when HDR is
   active. If sliders act weird, **Normal** always restores native color —
   it's a full reset, not a stored preset.
3. **First launch creates its own preset file** — nothing carries over from
   your machine. If you want to share your presets, use **Export** (titlebar
   `⋮` menu) → hand them the JSON → they **Import** it on their end. Import
   is additive, so it won't clobber anything they've already made.

## Not done (intentionally, for a "couple of buddies" scale)

- **No code signing.** Worth it if this ever goes wider; not worth the cost/
  process for a handful of friends who can click through SmartScreen once.
- **No auto-update.** Re-send the installer if you ship a new build; Tauri's
  updater plugin isn't wired in. Fine at this scale, adds real complexity
  (signing + an update server/manifest) if added.
