# Omarchy-shell plugin (Omarchy 4+)

Omarchy 4 replaced Waybar with `omarchy-shell` (Quickshell). agent-bar
integrates as a third-party bar-widget plugin.

## What setup installs

`agent-bar setup` detects omarchy-shell (`omarchy` CLI on PATH +
`/usr/share/omarchy/shell/`) and writes the drop-in:

```
~/.config/omarchy/plugins/agent-bar.usage/
  manifest.json          # id agent-bar.usage, version = binary version
  Widget.qml             # chips + popup (consumes `agent-bar --format json`)
  icons/                 # provider icons
  scripts/agent-bar-open-terminal
```

It then runs `omarchy plugin rescan`, `omarchy plugin enable agent-bar.usage`
and `omarchy bar plugin add agent-bar.usage` (best-effort: failures become
warnings and the commands can be run manually).

If Waybar is also installed, the classic Waybar flow runs alongside.

## Widget

- One chip per provider (icon + remaining % of the primary limit), shell
  theme colors, severity mirroring the TUI (≥60 ok / 30-59 / 10-29 / <10).
- Left click: native popup (primary/secondary windows, per-model breakdown,
  reset times, plan/account).
- Right click: opens the TUI (`agent-bar menu`) in a floating terminal.
- Middle click: forced refresh (`--refresh`).
- `refreshIntervalSec` setting (default 60, min 30) via
  `omarchy bar plugin set` or `shell.json`.

## Data

The QML runs `agent-bar --format json` (contract in
[`json-output.md`](json-output.md)). The QML files are embedded in the
binary — version-locked with the schema. After `agent-bar update`, re-run
`agent-bar setup` to refresh the drop-in (the update prints this hint).

## Removal

`agent-bar uninstall`/`remove` unregisters the widget
(`omarchy bar plugin remove` + `omarchy plugin remove`, best-effort) and
deletes the plugin directory.

## Testing

The flow is covered by `cargo test omarchy_integration` and
`cargo test setup` with temp dirs (`--omarchy-plugins-dir`). The QML has no
automated harness: visual changes require manual verification on a desktop.
