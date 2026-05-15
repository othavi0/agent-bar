# agent-bar-omarchy

`agent-bar-omarchy` shows Claude, Codex, GitHub Copilot, and Amp quota state in Waybar.

agent-bar-omarchy is now fully theme-agnostic. It owns its own Waybar integration and no longer depends on external theme repositories.

## Quick Start

```bash
git clone <repo-url> ~/Projects/agent-bar-omarchy
cd ~/Projects/agent-bar-omarchy
bun run setup
```

`bun run setup` installs dependencies, assets, wires `~/.config/waybar/config.jsonc` + `~/.config/waybar/style.css`, and reloads Waybar.

## Commands

```bash
agent-bar-omarchy                  # Waybar JSON (default)
agent-bar-omarchy status           # Terminal quota display
agent-bar-omarchy menu             # Interactive TUI menu
agent-bar-omarchy setup            # Full install + Waybar wiring
agent-bar-omarchy update           # Self-update via git
agent-bar-omarchy uninstall        # Interactive removal
agent-bar-omarchy remove           # Forced removal (no prompt)
```

## Docs

- [Docs index](docs/README.md)
- [Commands](docs/commands.md)
- [Runtime](docs/runtime.md)
- [Waybar contract](docs/waybar-contract.md)
- [Integration](docs/integration.md)
- [Troubleshooting](docs/troubleshooting.md)

## License

MIT
