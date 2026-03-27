# Waybar Contract

`agent-bar-omarchy` exposes a stable contract for module JSON and CSS generation. This same contract is used by `setup`/`apply-local` to wire live Waybar files.

## Asset Install

```bash
agent-bar-omarchy assets install --waybar-dir <path> --scripts-dir <path>
```

This copies:

- provider icons into `<waybar-dir>/icons`
- `agent-bar-omarchy-open-terminal` into `<scripts-dir>/agent-bar-omarchy-open-terminal`

## Module Export

```bash
agent-bar-omarchy export waybar-modules --app-bin <path> --terminal-script <path>
```

This prints JSON with:

- `providers`: normalized provider ids in render order
- `modules`: a map of Waybar module definitions

Current module ids:

- `custom/agent-bar-omarchy-claude`
- `custom/agent-bar-omarchy-codex`
- `custom/agent-bar-omarchy-amp`

Each module definition includes:

- `exec`
- `return-type`
- `interval`
- `tooltip`
- `on-click`
- `on-click-right`

## CSS Export

```bash
agent-bar-omarchy export waybar-css --icons-dir <path>
```

This prints JSON with a single `css` field. The CSS:

- resolves icon URLs from the provided icon directory
- emits provider-specific selectors for `claude`, `codex`, and `amp`
- emits separator styling based on current settings
- includes agent-bar-omarchy base module styling and status classes

## Returned Classes

The Waybar modules can emit these classes:

- `ok`
- `low`
- `warn`
- `critical`
- `disconnected`
- `agent-bar-omarchy-hidden`

`agent-bar-omarchy-hidden` is intended for consumers that collapse disabled providers without removing the module shell.
