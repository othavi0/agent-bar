# agent-bar-omarchy

Waybar modules for watching agent CLI usage limits.

It reads quota/usage data from local agent tools and shows the useful bits in
Waybar: remaining percentage, used percentage, reset times, provider state, and
errors when a provider is not logged in.

Supported providers:

- Claude Code
- OpenAI Codex
- GitHub Copilot
- Amp

It is a small status tool for people who run agent CLIs and want to know when a
model window is low or reset.

## Install

Requires Bun.

```bash
git clone git@github.com:othavioquiliao/agent-bar-omarchy.git ~/Projects/agent-bar-omarchy
cd ~/Projects/agent-bar-omarchy
bun run setup
```

`bun run setup` installs dependencies, creates the `agent-bar-omarchy` symlink,
writes the generated Waybar include/CSS, installs icons and the terminal helper,
then reloads Waybar.

The setup is idempotent. Running it again should update the managed files without
rewriting your full Waybar config.

## Use

```bash
agent-bar-omarchy
agent-bar-omarchy status
agent-bar-omarchy menu
```

- `agent-bar-omarchy` prints Waybar JSON. Waybar calls this.
- `agent-bar-omarchy status` prints the same provider state in the terminal.
- `agent-bar-omarchy menu` opens provider login and layout/model settings.

Filter one provider:

```bash
agent-bar-omarchy status --provider codex
agent-bar-omarchy --provider claude
```

Force a fresh fetch instead of using the cache:

```bash
agent-bar-omarchy status --refresh
```

## Commands

```bash
agent-bar-omarchy setup       # install/update Waybar integration
agent-bar-omarchy update      # git pull + bun install
agent-bar-omarchy uninstall   # interactive removal
agent-bar-omarchy remove      # non-interactive removal
agent-bar-omarchy --help
```

Lower-level export commands exist for tests/manual integration:

```bash
agent-bar-omarchy assets install
agent-bar-omarchy export waybar-modules
agent-bar-omarchy export waybar-css
```

## What It Owns

Runtime files:

- `~/.config/agent-bar-omarchy/settings.json`
- `~/.cache/agent-bar-omarchy/`
- `~/.local/bin/agent-bar-omarchy`
- `~/.config/waybar/agent-bar-omarchy/`
- `~/.config/waybar/scripts/agent-bar-omarchy-open-terminal`

Waybar files it patches:

- `~/.config/waybar/config.jsonc`
- `~/.config/waybar/style.css`

It adds managed include/import entries and generated `custom/agent-bar-omarchy-*`
modules. It should not replace unrelated Waybar config.

## Provider Credentials

The tool does not own provider credentials. It only reads or invokes the provider
CLIs/files they already use:

- Claude: `~/.claude/.credentials.json`
- Codex: `~/.codex/auth.json` and recent Codex session data
- Copilot: official Copilot CLI/config
- Amp: official `amp` CLI

## Development

```bash
bun install
bun test
bun run typecheck
bun run lint
```

Do not run live-mutating setup/remove commands as tests unless you actually want
to change your current Waybar setup.

## Docs

- [Commands](docs/commands.md)
- [Runtime paths](docs/runtime.md)
- [Waybar contract](docs/waybar-contract.md)
- [Integration behavior](docs/integration.md)
- [Troubleshooting](docs/troubleshooting.md)
