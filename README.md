<h1 align="center">Agent Bar</h1>

<p align="center">
  <img src="docs/assets/agent-bar-banner.png" alt="Conceptual Agent Bar banner">
</p>

Waybar modules for watching agent CLI usage limits: remaining quota, used quota,
reset windows, and login/error state.

Supported providers:

- Claude Code
- OpenAI Codex
- Amp

## Install

**Hosted installer** (recommended — runs setup automatically):

```bash
curl -fsSL https://raw.githubusercontent.com/othavioquiliao/agent-bar/master/install.sh | bash
```

Installs the standalone binary to `~/.local/bin/agent-bar` and runs
`agent-bar setup`.

`setup` installs the Waybar modules, CSS, provider icons, terminal helper, and
`~/.local/bin/agent-bar` symlink.

To update later, run:

```bash
agent-bar update
```

### Alternative: AUR (Arch)

```bash
yay -S agent-bar-bin   # or: paru -S agent-bar-bin
agent-bar setup
```

Update with your package manager (`paru -Syu`), not `agent-bar update`.

### Alternative: cargo-binstall

```bash
cargo binstall agent-bar
agent-bar setup
```

For development (building from source), see
[CONTRIBUTING.md](CONTRIBUTING.md).

## Commands

```bash
agent-bar               # Waybar JSON
agent-bar status        # Terminal quota view
agent-bar menu          # Login and layout TUI
agent-bar update        # Update the install (managed checkout or system package)
agent-bar setup         # Re-apply Waybar integration
agent-bar uninstall     # Interactive removal
agent-bar remove        # Forced removal
agent-bar doctor        # Detect & clean leftovers in $HOME
agent-bar --version     # Print version
```

### Use with other bars (Quickshell, Eww, Ironbar)

Waybar is the default, but any bar can consume the raw, versioned JSON contract:

```bash
agent-bar --format json   # one-shot structured JSON (all providers, no Pango)
agent-bar --watch         # stream NDJSON: one JSON object per line (default 60s)
```

See [JSON output](docs/json-output.md) for the schema and a Quickshell example.

`agent-bar update` detects the install type. For the managed `~/.agent-bar`
checkout (the install.sh path) it fetches and resets to upstream. For a system
package (AUR), it defers to the package manager. In a dev checkout it refuses
and tells you to use `git pull`.

## Docs

- [Docs index](docs/README.md)
- [Architecture](docs/architecture.md)
- [Commands](docs/commands.md)
- [Runtime](docs/runtime.md)
- [Waybar integration](docs/integration.md)
- [Waybar contract](docs/waybar-contract.md)
- [Troubleshooting](docs/troubleshooting.md)
- [New provider guide](docs/new-provider.md)
- [JSON output (Quickshell/Eww)](docs/json-output.md)
