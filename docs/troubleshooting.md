# Troubleshooting

## Decide Which Layer Owns The Problem

| Symptom                                 | Likely owner              |
| --------------------------------------- | ------------------------- |
| Provider auth/login failure             | `agent-bar-omarchy`                    |
| Cache looks stale                       | `agent-bar-omarchy`                    |
| `agent-bar-omarchy status` fails in a terminal       | `agent-bar-omarchy`                    |
| Waybar parser error after setup/apply   | Live Waybar config/style  |
| Waybar module missing from the live bar | Live Waybar config wiring |

## agent-bar-omarchy Runtime Checks

```bash
agent-bar-omarchy status --refresh
agent-bar-omarchy --provider claude
agent-bar-omarchy --provider codex
agent-bar-omarchy --provider amp
```

If these fail outside Waybar, the issue is in `agent-bar-omarchy` or provider auth.

## Common Cases

### `agent-bar-omarchy setup` finished but nothing appeared in Waybar

Run:

```bash
agent-bar-omarchy apply-local
```

Then reload Waybar manually if needed: `pkill -SIGUSR2 waybar`.

### Waybar fails after manual CSS edits

Waybar uses GTK CSS, not browser CSS. Avoid unsupported constructs in manual integration, especially web-style variables and pseudo-selectors that GTK rejects.

To reset agent-bar-omarchy-managed style wiring:

```bash
agent-bar-omarchy apply-local
```

### Provider order looks wrong

`agent-bar-omarchy` normalizes `waybar.providers` and `waybar.providerOrder` in `~/.config/agent-bar-omarchy/settings.json`. Unsupported providers are dropped and missing enabled providers are appended.

### Amp is missing or right-click does not start Amp login

The Amp flow now expects the official installer:

```bash
curl -fsSL https://ampcode.com/install.sh | bash
```

After install, run `amp login` or right-click the Amp module again. If Waybar still looks stale, reload it manually with `pkill -SIGUSR2 waybar`.

### Uninstall removed agent-bar-omarchy but Waybar still references agent-bar-omarchy modules

Run forced cleanup:

```bash
agent-bar-omarchy remove
```
