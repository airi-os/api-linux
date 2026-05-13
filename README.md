# computer-use-linux

Linux desktop control for any MCP host — AT-SPI accessibility, GNOME Shell window targeting, portal screenshots, and ydotool input. Wayland-first, X11 best-effort.

<!-- Badges (enable once CI / crates.io publish lands):
[![CI](https://github.com/avifenesh/computer-use-linux/actions/workflows/ci.yml/badge.svg)](https://github.com/avifenesh/computer-use-linux/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/computer-use-linux.svg)](https://crates.io/crates/computer-use-linux)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
-->

## What this is

`computer-use-linux` is a single static Rust binary that exposes a Model Context Protocol (MCP) stdio server. Any MCP host — Codex Desktop's Linux build, Claude Desktop, [Hermes Agent](https://github.com/NousResearch/hermes-agent), or your own client — can spawn it and gain full control of the local Linux desktop: read accessibility trees, list and focus windows, take screenshots, click, drag, scroll, type, and invoke semantic accessibility actions.

Most computer-use MCP servers are macOS-only (they rely on AppKit, AXUIElement, CGEvent). The few that target Linux either drive `xdotool` against an X11 root window or shell out to OCR over screenshots. This crate is different on three points worth caring about:

- **Wayland actually works.** Input goes through the `org.freedesktop.portal.RemoteDesktop` interface when the compositor offers it, with `ydotool` / `ydotoold` (uinput) as a deterministic fallback. Screenshots use the GNOME Shell DBus screenshot method when present and `org.freedesktop.portal.Screenshot` otherwise.
- **Semantic selectors, not pixel coordinates.** Tools like `click`, `perform_action`, and `set_value` accept `role` / `name` / `text` / `states` selectors backed by AT-SPI. Pixel coordinates remain available as a fallback for rendering-only surfaces (canvas, games, X clients without ATK).
- **One JSON readiness report.** `computer-use-linux doctor` returns a structured document covering platform, portals, AT-SPI, windowing, input, and a `readiness` summary with explicit blockers and a recommended next step. MCP hosts can render or surface that to the user without parsing prose.

The crate was extracted from [`codex-desktop-linux`](https://github.com/avifenesh/codex-desktop-linux) (the Linux distribution of Codex Desktop), which still bundles this binary as a built-in plugin. This standalone repo is the upstream.

## Features

15 MCP tools, all stable as of `v0.1.0`:

**Diagnostics**
- `doctor` — single-shot JSON readiness report (platform, portals, accessibility, windowing, input, readiness summary)
- `setup_accessibility` — flips the gsettings keys that enable AT-SPI bridges in GTK / Qt / Electron toolkits
- `setup_window_targeting` — installs and enables the bundled GNOME Shell extension when `org.gnome.Shell.Introspect` is locked down

**Discovery**
- `list_apps` — running desktop apps visible to the AT-SPI registry
- `list_windows` — compositor windows with title, app id, wm_class, focus state, client type (Wayland/X11), and bounds
- `focused_window` — the window currently holding keyboard focus
- `get_app_state` — combined screenshot + accessibility tree for a chosen app, with element indices that the input tools accept

**Input**
- `click` — by element index, semantic selector, or pixel coordinates
- `drag` — pixel-coordinate drag (start / end)
- `scroll` — page-based scroll on an element or at a pixel location
- `press_key` — keys / chords; can focus a window or terminal first
- `type_text` — literal text input, optionally targeted at a window or terminal

**Semantic actions**
- `perform_action` — invoke any AT-SPI action exposed by an element (`Press`, `Activate`, `Toggle`, …); defaults to the primary action
- `set_value` — write to a settable accessibility element (text fields, sliders, spinners)

**Navigation**
- `activate_window` — focus a window by `window_id`, `pid`, `app_id`, `wm_class`, `title`, or terminal selectors

The binary also exposes the same capabilities from the CLI for scripting and debugging:

```
computer-use-linux mcp                                  # stdio MCP server
computer-use-linux doctor                               # JSON readiness report
computer-use-linux setup                                # enable AT-SPI
computer-use-linux setup-window-targeting               # install GNOME Shell extension
computer-use-linux apps
computer-use-linux state [APP_NAME]
computer-use-linux screenshot [--output FILE]
computer-use-linux screenshot-area X Y WIDTH HEIGHT [--output FILE]
computer-use-linux screenshot-window WINDOW_ID [--output FILE]
computer-use-linux windows
```

## Support matrix

Tested on Ubuntu 25.10 (GNOME Shell 50.1, Wayland). Should work on any glibc-based distro with GNOME 45+ and `ydotool` packaged.

- **GNOME on Wayland** — full support. AT-SPI, screenshots, window listing/focus (via Shell extension when introspection is locked), input via remote-desktop portal or `ydotool`. Primary target.
- **GNOME on X11** — best-effort. AT-SPI works, `ydotool` works, `xdg-desktop-portal-gtk` screenshots work. The bundled GNOME Shell extension is irrelevant on X11; window listing falls back to `org.gnome.Shell.Introspect` when permitted, otherwise window-targeted input is unavailable.
- **KDE Plasma (Wayland)** — untested. AT-SPI should work for Qt apps; `ydotool` should work; window targeting via the GNOME extension does not apply. Expect `windowing.can_list_windows = false` in `doctor`.
- **Sway / Hyprland (wlroots)** — untested. No GNOME Shell extension fallback exists. `ydotool` and AT-SPI will work; the remote-desktop portal coverage depends on `xdg-desktop-portal-wlr`.
- **X11 generic (i3, XFCE, …)** — best-effort. AT-SPI + `ydotool` only.

If you run on a desktop not in the validated row, please open an issue with the output of `computer-use-linux doctor` so we can extend the matrix honestly.

## Install

### Option A — `./install.sh` (recommended)

Clones the repo, builds the release binary, installs it to `~/.local/bin`, enables and starts `ydotoold` as a user service, flips the AT-SPI gsettings keys, and installs the bundled GNOME Shell extension at `gnome-shell-extension/computer-use-linux@avifenesh.dev/`.

```bash
git clone https://github.com/avifenesh/computer-use-linux
cd computer-use-linux
./install.sh
# log out and back in if the GNOME extension was newly installed
computer-use-linux doctor | jq .readiness
```

### Option B — `cargo install` (binary only)

You handle the system-level pieces (`ydotoold`, AT-SPI, the GNOME extension) yourself.

```bash
cargo install --git https://github.com/avifenesh/computer-use-linux
computer-use-linux doctor
```

Then, as needed:

```bash
sudo apt install ydotool                      # or your distro's equivalent
systemctl --user enable --now ydotoold
computer-use-linux setup                      # gsettings AT-SPI bridge
computer-use-linux setup-window-targeting     # GNOME Shell extension
```

### Option C — prebuilt binaries

Static x86_64 / aarch64 builds are published with each tag. Each archive ships a `.sha256` next to it.

- Releases: <https://github.com/avifenesh/computer-use-linux/releases>

```bash
curl -L -o computer-use-linux.tar.gz \
  https://github.com/avifenesh/computer-use-linux/releases/latest/download/computer-use-linux-x86_64-unknown-linux-gnu.tar.gz
curl -L -o computer-use-linux.tar.gz.sha256 \
  https://github.com/avifenesh/computer-use-linux/releases/latest/download/computer-use-linux-x86_64-unknown-linux-gnu.tar.gz.sha256
sha256sum -c computer-use-linux.tar.gz.sha256
tar xzf computer-use-linux.tar.gz
install -m 0755 computer-use-linux ~/.local/bin/
```

You will still need `ydotoold` running and AT-SPI enabled (run `computer-use-linux setup` and the systemd commands above).

## Wire it into your MCP host

The binary speaks the `rmcp` 2024-11-05 stdio protocol. Pass `mcp` as the only argument; everything else is configured through MCP tool calls.

### Codex Desktop (Linux build)

The Linux build of Codex Desktop already bundles this binary as a plugin. You don't need to wire it up manually — the plugin definition lives in [`codex-desktop-linux`](https://github.com/avifenesh/codex-desktop-linux) under its `plugins/` directory and is enabled by default. To upgrade the plugin in place, replace the binary it ships with the one from this repo's release tarball.

### Claude Desktop

Edit `~/.config/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "computer-use-linux": {
      "command": "computer-use-linux",
      "args": ["mcp"]
    }
  }
}
```

Restart Claude Desktop. The 15 tools should appear in the tools list.

### Hermes Agent

Edit `~/.hermes/config.yaml`:

```yaml
mcp_servers:
  computer-use-linux:
    command: computer-use-linux
    args: ["mcp"]

# Optional: expose the tools to subagents as well.
inherit_mcp_toolsets: true
```

### Generic MCP client

Spawn the binary with `["mcp"]` as the argv tail. It speaks JSON-RPC over stdio per the rmcp 2024-11-05 spec — no environment variables, no sockets, no flags. All capability discovery happens through `tools/list` and the `doctor` tool.

## First-run checklist

1. **Run `doctor`.**

   ```bash
   computer-use-linux doctor | jq .readiness
   ```

   Aim for `can_register_mcp_tools`, `can_build_accessibility_tree`, `can_send_development_input`, and `can_query_windows` all `true`. The `blockers` array should be empty.

2. **If `accessibility.at_spi_bus.ok = false`** — run `computer-use-linux setup` (or call the `setup_accessibility` MCP tool). This flips:
   - `org.gnome.desktop.interface toolkit-accessibility true`
   - the equivalent Qt / Electron environment defaults

   You may need to restart toolkit-using apps for the change to take effect.

3. **If `windowing.can_list_windows = false`** — run `computer-use-linux setup-window-targeting` (or call `setup_window_targeting`). This installs the bundled `computer-use-linux@avifenesh.dev` GNOME Shell extension and enables it. **Log out and log back in** so GNOME Shell loads the extension, then re-run `doctor`.

4. **Grant the screencast portal on first screenshot.** The first time `get_app_state` or any screenshot subcommand runs, GNOME will pop a portal dialog asking to share the screen. Accept once and tick "remember" to make it sticky for the session.

5. **Confirm `ydotoold` is running.**

   ```bash
   systemctl --user status ydotoold
   ```

   Its socket should appear at `/run/user/$UID/.ydotool_socket`.

## Architecture

- **Accessibility tree** — [`atspi`](https://crates.io/crates/atspi) crate (tokio backend) talks to the AT-SPI registry on the user session bus. The tree is flattened to `(role, name, text, states, bounds)` tuples and indexed; element indices are stable for the duration of a `get_app_state` snapshot.
- **DBus everywhere else** — [`zbus`](https://crates.io/crates/zbus) for portal calls (`org.freedesktop.portal.Screenshot`, `…RemoteDesktop`, `…ScreenCast`), GNOME Shell screenshots (`org.gnome.Shell.Screenshot`), and the bundled extension's `dev.avifenesh.ComputerUseLinux.WindowControl` service.
- **MCP transport** — [`rmcp`](https://crates.io/crates/rmcp) with the `transport-io` feature; stdio framing, no network.
- **Input fallback** — when the remote-desktop portal isn't available or the host wants deterministic injection, the binary writes to `ydotoold`'s socket, which writes to `/dev/uinput`. `setup` ensures the user is in the `input` group and `ydotoold` is enabled.
- **Window targeting on locked-down GNOME** — recent GNOME builds deny `org.gnome.Shell.Introspect.GetWindows` to non-blessed clients. The bundled GNOME Shell extension exposes the same data (and an activation method) under the DBus name `dev.avifenesh.ComputerUseLinux.WindowControl`. The binary prefers the extension when present, falls back to `Introspect` when allowed, and returns `can_list_windows: false` with a clear `recommended_next_step` otherwise.
- **Terminal enrichment** — `list_windows` cross-references each terminal window with its controlling TTY and the foreground process on that TTY, so `type_text` / `press_key` can target "the terminal where `pytest` is running" without the host ever knowing the window id.

## Security

Computer-use tooling is, by definition, a privilege-escalation surface. The threat model:

- **`ydotoold` runs as a per-user systemd service** with read/write access to `/dev/uinput`. Any process that can connect to its socket (`/run/user/$UID/.ydotool_socket`, mode `0600` by default) can synthesize arbitrary input — keypresses, clicks, anything. Keep the socket in the user runtime dir (the default), not in `/tmp` or any world-readable location. Do not run `ydotoold` as a system service.
- **The screencast portal asks for permission once per session.** Granting it lets the calling MCP host capture the screen for the rest of the session. If you don't want that, decline the portal dialog and use `get_app_state` with `include_screenshot: false`.
- **AT-SPI exposes window contents to any client on your session bus.** Enabling the AT-SPI bridge (`setup_accessibility`) is a prerequisite for this binary; it's also what screen readers use, and it shares the same trust boundary.
- **The GNOME Shell extension** is loaded only into your user's GNOME Shell, runs in the Shell's JS sandbox, and exposes a single DBus interface on the user session bus. It does not request any extra permissions.
- **No network.** This binary opens no sockets, makes no outbound connections, and ships no telemetry.

If you're running this on a shared workstation, set `ydotoold`'s socket permissions to `0600` (the default) and audit which processes on your user can `connect()` to it.

## Troubleshooting

`computer-use-linux doctor` is the source of truth. Common failure modes and fixes:

- **`accessibility.at_spi_bus.ok = false`** — AT-SPI registry isn't running or the toolkit bridge is off. Fix: `computer-use-linux setup` (or call the `setup_accessibility` MCP tool). Restart the apps you want to drive.
- **`windowing.gnome_shell_introspect.ok = false` and `gnome_shell_extension_dbus.ok = false`** — GNOME blocks introspection and the extension isn't installed. Fix: `computer-use-linux setup-window-targeting`, then log out and log back in.
- **`input.ydotool_socket.ok = false`** — daemon isn't running. Fix: `systemctl --user enable --now ydotoold`. If the unit doesn't exist, install the `ydotool` package and rerun `./install.sh` (or copy the unit from `systemd/ydotoold.service` in this repo).
- **`input.uinput.ok = false`** — `/dev/uinput` isn't accessible to your user. Fix: add yourself to the `input` group (`sudo usermod -aG input $USER`) and re-login. On distros that ship `uinput` as a kernel module without auto-loading it, add `uinput` to `/etc/modules-load.d/`.
- **Portal calls hang or time out** — `xdg-desktop-portal` or its backend (`-gnome`, `-gtk`, `-kde`, `-wlr`) crashed. Fix: check `journalctl --user -u xdg-desktop-portal -u xdg-desktop-portal-gnome --since '5 min ago'` and restart the relevant unit.
- **Screenshots return black frames on multi-monitor setups** — known wlroots / mixed-DPI quirk. Use `screenshot-window` or `screenshot-area` with explicit bounds instead of full-screen capture.
- **`type_text` types into the wrong window** — pass an explicit target (`window_id`, `pid`, `wm_class`, `title`, or for terminals `tty` / `terminal_pid` / `terminal_command` / `terminal_cwd`). Without a target, input goes to whatever window currently has compositor focus.

If `doctor` is green and a specific tool still misbehaves, file an issue with the JSON output of `doctor` and the failing tool's request payload.

## Credits

Extracted from [`codex-desktop-linux`](https://github.com/avifenesh/codex-desktop-linux), the Linux distribution of Codex Desktop, which continues to ship this same binary as a bundled plugin. Maintained by [Avi Fenesh](https://github.com/avifenesh).

Built on top of:

- [`atspi`](https://crates.io/crates/atspi) — AT-SPI bindings
- [`zbus`](https://crates.io/crates/zbus) — async DBus
- [`rmcp`](https://crates.io/crates/rmcp) — MCP runtime
- [`ydotool`](https://github.com/ReimuNotMoe/ydotool) — Wayland-friendly uinput driver

## License

MIT — see [LICENSE](LICENSE).
