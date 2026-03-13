# qbar

`qbar` shows Claude, Codex, and Amp quota state in Waybar.

qbar is now fully theme-agnostic. It owns its own Waybar integration and no longer depends on external theme repositories.

## Quick Start

```bash
bun install
./scripts/qbar setup
```

`qbar setup` now installs assets, wires `~/.config/waybar/config.jsonc` + `~/.config/waybar/style.css`, and reloads Waybar.

## Commands

```bash
qbar
qbar status
qbar menu
qbar setup
qbar apply-local
qbar assets install --waybar-dir ~/.config/waybar/qbar --scripts-dir ~/.config/waybar/scripts
qbar export waybar-modules --qbar-bin '$HOME/.local/bin/qbar' --terminal-script ~/.config/waybar/scripts/qbar-open-terminal
qbar export waybar-css --icons-dir ~/.config/waybar/qbar/icons
qbar uninstall
qbar remove
qbar update
```

## Setup Scripts

```bash
./scripts/qbar-setup
./scripts/qbar-apply-local
./scripts/qbar-uninstall
./scripts/qbar-remove
```

- `qbar-setup`: full install + live Waybar wiring.
- `qbar-apply-local`: re-apply project changes to your live Waybar.
- `qbar-uninstall`: interactive removal of integration and owned files.
- `qbar-remove`: forced removal without prompt.

## Docs

- [Docs index](docs/README.md)
- [Commands](docs/commands.md)
- [Runtime](docs/runtime.md)
- [Waybar contract](docs/waybar-contract.md)
- [Integration](docs/integration.md)
- [Troubleshooting](docs/troubleshooting.md)

## License

MIT
