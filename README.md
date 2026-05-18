<h1 align="center">Agent Bar</h1>

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
bun add -g @noctuacore/agent-bar
agent-bar setup
```

`setup` installs the Waybar modules, CSS, provider icons, terminal helper, and
`~/.local/bin/agent-bar` symlink.

To update the npm package, rerun the global install and re-apply setup:

```bash
bun add -g @noctuacore/agent-bar
agent-bar setup
```

For development, use a normal checkout:

```bash
git clone git@github.com:othavioquiliao/agent-bar.git
cd agent-bar
bun install
bun run start status
```

## Commands

```bash
agent-bar             # Waybar JSON
agent-bar status      # Terminal quota view
agent-bar menu        # Login and layout TUI
agent-bar update      # Update managed ~/.agent-bar checkout only
agent-bar setup       # Re-apply Waybar integration
agent-bar uninstall   # Interactive removal
agent-bar remove      # Forced removal
```

`update` is only for the legacy managed `~/.agent-bar` checkout flow. It is not
the npm package updater. For npm installs, use `bun add -g @noctuacore/agent-bar`
and then `agent-bar setup`.

## Docs

- [Docs index](docs/README.md)
- [Commands](docs/commands.md)
- [Runtime](docs/runtime.md)
- [Waybar integration](docs/integration.md)
- [Waybar contract](docs/waybar-contract.md)
- [Troubleshooting](docs/troubleshooting.md)
- [New provider guide](docs/new-provider.md)
