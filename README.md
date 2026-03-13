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
<flat-onedark-theme-repo>/scripts/enable-qbar-safe.sh
```

For manual Waybar integration (without the flat-onedark theme), merge the snippets in `snippets/` into your `~/.config/waybar/config.jsonc` and `style.css`.

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

See the flat-onedark theme repo for:
- flat-onedark README
- flat-onedark qbar integration
- flat-onedark build and apply
- flat-onedark troubleshooting

## License

MIT
