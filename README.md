<h1 align="center">Agent Bar Omarchy</h1>

<p align="center">
  <img src="docs/assets/agent-bar-banner.png" alt="Conceptual Agent Bar banner">
</p>

Waybar modules for watching agent CLI usage limits: remaining quota, used quota,
reset windows, and login/error state.

Supported providers:

- Claude Code
- OpenAI Codex
- GitHub Copilot
- Amp

## Install

Requires Bun.

```bash
git clone git@github.com:othavioquiliao/agent-bar.git ~/.agent-bar
cd ~/.agent-bar
bun run setup
```

`setup` installs the Waybar modules, CSS, provider icons, terminal helper, and
`~/.local/bin/agent-bar` symlink.

## Commands

```bash
agent-bar             # Waybar JSON
agent-bar status      # Terminal quota view
agent-bar menu        # Login and layout TUI
agent-bar update      # Update managed ~/.agent-bar install
agent-bar setup       # Re-apply Waybar integration
agent-bar uninstall   # Interactive removal
agent-bar remove      # Forced removal
```

`update` is for the managed `~/.agent-bar` checkout. It discards local changes
there, resets to upstream, installs dependencies when needed, and re-runs setup.

## Docs

- [Docs index](docs/README.md)
- [Commands](docs/commands.md)
- [Runtime](docs/runtime.md)
- [Waybar integration](docs/integration.md)
- [Waybar contract](docs/waybar-contract.md)
- [Troubleshooting](docs/troubleshooting.md)
- [New provider guide](docs/new-provider.md)
