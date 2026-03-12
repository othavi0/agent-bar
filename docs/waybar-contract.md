# Waybar Contract

`qbar` exposes a safe contract for external Waybar owners. It does not patch live Waybar files directly.

## Asset Install

```bash
qbar assets install --waybar-dir <path> --scripts-dir <path>
```

This copies:

- provider icons into `<waybar-dir>/icons`
- `qbar-open-terminal` into `<scripts-dir>/qbar-open-terminal`

## Module Export

```bash
qbar export waybar-modules --qbar-bin <path> --terminal-script <path>
```

This prints JSON with:

- `providers`: normalized provider ids in render order
- `modules`: a map of Waybar module definitions

Current module ids:

- `custom/qbar-claude`
- `custom/qbar-codex`
- `custom/qbar-amp`

Each module definition includes:

- `exec`
- `return-type`
- `interval`
- `tooltip`
- `on-click`
- `on-click-right`

## CSS Export

```bash
qbar export waybar-css --icons-dir <path>
```

This prints JSON with a single `css` field. The CSS:

- resolves icon URLs from the provided icon directory
- emits provider-specific selectors for `claude`, `codex`, and `amp`
- emits separator styling based on current settings

## Returned Classes

The Waybar modules can emit these classes:

- `ok`
- `low`
- `warn`
- `critical`
- `disconnected`
- `qbar-hidden`

`qbar-hidden` is intended for consumers that collapse disabled providers without removing the module shell.

## Supported Consumer

The primary supported consumer in this workspace is:

- [/home/othavio/Work/themes/omarchy-flat-onedark-theme/scripts/apply-qbar-overlay.sh](/home/othavio/Work/themes/omarchy-flat-onedark-theme/scripts/apply-qbar-overlay.sh)

Related theme docs:

- [flat-onedark qbar integration](/home/othavio/Work/themes/omarchy-flat-onedark-theme/docs/qbar-integration.md)
- [flat-onedark build and apply](/home/othavio/Work/themes/omarchy-flat-onedark-theme/docs/build-and-apply.md)
