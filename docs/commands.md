# Commands

## Public Commands

| Command | What it does | Writes where |
| --- | --- | --- |
| `qbar` | Prints Waybar JSON for one provider or the default surface. | No writes unless cache refresh runs. |
| `qbar status` | Prints quota status in the terminal. | Cache only. |
| `qbar menu` | Opens the TUI menu. | Settings and provider auth as needed. |
| `qbar setup` | Full setup. Installs assets, symlink, Waybar config wiring, and style import. | `~/.config/waybar/*`, `~/.local/bin/qbar`, qbar paths |
| `qbar apply-local` | Re-applies local project changes to live Waybar. | `~/.config/waybar/*` qbar-managed entries |
| `qbar assets install --waybar-dir <path> --scripts-dir <path>` | Installs icons and terminal helper into caller-selected paths. | Caller-selected asset paths only. |
| `qbar export waybar-modules --qbar-bin <path> --terminal-script <path>` | Prints the JSON module contract. | No writes. |
| `qbar export waybar-css --icons-dir <path>` | Prints the qbar CSS contract. | No writes. |
| `qbar uninstall` | Interactive removal of qbar integration + owned files. | qbar-managed entries and qbar-owned paths |
| `qbar remove` | Forced removal without prompt. | Same targets as uninstall |
| `qbar update` | Updates the local qbar checkout. | Repo checkout and installed symlink target. |

## Common Flags

| Flag | Meaning |
| --- | --- |
| `-t`, `--terminal` | Force terminal output mode. |
| `-p`, `--provider <id>` | Limit output to `claude`, `codex`, or `amp`. |
| `-r`, `--refresh` | Force a refresh instead of relying on cache. |
| `-h`, `--help` | Print CLI help. |

## Operational Notes

- `qbar setup` and `qbar apply-local` are idempotent.
- qbar uses managed include/import entries instead of replacing your entire Waybar files.
- `qbar remove` is intended for non-interactive cleanup scripts.

## Examples

```bash
qbar
qbar status --provider codex
qbar menu
qbar setup
qbar apply-local
qbar assets install --waybar-dir ~/.config/waybar/qbar --scripts-dir ~/.config/waybar/scripts
qbar export waybar-modules --qbar-bin '$HOME/.local/bin/qbar' --terminal-script ~/.config/waybar/scripts/qbar-open-terminal
qbar export waybar-css --icons-dir ~/.config/waybar/qbar/icons
qbar uninstall
qbar remove
```
