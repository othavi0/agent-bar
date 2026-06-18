# Waybar Contract

This is the generated contract used by setup and by the export commands.

## Providers

Built-in Waybar providers:

- `claude`
- `codex`
- `amp`

Generated module IDs:

- `custom/agent-bar-claude`
- `custom/agent-bar-codex`
- `custom/agent-bar-amp`

CSS selectors use:

```text
#custom-agent-bar-<provider>
```

## Module Export

```bash
agent-bar export waybar-modules \
  --app-bin '$HOME/.local/bin/agent-bar' \
  --terminal-script ~/.config/waybar/scripts/agent-bar-open-terminal
```

The JSON contains:

- `providers`: normalized provider IDs in render order
- `modules`: Waybar module definitions keyed by module ID

Each module includes:

- `exec` — `<app-bin> --provider <provider>`
- `return-type: json`
- `interval` (120 seconds)
- `exec-on-event`
- `tooltip`
- `on-click` — left click; runs `<terminal-script> <app-bin> menu`
- `on-click-right` — right click; runs `<terminal-script> <app-bin> action-right <provider>`

### Click Actions

Both click handlers wrap the command in the terminal helper so it opens a
window. `menu` is the interactive TUI; `action-right` refreshes the provider, or
starts its login flow when it is disconnected. See
[commands.md → Internal Commands](commands.md#internal-commands-waybar-triggered)
for the `action-right` branch logic.

### Signal (On-Demand Refresh)

Set `"signal": <N>` under `waybar` in `~/.config/agent-bar/settings.json`
(`N` between 1 and 30) to add `signal: N` to every generated module. Waybar then
re-runs the module when it receives `SIGRTMIN+N`. **Off by default** (no `signal`
key in the module unless set), so there is no collision risk out of the box.

The module's `exec` is unchanged (`agent-bar --provider <provider>`), which reads
the 5-minute quota cache — so a bare signal only re-renders cached data. To force
a **fresh** fetch on demand, invalidate the cache first, then signal:

```bash
agent-bar -p claude -r && pkill -RTMIN+8 waybar
```

Wire that as a Claude Code Stop hook to refresh the bar when a task finishes
(`~/.claude/settings.json`):

```json
{
  "hooks": {
    "Stop": [
      { "hooks": [{ "type": "command", "command": "agent-bar -p claude -r && pkill -RTMIN+8 waybar" }] }
    ]
  }
}
```

## CSS Export

```bash
agent-bar export waybar-css \
  --icons-dir ~/.config/waybar/agent-bar/icons
```

The JSON contains one `css` field. Generated CSS includes:

- base module styling
- provider icon backgrounds
- provider state colors
- separator style from settings
- hidden-module styling for disabled single-provider modules

## Classes

Single-provider output starts with:

```text
agent-bar-<provider>
```

and adds one state class:

- `ok`
- `low`
- `warn`
- `critical`
- `disconnected`

Disabled single-provider modules use:

```text
agent-bar-hidden
```

Aggregate output starts with:

```text
agent-bar
```

and adds provider-scoped classes such as `claude-ok`, `codex-warn`, or
`amp-critical`.

## Output Fields (`alt` / `percentage`)

Single-provider modules (`--provider <id>`) emit, in addition to `text`,
`tooltip`, and `class`:

- `alt` — the health state (`ok` / `low` / `warn` / `critical` / `disconnected`),
  for `format-icons` keyed by state.
- `percentage` — the displayMode-aware quota value (the same number shown in
  `text`), for `{percentage}` in `format` or `format-icons` arrays. **Omitted**
  when there is no data or the provider is disconnected.

The aggregate module (no `--provider`) does **not** emit `alt`/`percentage`.

Example `format-icons` keyed by state (`alt`) — replace the emoji with your own
glyphs (e.g. Nerd Font):

```jsonc
"custom/agent-bar-claude": {
  "format": "{icon} {percentage}%",
  "format-icons": {
    "ok": "🟢",
    "low": "🟡",
    "warn": "🟠",
    "critical": "🔴",
    "disconnected": "⚫"
  }
}
```

## Asset Install

```bash
agent-bar assets install \
  --waybar-dir ~/.config/waybar/agent-bar \
  --scripts-dir ~/.config/waybar/scripts
```

This copies:

- provider icons into `<waybar-dir>/icons`
- terminal helper into `<scripts-dir>/agent-bar-open-terminal`
