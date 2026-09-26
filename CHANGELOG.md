# Changelog

All notable changes to EXFIL. Earlier releases are described in their git tags.

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
