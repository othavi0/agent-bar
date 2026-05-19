# Troubleshooting

## Start With The Layer

| Symptom | First check |
| --- | --- |
| Waybar shows nothing | `agent-bar status --refresh` |
| One provider is missing | `agent-bar status --provider <id> --refresh` |
| Waybar JSON/parser error | run the module command in a terminal |
| Waybar layout changed unexpectedly | inspect `~/.config/waybar/config.jsonc` managed entries |
| Style broke after manual edits | inspect GTK CSS, not browser CSS assumptions |

## Runtime Checks

```bash
agent-bar status --refresh
agent-bar status --provider claude --refresh
agent-bar status --provider codex --refresh
agent-bar status --provider copilot --refresh
agent-bar status --provider amp --refresh
```

If these fail outside Waybar, fix provider auth/runtime first. Waybar is not the
first suspect.

## Setup Finished But Modules Do Not Appear

Run setup again:

```bash
agent-bar setup
```

Then reload Waybar manually if needed:

```bash
pkill -SIGUSR2 waybar
```

## Update Refuses To Run

`agent-bar update` refuses when it runs from a development checkout (a git
checkout that is not `~/.agent-bar`). Update a development checkout with git
directly:

```bash
git pull
```

For npm installs and the legacy `~/.agent-bar` checkout, `agent-bar update`
works without extra steps.

## Provider Auth

### Claude

Claude uses Claude Code credentials from `~/.claude/.credentials.json`.

### Codex

Codex uses `~/.codex/auth.json`, recent session rate-limit events, or the Codex
app-server protocol when available.

### Copilot

Install and log in with the official Copilot CLI. See the Copilot CLI
documentation for the current login command.

The active Copilot CLI account determines which quota appears.

### Amp

Install Amp with the official installer:

```bash
curl -fsSL https://ampcode.com/install.sh | bash
```

Then run:

```bash
amp login
```

## Reset Managed Waybar Entries

For a normal reset:

```bash
agent-bar setup
```

For removal:

```bash
agent-bar uninstall
```

For non-interactive forced cleanup:

```bash
agent-bar remove
```
