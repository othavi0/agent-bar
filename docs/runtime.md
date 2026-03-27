# Runtime

## Owned Paths

| Path | Purpose | Owner |
| --- | --- | --- |
| `~/.config/agent-bar-omarchy/settings.json` | Persistent user settings. | `agent-bar-omarchy` |
| `~/.cache/agent-bar-omarchy/` | Active quota cache. | `agent-bar-omarchy` |
| `~/.local/bin/agent-bar-omarchy` | Convenience symlink to the repo script. | `agent-bar-omarchy setup` |
| `~/.config/waybar/agent-bar-omarchy/icons/` | Provider icons consumed by Waybar. | `agent-bar-omarchy assets install` |
| `~/.config/waybar/scripts/agent-bar-omarchy-open-terminal` | Helper that opens the agent-bar-omarchy menu in a terminal. | `agent-bar-omarchy assets install` |
| `~/.config/waybar/agent-bar-omarchy/modules.jsonc` | Generated Waybar module include file. | `agent-bar-omarchy setup` / `agent-bar-omarchy apply-local` |
| `~/.config/waybar/agent-bar-omarchy/style.css` | Generated agent-bar-omarchy Waybar stylesheet. | `agent-bar-omarchy setup` / `agent-bar-omarchy apply-local` |

## Managed Entries In Live Waybar Files

- `config.jsonc`: agent-bar-omarchy appends `custom/agent-bar-omarchy-*` modules to `modules-right` and ensures an `include` entry.
- `style.css`: agent-bar-omarchy ensures one import line: `@import url("./agent-bar-omarchy/style.css");`.

agent-bar-omarchy does not replace the full file contents.

## Settings Normalization

`agent-bar-omarchy` treats `waybar.providers` and `waybar.providerOrder` as one normalized selection:

- unknown providers are discarded
- duplicate providers are collapsed
- enabled providers missing from `providerOrder` are appended
- normalized settings are written back to `~/.config/agent-bar-omarchy/settings.json`

The supported provider set is:

- `claude`
- `codex`
- `amp`

## Cache Behavior

- Primary cache path: `~/.cache/agent-bar-omarchy`
- Default TTL: 5 minutes
- Legacy cache under `~/.config/waybar/agent-bar-omarchy/cache` is cleaned on uninstall/remove.

## Related Docs

- [Commands](commands.md)
- [Waybar contract](waybar-contract.md)
- [Integration](integration.md)
