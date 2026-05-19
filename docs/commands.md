# Commands

## Primary Commands

| Command | Purpose | Writes |
| --- | --- | --- |
| `agent-bar` | Print Waybar JSON for enabled providers. | Cache only when providers fetch fresh data |
| `agent-bar status` | Print the quota view in a terminal. | Cache only |
| `agent-bar menu` | Open provider login, model, and layout settings. | Settings and provider auth as requested |
| `agent-bar update` | Update the install: npm package via Bun, or the legacy `~/.agent-bar` checkout. | Global package or `~/.agent-bar`, managed Waybar files |

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

`agent-bar update` detects the install type and updates accordingly:

- **npm/Bun install:** after confirmation, runs `bun add -g @noctuacore/agent-bar`
  and re-applies setup.
- **Legacy managed `~/.agent-bar` checkout:** must run from `~/.agent-bar`;
  fetches upstream, shows incoming commits and local changes, and after
  confirmation runs `git reset --hard <upstream>` + `git clean -fd`, installs
  dependencies when they changed or `node_modules` is missing, and re-applies
  setup.
- **Development checkout:** refuses and tells you to update with git directly.
