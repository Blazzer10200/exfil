# Changelog

All notable changes to EXFIL. Earlier releases are described in their git tags.

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
