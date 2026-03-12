# Troubleshooting

## Decide Which Layer Owns The Problem

| Symptom | Likely owner |
| --- | --- |
| Provider auth/login failure | `qbar` |
| Cache looks stale | `qbar` |
| `qbar status` fails in a terminal | `qbar` |
| Waybar parser error after overlay apply | Theme or manual Waybar wiring |
| Waybar module missing from the live bar | Theme or manual Waybar wiring |
| Overlay state file exists but bar is stock | Theme |

## qbar Runtime Checks

```bash
qbar status --refresh
qbar --provider claude
qbar --provider codex
qbar --provider amp
```

If these fail outside Waybar, the issue is in `qbar` or provider auth, not in the theme overlay.

## Common Cases

### `qbar setup` finished but nothing appeared in Waybar

Expected. `qbar setup` no longer edits live Waybar files.

For `flat-onedark`, run:

```bash
/home/othavio/Work/themes/omarchy-flat-onedark-theme/scripts/enable-qbar-safe.sh
```

### Waybar fails after manual CSS edits

Waybar uses GTK CSS, not browser CSS. Avoid unsupported constructs in manual integration, especially web-style variables and pseudo-selectors that GTK rejects.

If you use `flat-onedark`, restore the supported path instead of debugging drifted manual edits:

```bash
/home/othavio/Work/themes/omarchy-flat-onedark-theme/scripts/disable-qbar-safe.sh
/home/othavio/Work/themes/omarchy-flat-onedark-theme/scripts/enable-qbar-safe.sh
```

### Provider order looks wrong

`qbar` normalizes `waybar.providers` and `waybar.providerOrder` in `~/.config/qbar/settings.json`. Unsupported providers are dropped and missing enabled providers are appended.

### Uninstall removed qbar but Waybar still references qbar modules

That is expected if the external consumer still owns the wiring. For `flat-onedark`, disable the overlay from the theme repo before or after uninstall:

```bash
/home/othavio/Work/themes/omarchy-flat-onedark-theme/scripts/disable-qbar-safe.sh
qbar uninstall
```

## Related Theme Docs

- [flat-onedark qbar integration](/home/othavio/Work/themes/omarchy-flat-onedark-theme/docs/qbar-integration.md)
- [flat-onedark build and apply](/home/othavio/Work/themes/omarchy-flat-onedark-theme/docs/build-and-apply.md)
- [flat-onedark troubleshooting](/home/othavio/Work/themes/omarchy-flat-onedark-theme/docs/troubleshooting.md)
