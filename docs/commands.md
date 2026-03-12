# Commands

## Public Commands

| Command | What it does | Writes where |
| --- | --- | --- |
| `qbar` | Prints Waybar JSON for one provider or the default surface. | No writes unless cache refresh runs. |
| `qbar status` | Prints quota status in the terminal. | Cache only. |
| `qbar menu` | Opens the TUI menu. | Settings and provider auth as needed. |
| `qbar setup` | Safe setup wrapper. Installs qbar-owned assets and symlink only. | `~/.config/waybar/qbar`, `~/.config/waybar/scripts/qbar-open-terminal`, `~/.local/bin/qbar` |
| `qbar assets install --waybar-dir <path> --scripts-dir <path>` | Installs icons and terminal helper into caller-selected paths. | Caller-selected asset paths only. |
| `qbar export waybar-modules --qbar-bin <path> --terminal-script <path>` | Prints the JSON module contract consumed by theme-owned wiring. | No writes. |
| `qbar export waybar-css --icons-dir <path>` | Prints the CSS contract consumed by theme-owned wiring. | No writes. |
| `qbar uninstall` | Removes qbar-owned files without touching live Waybar config/style. | qbar-owned paths only. |
| `qbar update` | Updates the local qbar checkout. | Repo checkout and installed symlink target. |

## Common Flags

| Flag | Meaning |
| --- | --- |
| `-t`, `--terminal` | Force terminal output mode. |
| `-p`, `--provider <id>` | Limit output to `claude`, `codex`, or `amp`. |
| `-r`, `--refresh` | Force a refresh instead of relying on cache. |
| `-h`, `--help` | Print CLI help. |

## Operational Notes

- `qbar setup` is not a Waybar mutation command anymore.
- `qbar uninstall` intentionally leaves `~/.config/waybar/config.jsonc` and `style.css` alone.
- For `flat-onedark`, use the theme-owned overlay entrypoints instead of manual edits:
  - [/home/othavio/Work/themes/omarchy-flat-onedark-theme/scripts/enable-qbar-safe.sh](/home/othavio/Work/themes/omarchy-flat-onedark-theme/scripts/enable-qbar-safe.sh)
  - [/home/othavio/Work/themes/omarchy-flat-onedark-theme/scripts/disable-qbar-safe.sh](/home/othavio/Work/themes/omarchy-flat-onedark-theme/scripts/disable-qbar-safe.sh)

## Examples

```bash
qbar
qbar status --provider codex
qbar menu
qbar assets install --waybar-dir ~/.config/waybar/qbar --scripts-dir ~/.config/waybar/scripts
qbar export waybar-modules --qbar-bin '$HOME/.local/bin/qbar' --terminal-script ~/.config/waybar/scripts/qbar-open-terminal
qbar export waybar-css --icons-dir ~/.config/waybar/qbar/icons
```
