# qbar

`qbar` shows Claude, Codex, and Amp quota state in Waybar.

This repo owns the `qbar` runtime, settings, cache, icons, and helper script. It does not own your live `~/.config/waybar/config.jsonc` or `style.css`.

## Quick Start

```bash
bun install
./scripts/qbar setup
```

`qbar setup` is a safe wrapper. It installs qbar-owned assets and the local symlink, but it does not edit live Waybar files.

For the supported `flat-onedark` integration, enable the overlay from the theme repo after setup:

```bash
/home/othavio/Work/themes/omarchy-flat-onedark-theme/scripts/enable-qbar-safe.sh
```

Manual snippet-based wiring still exists as reference, but it is not the recommended path for `flat-onedark`.

## Commands

```bash
qbar
qbar status
qbar menu
qbar setup
qbar assets install --waybar-dir ~/.config/waybar/qbar --scripts-dir ~/.config/waybar/scripts
qbar export waybar-modules --qbar-bin '$HOME/.local/bin/qbar' --terminal-script ~/.config/waybar/scripts/qbar-open-terminal
qbar export waybar-css --icons-dir ~/.config/waybar/qbar/icons
qbar uninstall
qbar update
```

## Docs

- [Docs index](docs/README.md)
- [Commands](docs/commands.md)
- [Runtime](docs/runtime.md)
- [Waybar contract](docs/waybar-contract.md)
- [Integration](docs/integration.md)
- [Troubleshooting](docs/troubleshooting.md)

## Related Theme Docs

- [flat-onedark README](/home/othavio/Work/themes/omarchy-flat-onedark-theme/README.md)
- [flat-onedark qbar integration](/home/othavio/Work/themes/omarchy-flat-onedark-theme/docs/qbar-integration.md)
- [flat-onedark build and apply](/home/othavio/Work/themes/omarchy-flat-onedark-theme/docs/build-and-apply.md)
- [flat-onedark troubleshooting](/home/othavio/Work/themes/omarchy-flat-onedark-theme/docs/troubleshooting.md)

## License

MIT
