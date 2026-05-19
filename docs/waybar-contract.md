# Waybar Contract

This is the generated contract used by setup and by the export commands.

## Providers

Built-in Waybar providers:

- `claude`
- `codex`
- `copilot`
- `amp`

Generated module IDs:

- `custom/agent-bar-claude`
- `custom/agent-bar-codex`
- `custom/agent-bar-copilot`
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

- `exec`
- `return-type: json`
- `interval` (120 seconds)
- `exec-on-event`
- `tooltip`
- `on-click`
- `on-click-right`

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

## Asset Install

```bash
agent-bar assets install \
  --waybar-dir ~/.config/waybar/agent-bar \
  --scripts-dir ~/.config/waybar/scripts
```

This copies:

- provider icons into `<waybar-dir>/icons`
- terminal helper into `<scripts-dir>/agent-bar-open-terminal`
