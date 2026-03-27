# Integration

## Ownership Model

agent-bar-omarchy owns both runtime and Waybar integration.

- agent-bar-omarchy installs assets under `~/.config/waybar/agent-bar-omarchy`
- agent-bar-omarchy injects module references into `modules-right`
- agent-bar-omarchy injects a managed `include` entry for module definitions
- agent-bar-omarchy injects a managed CSS `@import` for agent-bar-omarchy styles

Your existing Waybar layout remains intact. agent-bar-omarchy patches only agent-bar-omarchy-specific entries.

## Setup Flow

`agent-bar-omarchy setup` performs:

1. install icons to `~/.config/waybar/agent-bar-omarchy/icons`
2. install `agent-bar-omarchy-open-terminal` to `~/.config/waybar/scripts`
3. create `~/.local/bin/agent-bar-omarchy` symlink
4. wire `config.jsonc` and `style.css`
5. reload Waybar

## Local Re-Apply

Use `agent-bar-omarchy apply-local` when you are inside the project and want to re-sync live Waybar files with the current checkout.

## Removal

- `agent-bar-omarchy uninstall`: interactive cleanup.
- `agent-bar-omarchy remove`: force cleanup without prompt.

Both commands remove agent-bar-omarchy-managed config/style entries and agent-bar-omarchy-owned files.

## Snippets

`snippets/` still exists as reference material, but is not required for normal setup.

Reference files:

- [`snippets/waybar-config.jsonc`](../snippets/waybar-config.jsonc)
- [`snippets/waybar-modules.jsonc`](../snippets/waybar-modules.jsonc)
- [`snippets/waybar-style.css`](../snippets/waybar-style.css)
