# Integration

## Ownership Model

qbar owns both runtime and Waybar integration.

- qbar installs assets under `~/.config/waybar/qbar`
- qbar injects module references into `modules-right`
- qbar injects a managed `include` entry for module definitions
- qbar injects a managed CSS `@import` for qbar styles

Your existing Waybar layout remains intact. qbar patches only qbar-specific entries.

## Setup Flow

`qbar setup` performs:

1. install icons to `~/.config/waybar/qbar/icons`
2. install `qbar-open-terminal` to `~/.config/waybar/scripts`
3. create `~/.local/bin/qbar` symlink
4. wire `config.jsonc` and `style.css`
5. reload Waybar

## Local Re-Apply

Use `qbar apply-local` when you are inside the project and want to re-sync live Waybar files with the current checkout.

## Removal

- `qbar uninstall`: interactive cleanup.
- `qbar remove`: force cleanup without prompt.

Both commands remove qbar-managed config/style entries and qbar-owned files.

## Snippets

`snippets/` still exists as reference material, but is not required for normal setup.

Reference files:

- [`snippets/waybar-config.jsonc`](../snippets/waybar-config.jsonc)
- [`snippets/waybar-modules.jsonc`](../snippets/waybar-modules.jsonc)
- [`snippets/waybar-style.css`](../snippets/waybar-style.css)
