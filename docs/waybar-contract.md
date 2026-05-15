# Waybar Contract

This is the generated contract used by setup and by the export commands.

## Providers

Built-in Waybar providers:

- `claude`
- `codex`
- `copilot`
- `amp`

Generated module IDs:

- `custom/agent-bar-omarchy-claude`
- `custom/agent-bar-omarchy-codex`
- `custom/agent-bar-omarchy-copilot`
- `custom/agent-bar-omarchy-amp`

CSS selectors use:

```text
#custom-agent-bar-omarchy-<provider>
```

## Module Export

```bash
agent-bar-omarchy export waybar-modules \
  --app-bin '$HOME/.local/bin/agent-bar-omarchy' \
  --terminal-script ~/.config/waybar/scripts/agent-bar-omarchy-open-terminal
```

The JSON contains:

- `providers`: normalized provider IDs in render order
- `modules`: Waybar module definitions keyed by module ID

Each module includes:

- `exec`
- `return-type: json`
- `interval`
- `tooltip`
- `on-click`
- `on-click-right`

## CSS Export

```bash
agent-bar-omarchy export waybar-css \
  --icons-dir ~/.config/waybar/agent-bar-omarchy/icons
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
agent-bar-omarchy-<provider>
```

and adds one state class:

- `ok`
- `low`
- `warn`
- `critical`
- `disconnected`

Disabled single-provider modules use:

```text
agent-bar-omarchy-hidden
```

Aggregate output starts with:

```text
agent-bar-omarchy
```

and adds provider-scoped classes such as `claude-ok`, `codex-warn`, or
`amp-critical`.

## Asset Install

```bash
agent-bar-omarchy assets install \
  --waybar-dir ~/.config/waybar/agent-bar-omarchy \
  --scripts-dir ~/.config/waybar/scripts
```

This copies:

- provider icons into `<waybar-dir>/icons`
- terminal helper into `<scripts-dir>/agent-bar-omarchy-open-terminal`
