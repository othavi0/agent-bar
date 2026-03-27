# agent-bar-omarchy Docs

`agent-bar-omarchy` provides quota runtime, assets, and native Waybar integration.

## Read In This Order

1. [Commands](commands.md)
2. [Runtime](runtime.md)
3. [Waybar contract](waybar-contract.md)
4. [Integration](integration.md)
5. [Troubleshooting](troubleshooting.md)

## Model

- `agent-bar-omarchy` owns providers, auth flow, settings, cache, icons, terminal helper, and Waybar wiring.
- `agent-bar-omarchy setup` installs and wires `config.jsonc` + `style.css` in an idempotent way.
- `agent-bar-omarchy apply-local` re-syncs the live Waybar setup from the current project checkout.
- `agent-bar-omarchy uninstall` and `agent-bar-omarchy remove` clean both integration and owned artifacts.

## Historical Notes

`docs/plans/` is historical planning material. It is not the operational source of truth.
