---
name: computer-use-linux
description: "Use when Hermes needs Linux desktop observation or control through the computer-use-linux MCP server."
version: 0.2.1
author: agent-sh
license: MIT
platforms: [linux]
---

# computer-use-linux

Use `computer-use-linux` when Hermes needs to observe or operate a local Linux desktop through MCP: inspect the accessibility tree, list/focus windows, take screenshots, click, scroll, type, press keys, or invoke AT-SPI actions.

## When to Use

Use this skill when:
- The user wants Hermes to control a Linux GUI app.
- You need desktop state from AT-SPI, screenshots, or compositor window metadata.
- You are configuring the `computer-use-linux` MCP server for Hermes.
- A desktop action needs target-aware input instead of blind shell commands.

Do not use this for remote browsers, websites, or headless automation when a browser-specific tool is available. Do not assume desktop actions are safe just because the MCP connection works.

## Install

Preferred install for Hermes users:

```bash
npm install -g @agent-sh/computer-use-linux
computer-use-linux doctor | jq .readiness
```

Rust users can install the same server from crates.io:

```bash
cargo install computer-use-linux
computer-use-linux doctor | jq .readiness
```

If `doctor` reports missing input or accessibility support, run:

```bash
computer-use-linux setup
systemctl --user enable --now ydotoold
computer-use-linux setup-window-targeting
computer-use-linux doctor | jq .readiness
```

On GNOME Wayland, log out and back in after `setup-window-targeting` if the GNOME Shell extension was newly installed.

## Configure Hermes

Add the server with the Hermes MCP CLI:

```bash
hermes mcp add computer-use-linux --command computer-use-linux --args mcp
hermes mcp test computer-use-linux
hermes mcp configure computer-use-linux
```

`configure` opens Hermes' tool-selection UI for this MCP server.

The generated config should look like this:

```yaml
mcp_servers:
  computer-use-linux:
    command: computer-use-linux
    args: ["mcp"]
    timeout: 120
    connect_timeout: 30
```

If the binary is not on `PATH`, pass the absolute path to `--command`.

Hermes registers tools using the `mcp_<server>_<tool>` pattern. With this config, tool names are prefixed as `mcp_computer_use_linux_`, for example:

| MCP tool | Hermes tool name |
| --- | --- |
| `doctor` | `mcp_computer_use_linux_doctor` |
| `get_app_state` | `mcp_computer_use_linux_get_app_state` |
| `list_windows` | `mcp_computer_use_linux_list_windows` |
| `click` | `mcp_computer_use_linux_click` |
| `type_text` | `mcp_computer_use_linux_type_text` |

Restart Hermes after changing MCP config.

## Procedure

1. Start every desktop-control session with `doctor`.
2. If `can_build_accessibility_tree` is false, run `setup` and restart the target app.
3. If `can_query_windows` is false on GNOME Wayland, run `setup-window-targeting` and ask the user to log out and back in if setup says the shell extension needs a reload.
4. Before targeted input, call `list_windows` or `focused_window` and verify the intended window by title, app id, pid, or wm class.
5. Prefer semantic targeting from `get_app_state`: use element indices or role/name/text/states selectors.
6. Use coordinates only when the UI surface has no useful accessibility tree.
7. For text input, prefer `type_text` with a target selector (`window_id`, `pid`, `app_id`, `wm_class`, `title`, `tty`, `terminal_pid`, `terminal_command`, or `terminal_cwd`) rather than relying on current focus.
8. After mutating actions, re-check state with `get_app_state`, `focused_window`, or an app-specific readback.

## Pitfalls

- Already-running GTK, Qt, and Electron apps may need a restart after AT-SPI is enabled.
- GNOME may show a portal prompt on the first screenshot or `get_app_state` call with screenshots enabled.
- Desktop input is stateful. Avoid concurrent tool calls against this MCP server.
- `click`, `drag`, `press_key`, `type_text`, `perform_action`, and `set_value` can change real application state.
- `ydotoold` should run as a per-user service with its socket under `/run/user/$UID`, not as a system-wide service.
- On COSMIC, the standard npm, Cargo, and install-script paths install the `computer-use-linux-cosmic` helper automatically. Manual binary installs must copy both binaries.

## Verification

Run:

```bash
computer-use-linux doctor | jq .readiness
hermes chat --toolsets mcp-computer-use-linux -q "List the current desktop windows."
```

Ready output should have:

- `can_register_mcp_tools: true`
- `can_build_accessibility_tree: true`
- `can_query_windows: true`
- `can_send_development_input: true`
- `blockers: []`

If Hermes does not expose the tools, check startup logs for MCP discovery errors and confirm the server name in `config.yaml` is exactly `computer-use-linux`.
