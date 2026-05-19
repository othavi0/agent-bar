# Waybar Integration

## Setup

The primary install flow is:

```bash
bun add -g @noctuacore/agent-bar
agent-bar setup
```

`agent-bar setup` performs one managed install pass:

1. copy provider icons to `~/.config/waybar/agent-bar/icons`
2. copy the terminal helper to `~/.config/waybar/scripts`
3. create `~/.local/bin/agent-bar`
4. write generated `modules.jsonc` and `style.css`
5. patch Waybar `config.jsonc` and `style.css`
6. reload Waybar with `SIGUSR2`

Setup is idempotent. It updates managed entries and leaves unrelated Waybar
content alone.

## Update

`agent-bar update` detects the install type. For an npm/Bun install it runs
`bun add -g @noctuacore/agent-bar` and re-applies setup. For the legacy
managed `~/.agent-bar` checkout it pulls upstream and re-applies setup. In a
development checkout it refuses and points you to git.

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
