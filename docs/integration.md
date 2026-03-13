# Integration

## Supported Model

`qbar` is runtime-owned. The Waybar wiring can be owned by another system.

In the supported `flat-onedark` environment:

- `qbar` installs assets and exposes exports
- the theme owns the live Waybar files
- the theme persists overlay state in `~/.config/waybar/.flat-onedark-qbar-overlay.json`
- the theme re-applies the overlay after repair and theme apply flows

Use the following scripts from the flat-onedark theme repo:

- `scripts/enable-qbar-safe.sh`
- `scripts/disable-qbar-safe.sh`
- `scripts/apply-theme.sh`

## What `qbar setup` Does

`qbar setup` is a safe wrapper. It only:

1. installs icons under `~/.config/waybar/qbar/icons`
2. installs `qbar-open-terminal` under `~/.config/waybar/scripts`
3. creates `~/.local/bin/qbar`

It does not:

- edit `~/.config/waybar/config.jsonc`
- edit `~/.config/waybar/style.css`
- enable a theme overlay by itself

## Manual Integration

Manual snippet-based wiring still exists for non-theme consumers, but it is a reference path. If you use it:

- you own all edits to live Waybar files
- you own placement under `modules-right`
- you own CSS compatibility with Waybar/GTK

The reference snippets live in:

- [`snippets/waybar-config.jsonc`](../snippets/waybar-config.jsonc)
- [`snippets/waybar-modules.jsonc`](../snippets/waybar-modules.jsonc)
- [`snippets/waybar-style.css`](../snippets/waybar-style.css)

## Theme Cross-Links

See the flat-onedark theme repo for:
- flat-onedark README
- flat-onedark qbar integration docs
- flat-onedark troubleshooting
