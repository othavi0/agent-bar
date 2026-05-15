# agent-bar-omarchy Docs

Operational notes for the Waybar quota monitor.

## Read In This Order

1. [Commands](commands.md)
2. [Runtime](runtime.md)
3. [Waybar contract](waybar-contract.md)
4. [Integration](integration.md)
5. [Troubleshooting](troubleshooting.md)

## Model

- `agent-bar-omarchy` reads agent CLI usage/quota state and renders Waybar modules.
- Provider credentials stay owned by each provider CLI/config.
- `agent-bar-omarchy setup` installs and wires `config.jsonc` + `style.css` in an idempotent way.
- `agent-bar-omarchy uninstall` and `agent-bar-omarchy remove` clean both integration and owned artifacts.

## Historical Notes

`docs/plans/` is historical planning material. It is not the operational source of truth.
