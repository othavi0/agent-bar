# qbar Docs

`qbar` provides quota runtime, assets, and a Waybar export contract. In the supported `flat-onedark` setup, the theme owns the live Waybar wiring.

## Read In This Order

1. [Commands](commands.md)
2. [Runtime](runtime.md)
3. [Waybar contract](waybar-contract.md)
4. [Integration](integration.md)
5. [Troubleshooting](troubleshooting.md)

## Model

- `qbar` owns providers, auth flow, settings, cache, icons, and the terminal helper.
- `qbar setup` is safe. It does not edit `~/.config/waybar/config.jsonc` or `style.css`.
- `qbar export waybar-modules` and `qbar export waybar-css` expose the supported Waybar contract.
- `flat-onedark` consumes that contract and owns the optional overlay on the live bar.

## Theme Cross-Links

- [flat-onedark README](/home/othavio/Work/themes/omarchy-flat-onedark-theme/README.md)
- [flat-onedark qbar integration](/home/othavio/Work/themes/omarchy-flat-onedark-theme/docs/qbar-integration.md)
- [flat-onedark build and apply](/home/othavio/Work/themes/omarchy-flat-onedark-theme/docs/build-and-apply.md)

## Historical Notes

`docs/plans/` is historical planning material. It is not the operational source of truth.
