# Waybar Integration

## Setup

`agent-bar setup` performs one managed install pass:

1. copy provider icons to `~/.config/waybar/agent-bar/icons`
2. copy the terminal helper to `~/.config/waybar/scripts`
3. create `~/.local/bin/agent-bar`
4. write generated `modules.jsonc` and `style.css`
5. patch Waybar `config.jsonc` and `style.css`
6. reload Waybar with `SIGUSR2`

Setup is idempotent. It updates managed entries and leaves unrelated Waybar
content alone.

During the rename from `agent-bar-omarchy`, setup also replaces old generated
Waybar modules/imports with the new `agent-bar` namespace and keeps a legacy
`agent-bar-omarchy` CLI alias for existing shell usage.

## Update

`agent-bar update` is the managed-install updater for `~/.agent-bar`.

It discards local changes in that checkout after confirmation, resets to
upstream, installs dependencies when needed, and then runs setup without a second
prompt.

The command aborts outside `~/.agent-bar` so development checkouts are not reset
by accident.

## Removal

- `agent-bar uninstall`: interactive cleanup
- `agent-bar remove`: forced cleanup without prompt

Both remove managed Waybar entries and owned files.

## Backups

Before first live Waybar mutation, integration code creates backups using the
project backup suffix. Repeated setup runs should update managed entries without
creating duplicate include/import lines.

## Manual Integration

Normal users should use `setup`. The export commands exist for tests, packagers,
or unusual manual wiring:

```bash
agent-bar assets install --waybar-dir <path> --scripts-dir <path>
agent-bar export waybar-modules --app-bin <path> --terminal-script <path>
agent-bar export waybar-css --icons-dir <path>
```
