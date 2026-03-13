# Troubleshooting

## Decide Which Layer Owns The Problem

| Symptom | Likely owner |
| --- | --- |
| Provider auth/login failure | `qbar` |
| Cache looks stale | `qbar` |
| `qbar status` fails in a terminal | `qbar` |
| Waybar parser error after setup/apply | Live Waybar config/style |
| Waybar module missing from the live bar | Live Waybar config wiring |

## qbar Runtime Checks

```bash
qbar status --refresh
qbar --provider claude
qbar --provider codex
qbar --provider amp
```

If these fail outside Waybar, the issue is in `qbar` or provider auth.

## Common Cases

### `qbar setup` finished but nothing appeared in Waybar

Run:

```bash
qbar apply-local
```

Then reload Waybar manually if needed: `pkill -SIGUSR2 waybar`.

### Waybar fails after manual CSS edits

Waybar uses GTK CSS, not browser CSS. Avoid unsupported constructs in manual integration, especially web-style variables and pseudo-selectors that GTK rejects.

To reset qbar-managed style wiring:

```bash
qbar apply-local
```

### Provider order looks wrong

`qbar` normalizes `waybar.providers` and `waybar.providerOrder` in `~/.config/qbar/settings.json`. Unsupported providers are dropped and missing enabled providers are appended.

### Uninstall removed qbar but Waybar still references qbar modules

Run forced cleanup:

```bash
qbar remove
```
