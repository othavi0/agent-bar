# qbar Docs

`qbar` provides quota runtime, assets, and native Waybar integration.

## Read In This Order

1. [Commands](commands.md)
2. [Runtime](runtime.md)
3. [Waybar contract](waybar-contract.md)
4. [Integration](integration.md)
5. [Troubleshooting](troubleshooting.md)

## Model

- `qbar` owns providers, auth flow, settings, cache, icons, terminal helper, and Waybar wiring.
- `qbar setup` installs and wires `config.jsonc` + `style.css` in an idempotent way.
- `qbar apply-local` re-syncs the live Waybar setup from the current project checkout.
- `qbar uninstall` and `qbar remove` clean both integration and owned artifacts.

## Historical Notes

`docs/plans/` is historical planning material. It is not the operational source of truth.
