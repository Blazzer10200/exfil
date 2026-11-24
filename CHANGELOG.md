# Changelog

All notable changes to EXFIL. Earlier releases are described in their git tags.

## [Unreleased]

### Added
- **Smooth gradients** (Settings → General, NVIDIA only). Switches on the GPU
  driver's temporal dithering, which hides the banding a preset can cause on an
  8-bit signal. Off by default; EXFIL hands dithering back to the driver when
  you turn it off or quit. Driver-level like everything else: no injection.
- **Shades readout** on the Color page. Shows how many of the 256 shades the
  current gamma, brightness and contrast keep (256 means none are lost),
  counted for the bit depth your outputs actually carry (8-bit or 10-bit).

### Removed
- **Crosshair overlay.** The Crosshair tab, the crosshair library, the overlay
  window, per-game crosshair bindings, the tray CROSSHAIR item and the
  **Ctrl+Shift+F11** / **Ctrl+Shift+F12** hotkeys are gone. EXFIL is a color
  tool again: presets, per-game auto-switch, and the two color hotkeys.
  A `crosshairs.json` left in `%APPDATA%\exfil-v2\` by an earlier version is
  no longer read and can be deleted.

### Changed
- **Global hotkeys are down to two**: **Ctrl+Shift+F9** cycles presets and
  **Ctrl+Shift+F10** snaps to Normal.
- The Games page shows one preset per program; picking another preset in the
  row's dropdown moves the program to it.
- Internal cleanup with no behaviour change: the backend is split into focused
  modules, the frontend lost its unused options, and CI now runs the
  typecheck, rustfmt, clippy and the unit tests on every push.

### Fixed
- **A damaged `presets.json` is no longer overwritten.** If the file can't be
  read or parsed, EXFIL keeps a copy as `presets.json.bad` before starting
  from defaults, and it repairs a file with a missing Normal preset or a
  reused preset key.
- **Quitting could leave a preset's gamma on screen.** The once-a-second
  re-apply could run between the exit reset and the process ending and write
  the old ramp back.
- **A failed update check no longer reports "UP TO DATE"**, and an update
  download keeps its progress if you leave the Settings page.
- **Auto-switch picked the wrong game** after a program was unbound and bound
  again while another bound game was running.
- **"In front" could show a stale program** for a few seconds when switching
  between the desktop and a File Explorer window.
- Rename and Bind from a preset's right-click menu could act on a different
  preset if selecting it failed.
- NVIDIA loader: a driver DLL without `NvAPI_Initialize` is now handled
  instead of being undefined behaviour; a driver reporting an inverted
  vibrance range is an error instead of a crash.
- AMD: a small leak when a display query failed.
- **White box / hung app on start-with-Windows.** The main window was created
  visible and only hidden at the end of startup, and the tray popup webview was
  built inline, so a slow WebView2 at boot stalled startup with an empty white
  window on screen. The window now starts hidden (`"visible": false`) and the
  tray popup is built on its own thread.

## [2.5.1] — 2026-09-26

### Fixed
- **Crosshair centering.** The center dot and outer lines now sit exactly on the
  main stroke's center even when their pixel width has the opposite parity
  (a 2px dot on 1px arms, 1px outer lines on 2px arms). Before, they snapped a
  whole pixel right/down and looked off-center.
- **"From running program"** on a game that already has a crosshair or preset
  now opens the existing one instead of creating a stock duplicate that silently
  took over the binding, leaving your tuned crosshair unbound.

## [2.5.0] — 2026-09-26

### Changed
- **New "Tactical HUD" interface.** Every screen was rebuilt to the Claude Design
  handoff: a 52px icon rail (Color · Crosshair · Games · Settings), a breadcrumb
  titlebar, horizontal section strips instead of side rails, dial cards for the
  color controls, and a status bar that shows the GPU vendor, what's in front, and
  every result toast.
- **Crosshair editor** is now a zoomable stage (1×–8×, pixel grid, 1:1 inset,
  backdrop picker including your own screenshot) with collapsible part panels:
  shape (Cross / X / T / Chevron / Brackets), color + glow, inner and outer lines,
  center dot, ring, outline, position with nudge, and a **share code** you can
  copy or paste. Valorant and CS2 codes paste straight in.
- **Settings** is a page instead of a modal.

### Added
- **Crosshair library** — 12 starter crosshairs with a live preview, one-click
  USE TEMPLATE, a 5-second on-screen preview, and code import.
- **Games** page — one row per bound program showing which preset and crosshair
  switch in, with pills to change either and a live RUNNING / IDLE status.
- **Ctrl+Shift+F12** cycles crosshairs; the tray menu gained a CROSSHAIR ON/OFF item
  and shows the active preset + crosshair.

## [2.4.0] — 2026-09-26

### Added
- **Crosshairs tab** — a crosshair overlay with its own editor: arms, dot, ring,
  outline, T-style, color, opacity and offset, with a pixel-exact live preview.
- Four built-in crosshairs, plus create / rename / duplicate / delete from the
  crosshair list's right-click menu.
- **Per-game crosshairs** — bind a crosshair to a game (or create one straight
  from a running game) and it switches in while that game is in front.
- **None** entry: show no crosshair outside bound games. Its panel lists every
  per-game crosshair.
- **Ctrl+Shift+F11** toggles the crosshair overlay (follows the global hotkeys
  setting).

### Changed
- Sliders now glide smoothly with the pointer and snap to their step on release.
- The preset and crosshair lists share the same Add ▾ popover and right-click
  menu styling.

### Notes
- The overlay is a click-through window on the primary monitor. It shows over
  borderless and windowed games, not exclusive fullscreen.
- Still no injection: foreground-game detection uses the same read-only process
  snapshot as the color auto-switch.
