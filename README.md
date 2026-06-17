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

Recommended (zero pollution, runs setup automatically):

```bash
curl -fsSL https://raw.githubusercontent.com/othavioquiliao/agent-bar/master/install.sh | bash
```

Requires `bun` and `git`. Installs to `~/.agent-bar` and runs `agent-bar setup`.

`setup` installs the Waybar modules, CSS, provider icons, terminal helper, and
`~/.local/bin/agent-bar` symlink.

To update later, run:

```bash
agent-bar update
```

### Alternative: Bun global

If you already use Bun globally and prefer that workflow:

```bash
bun add -g @noctuacore/agent-bar && agent-bar setup
```

> ⚠ Don't drop the `-g`. Without it, `bun add` writes `package.json` + `bun.lock`
> to your current directory. If that happens, run `agent-bar doctor` to clean up.

For development (live edits reflected in Waybar on the next poll), see
[CONTRIBUTING.md → Dev install](CONTRIBUTING.md#dev-install-live-edit--live-waybar).

## Commands

```bash
agent-bar             # Waybar JSON
agent-bar status      # Terminal quota view
agent-bar menu        # Login and layout TUI
agent-bar update      # Update the install (npm package or managed checkout)
agent-bar setup       # Re-apply Waybar integration
agent-bar uninstall   # Interactive removal
agent-bar remove      # Forced removal
agent-bar doctor      # Detect & clean leftovers in $HOME
```

`agent-bar update` detects the install type. For the managed `~/.agent-bar`
checkout (the install.sh path) it fetches and resets to upstream. For an
npm/Bun global install it updates the package. In a dev checkout it refuses
and tells you to use `git pull`.

## Docs

- [Docs index](docs/README.md)
- [Commands](docs/commands.md)
- [Runtime](docs/runtime.md)
- [Waybar integration](docs/integration.md)
- [Waybar contract](docs/waybar-contract.md)
- [Troubleshooting](docs/troubleshooting.md)
- [New provider guide](docs/new-provider.md)
- [JSON output (Quickshell/Eww)](docs/json-output.md)
