# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-05-13

### Added
- Initial public release as a standalone repository, extracted from
  [`codex-desktop-linux-local-stack`](https://github.com/avifenesh/codex-desktop-linux).
- Linux Computer Use MCP server (`computer-use-linux` binary) speaking
  [rmcp](https://docs.rs/rmcp) over stdio.
- 15 MCP tools: `doctor`, `setup_accessibility`, `setup_window_targeting`,
  `list_apps`, `get_app_state`, `list_windows`, `focused_window`,
  `activate_window`, `click`, `drag`, `scroll`, `press_key`, `type_text`,
  `perform_action`, `set_value`.
- AT-SPI accessibility tree with semantic element selectors (role / name /
  text / states) for `click`, `perform_action`, and `set_value`.
- GNOME Shell window targeting via the bundled
  `computer-use-linux@avifenesh.dev` Shell extension (DBus service
  `dev.avifenesh.ComputerUseLinux.WindowControl`), with automatic fallback to
  `org.gnome.Shell.Introspect` when the extension is not installed.
- Screenshot capture through GNOME Shell DBus (preferred) and
  `org.freedesktop.portal.Screenshot` (fallback). Supports full-screen,
  per-app, per-window, region, and per-element scopes.
- Input synthesis through the Wayland remote-desktop portal when available,
  falling back to `ydotool` / `ydotoold` for keystrokes and pointer events.
- Best-effort terminal-window enrichment: maps each terminal window to its
  active TTY and foreground process for targeted `type_text` / `press_key`.
- `doctor` subcommand reporting AT-SPI bus health, GNOME Shell introspection
  status, extension status, ydotool socket readiness, and portal coverage in
  a single JSON document.

### Architecture
- Wayland-first; X11 best-effort through AT-SPI + ydotool.
- Validated against GNOME 50.1 on Wayland (Ubuntu 25.10).
- KDE / Sway / Hyprland untested — see README support matrix.

[Unreleased]: https://github.com/avifenesh/computer-use-linux/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/avifenesh/computer-use-linux/releases/tag/v0.1.0
