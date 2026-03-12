# Runtime

## Owned Paths

| Path | Purpose | Owner |
| --- | --- | --- |
| `~/.config/qbar/settings.json` | Persistent user settings. | `qbar` |
| `~/.cache/qbar/` | Active quota cache. | `qbar` |
| `~/.local/bin/qbar` | Convenience symlink to the repo script. | `qbar setup` |
| `~/.config/waybar/qbar/icons/` | Provider icons consumed by Waybar. | `qbar assets install` |
| `~/.config/waybar/scripts/qbar-open-terminal` | Helper that opens the qbar menu in a terminal. | `qbar assets install` |

## Not Owned By qbar

- `~/.config/waybar/config.jsonc`
- `~/.config/waybar/style.css`
- `~/.config/waybar/.flat-onedark-qbar-overlay.json`

Those files belong to the theme-owned overlay flow in `flat-onedark`, not to `qbar`.

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
- Older installs may still have leftover cache data from the pre-migration layout.
- New runtime writes belong under `~/.cache/qbar`.

## Related Docs

- [Commands](commands.md)
- [Waybar contract](waybar-contract.md)
- [Integration](integration.md)
