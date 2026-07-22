# Troubleshooting

## Start With The Layer

| Symptom | First check |
| --- | --- |
| Waybar shows nothing | `agent-bar status --refresh` |
| One provider is missing | `agent-bar status --provider <id> --refresh` |
| Waybar JSON/parser error | run the module command in a terminal |
| Waybar layout changed unexpectedly | inspect `~/.config/waybar/config.jsonc` managed entries |
| Style broke after manual edits | inspect GTK CSS, not browser CSS assumptions |
| `[agent-bar] Pollution detected in $HOME` warnings | run `agent-bar doctor` |

## Runtime Checks

```bash
agent-bar status --refresh
agent-bar status --provider claude --refresh
agent-bar status --provider codex --refresh
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
checkout that is not `~/.agent-bar`). Update a dev checkout with git directly:

```bash
git pull
```

For the managed `~/.agent-bar` checkout (the install.sh path), `agent-bar update`
works without extra steps. For system installs (AUR), use the package manager
(`paru -Syu agent-bar-bin`).

## `$HOME` Pollution

If leftover agent-bar artifacts are detected in `$HOME`, clean them up with:

```bash
agent-bar doctor
```

See [Commands → `doctor`](commands.md) for flags.

## Provider Auth

### Claude

Claude uses Claude Code credentials from `~/.claude/.credentials.json`.

### Codex

Codex uses `~/.codex/auth.json`, recent session rate-limit events, or the Codex
app-server protocol when available.

### Amp

Install Amp with the official installer:

```bash
curl -fsSL https://ampcode.com/install.sh | bash
```

Then run:

```bash
amp login
```

### Grok

Grok Build CLI uses OAuth in `~/.grok/auth.json`; the provider itself
makes zero network calls — it reads session `signals.json` files under
`~/.grok/sessions/**` for context-window data. Log in with:

```bash
grok login
```

An access token past its `expires_at` does **not** log you out: the Grok
CLI renews it via refresh token, and agent-bar only checks for a
non-empty `key` in `auth.json`.

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
