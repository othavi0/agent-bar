# Commands

## Primary Commands

| Command | Purpose | Writes |
| --- | --- | --- |
| `agent-bar` | Print Waybar JSON for enabled providers. | Cache only when providers fetch fresh data |
| `agent-bar status` | Print the quota view in a terminal. | Cache only |
| `agent-bar menu` | Open provider login, model, and layout settings. | Settings and provider auth as requested |
| `agent-bar update` | Update the legacy managed `~/.agent-bar` checkout and re-run setup. | `~/.agent-bar`, managed Waybar files |

## Install And Removal

| Command | Purpose | Writes |
| --- | --- | --- |
| `agent-bar setup` | Install assets, create symlink, patch Waybar, reload Waybar. | `~/.local/bin`, `~/.config/waybar` |
| `agent-bar uninstall` | Interactive removal of managed integration and owned files. | Removes managed files/entries |
| `agent-bar remove` | Non-interactive forced removal. | Same targets as uninstall |

## Export And Assets

These are mostly for tests, packagers, and manual integration.

| Command | Purpose |
| --- | --- |
| `agent-bar assets install --waybar-dir <path> --scripts-dir <path>` | Copy icons and terminal helper into explicit paths. |
| `agent-bar export waybar-modules --app-bin <path> --terminal-script <path>` | Print generated Waybar module JSON. |
| `agent-bar export waybar-css --icons-dir <path>` | Print generated Waybar CSS JSON. |

## Flags

| Flag | Purpose |
| --- | --- |
| `-p`, `--provider <id>` | Limit output to `claude`, `codex`, `copilot`, or `amp`. |
| `-r`, `--refresh` | Ignore cache and fetch fresh provider data. |
| `-t`, `--terminal` | Force terminal output mode. |
| `-v`, `--verbose` | Enable diagnostic logging. |
| `-h`, `--help` | Print CLI help. |

## Update Behavior

For npm installs, update the package with Bun and re-apply setup:

```bash
bun add -g @noctuacore/agent-bar
agent-bar setup
```

`agent-bar update` is intentionally destructive only for the legacy managed
checkout path:

1. It must run from the `~/.agent-bar` checkout.
2. It fetches upstream.
3. It shows incoming commits and local changes.
4. After confirmation, it runs `git reset --hard <upstream>` and `git clean -fd`.
5. It runs `bun install` when dependency files changed or `node_modules` is missing.
6. It re-runs setup without a second confirmation.

Use a separate checkout for development work.
