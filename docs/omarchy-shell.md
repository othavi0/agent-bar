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

- One chip per **enabled** provider (icon + % of the primary limit), shell
  theme colors, severity mirroring the TUI (≥60 ok / 30-59 / 10-29 / <10).
  Enabled set and order come from `waybar.providers` /
  `waybar.provider_order` in `~/.config/agent-bar/settings.json` (same
  keys the TUI Config editor uses). The quota poll
  (`agent-bar --format json`) still returns the full envelope; the QML
  filters chips and usage sections client-side.
- **Left click:** native usage popup (primary/secondary windows, per-model
  breakdown, reset times, plan/account). Footer has a link
  **Abrir menu (TUI)** that launches `agent-bar menu` via the terminal
  helper.
- **Right click:** same popup in **settings mode** (`settingsMode`) —
  providers on/off and order, display remaining/used, desktop notify
  toggle, refresh interval. Not the full TUI.
- **Middle click:** forced refresh (`--refresh`).
- `refreshIntervalSec` (default 60, min 30, max 3600) lives on the plugin
  entry in Omarchy `shell.json`, not in agent-bar `settings.json`.

### Settings save (dual-write)

One Save button, two writers:

| Field | Written by | Where |
| --- | --- | --- |
| `providers`, `providerOrder`, `displayMode`, `notify.enabled` | `agent-bar config apply` | `~/.config/agent-bar/settings.json` |
| `refreshIntervalSec` | `bar.shell.updateEntryInline` | plugin entry in Omarchy `shell.json` |

If `config apply` fails, the interval is **not** written. If apply succeeds
but inline fails, settings are saved and the UI reports that the interval
was not persisted.

### `config apply` does not reload Waybar

`agent-bar config apply` only patches and saves the settings subset. It does
**not** call `apply_waybar_integration` or reload Waybar. On a mixed
desktop (Waybar + Omarchy), enabled providers in `settings.json` stay
aligned for both; Waybar module layout only updates on the next TUI Config
save or `agent-bar setup`.

## Data

The QML runs `agent-bar --format json` (contract in
[`json-output.md`](json-output.md)). Editable settings for the popup come
from `agent-bar config show` / stdout of `config apply` (contract in
[`commands.md`](commands.md)). The QML files are embedded in the binary —
version-locked with the schema. After `agent-bar update`, re-run
`agent-bar setup` to refresh the drop-in (the update prints this hint).

## Removal

`agent-bar uninstall`/`remove` unregisters the widget
(`omarchy bar plugin remove` + `omarchy plugin remove`, best-effort) and
deletes the plugin directory.

## Testing

The flow is covered by `cargo test omarchy_integration` and
`cargo test setup` with temp dirs (`--omarchy-plugins-dir`). The QML has no
automated harness: visual changes require manual verification on a desktop.
