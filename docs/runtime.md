# Runtime

## Owned Paths

| Path | Purpose | Owner |
| --- | --- | --- |
| `~/.config/qbar/settings.json` | Persistent user settings. | `qbar` |
| `~/.cache/qbar/` | Active quota cache. | `qbar` |
| `~/.local/bin/qbar` | Convenience symlink to the repo script. | `qbar setup` |
| `~/.config/waybar/qbar/icons/` | Provider icons consumed by Waybar. | `qbar assets install` |
| `~/.config/waybar/scripts/qbar-open-terminal` | Helper that opens the qbar menu in a terminal. | `qbar assets install` |
| `~/.config/waybar/qbar/modules.jsonc` | Generated Waybar module include file. | `qbar setup` / `qbar apply-local` |
| `~/.config/waybar/qbar/style.css` | Generated qbar Waybar stylesheet. | `qbar setup` / `qbar apply-local` |

## Managed Entries In Live Waybar Files

- `config.jsonc`: qbar appends `custom/qbar-*` modules to `modules-right` and ensures an `include` entry.
- `style.css`: qbar ensures one import line: `@import url("./qbar/style.css");`.

qbar does not replace the full file contents.

## Settings Normalization

`qbar` treats `waybar.providers` and `waybar.providerOrder` as one normalized selection:

- unknown providers are discarded
- duplicate providers are collapsed
- enabled providers missing from `providerOrder` are appended
- normalized settings are written back to `~/.config/qbar/settings.json`

The supported provider set is:

- `claude`
- `codex`
- `amp`

## Cache Behavior

- Primary cache path: `~/.cache/qbar`
- Default TTL: 5 minutes
- Legacy cache under `~/.config/waybar/qbar/cache` is cleaned on uninstall/remove.

## Related Docs

- [Commands](commands.md)
- [Waybar contract](waybar-contract.md)
- [Integration](integration.md)
