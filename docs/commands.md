# Commands

## Public Commands

| Command | What it does | Writes where |
| --- | --- | --- |
| `agent-bar-omarchy` | Prints Waybar JSON for one provider or the default surface. | No writes unless cache refresh runs. |
| `agent-bar-omarchy status` | Prints quota status in the terminal. | Cache only. |
| `agent-bar-omarchy menu` | Opens the TUI menu. | Settings and provider auth as needed. |
| `agent-bar-omarchy setup` | Full setup. Installs assets, symlink, Waybar config wiring, and style import. | `~/.config/waybar/*`, `~/.local/bin/agent-bar-omarchy`, agent-bar-omarchy paths |
| `agent-bar-omarchy apply-local` | Re-applies local project changes to live Waybar. | `~/.config/waybar/*` agent-bar-omarchy-managed entries |
| `agent-bar-omarchy assets install --waybar-dir <path> --scripts-dir <path>` | Installs icons and terminal helper into caller-selected paths. | Caller-selected asset paths only. |
| `agent-bar-omarchy export waybar-modules --app-bin <path> --terminal-script <path>` | Prints the JSON module contract. | No writes. |
| `agent-bar-omarchy export waybar-css --icons-dir <path>` | Prints the agent-bar-omarchy CSS contract. | No writes. |
| `agent-bar-omarchy uninstall` | Interactive removal of agent-bar-omarchy integration + owned files. | agent-bar-omarchy-managed entries and agent-bar-omarchy-owned paths |
| `agent-bar-omarchy remove` | Forced removal without prompt. | Same targets as uninstall |
| `agent-bar-omarchy update` | Updates the local agent-bar-omarchy checkout. | Repo checkout and installed symlink target. |

## Common Flags

| Flag | Meaning |
| --- | --- |
| `-t`, `--terminal` | Force terminal output mode. |
| `-p`, `--provider <id>` | Limit output to `claude`, `codex`, or `amp`. |
| `-r`, `--refresh` | Force a refresh instead of relying on cache. |
| `-h`, `--help` | Print CLI help. |

## Operational Notes

- `agent-bar-omarchy setup` and `agent-bar-omarchy apply-local` are idempotent.
- agent-bar-omarchy uses managed include/import entries instead of replacing your entire Waybar files.
- `agent-bar-omarchy remove` is intended for non-interactive cleanup scripts.

## Examples

```bash
agent-bar-omarchy
agent-bar-omarchy status --provider codex
agent-bar-omarchy menu
agent-bar-omarchy setup
agent-bar-omarchy apply-local
agent-bar-omarchy assets install --waybar-dir ~/.config/waybar/agent-bar-omarchy --scripts-dir ~/.config/waybar/scripts
agent-bar-omarchy export waybar-modules --app-bin '$HOME/.local/bin/agent-bar-omarchy' --terminal-script ~/.config/waybar/scripts/agent-bar-omarchy-open-terminal
agent-bar-omarchy export waybar-css --icons-dir ~/.config/waybar/agent-bar-omarchy/icons
agent-bar-omarchy uninstall
agent-bar-omarchy remove
```
