# agent-bar-omarchy

`agent-bar-omarchy` shows Claude, Codex, and Amp quota state in Waybar.

agent-bar-omarchy is now fully theme-agnostic. It owns its own Waybar integration and no longer depends on external theme repositories.

## Quick Start

```bash
bun install
./scripts/agent-bar-omarchy setup
```

`agent-bar-omarchy setup` now installs assets, wires `~/.config/waybar/config.jsonc` + `~/.config/waybar/style.css`, and reloads Waybar.

## Commands

```bash
agent-bar-omarchy
agent-bar-omarchy status
agent-bar-omarchy menu
agent-bar-omarchy setup
agent-bar-omarchy apply-local
agent-bar-omarchy assets install --waybar-dir ~/.config/waybar/agent-bar-omarchy --scripts-dir ~/.config/waybar/scripts
agent-bar-omarchy export waybar-modules --app-bin '$HOME/.local/bin/agent-bar-omarchy' --terminal-script ~/.config/waybar/scripts/agent-bar-omarchy-open-terminal
agent-bar-omarchy export waybar-css --icons-dir ~/.config/waybar/agent-bar-omarchy/icons
agent-bar-omarchy uninstall
agent-bar-omarchy remove
agent-bar-omarchy update
```

## Setup Scripts

```bash
./scripts/agent-bar-omarchy-setup
./scripts/agent-bar-omarchy-apply-local
./scripts/agent-bar-omarchy-uninstall
./scripts/agent-bar-omarchy-remove
```

- `agent-bar-omarchy-setup`: full install + live Waybar wiring.
- `agent-bar-omarchy-apply-local`: re-apply project changes to your live Waybar.
- `agent-bar-omarchy-uninstall`: interactive removal of integration and owned files.
- `agent-bar-omarchy-remove`: forced removal without prompt.

## Docs

- [Docs index](docs/README.md)
- [Commands](docs/commands.md)
- [Runtime](docs/runtime.md)
- [Waybar contract](docs/waybar-contract.md)
- [Integration](docs/integration.md)
- [Troubleshooting](docs/troubleshooting.md)

## License

MIT
