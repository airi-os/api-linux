# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-05-14

### Added
- **Multi-compositor window targeting**: native backends for **KWin**
  (KDE Plasma), **Hyprland**, **i3**, and **COSMIC** Wayland alongside
  the existing GNOME Shell backend. Window listing, focus tracking, and
  activation now work across all five compositors with automatic backend
  selection at runtime.
- **COSMIC Wayland helper** (`computer-use-linux-cosmic` binary) that
  speaks the `zcosmic_toplevel_info_v1` and `zcosmic_toplevel_manager_v1`
  protocols, used by the main server when running under COSMIC.
- **`windowing/` crate-internal module** consolidating all backends behind
  a uniform `WindowBackend` trait, with a registry that picks the right
  backend per session.
- **Datagram ydotool socket support** in addition to the existing stream
  sockets. Aligns with `ydotoold`'s newer default and avoids a
  reconnection penalty per input event.
- **Raw-keycode keyboard input** path for ydotool, fixing keystroke
  delivery on layouts where the symbolic keysym path was unreliable.
- **Compact Linux accessibility trees** in `get_app_state` — deduplicated
  redundant container nodes for smaller, more focused snapshots.
- **Enriched AT-SPI state readback** — element states (`focused`,
  `selected`, `expanded`, `checked`, …) now flow through to the response
  schema for `get_app_state`.

### Changed
- Server-side rejection of empty window-backend results (returns a
  structured "no backend available" error instead of an empty list).
- Stale-client eviction in the chrome-extension host path on backend
  side (host binary itself not shipped here — see Removed).

### Removed
- **`codex-chrome-extension-host` binary** is intentionally not shipped
  in this fork — it is a Chrome native messaging host scoped to Codex
  browser automation (`com.openai.codexextension`), unrelated to the
  computer-use MCP. The cosmic helper binary is renamed to
  `computer-use-linux-cosmic` to match the project naming.

### Synced from upstream
- This release tracks
  [`ilysenko/codex-desktop-linux`](https://github.com/ilysenko/codex-desktop-linux)
  through commit `4d6fd96` (May 2026), then re-applies the rebrand
  (DBus names, env vars, GNOME extension UUID, cache-file prefixes).
  Upstream credit goes to ilysenko, mosesmrima, PinguuSS, and the
  original Codex contributors.

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

[Unreleased]: https://github.com/avifenesh/computer-use-linux/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/avifenesh/computer-use-linux/releases/tag/v0.2.0
[0.1.0]: https://github.com/avifenesh/computer-use-linux/releases/tag/v0.1.0
