# Waybar Integration

## Setup

The primary install flow uses the hosted installer:

```bash
curl -fsSL https://raw.githubusercontent.com/othavi0/agent-bar/master/install.sh | bash
```

It installs the binary and optionally runs `agent-bar setup`. `agent-bar setup`
performs one managed install pass:

1. copy provider icons to `~/.config/waybar/agent-bar/icons`
2. copy the terminal helper to `~/.config/waybar/scripts`
3. create `~/.local/bin/agent-bar`
4. write generated `modules.jsonc` and `style.css`
5. patch Waybar `config.jsonc` and `style.css`
6. reload Waybar with `SIGUSR2`

Setup is idempotent. It updates managed entries and leaves unrelated Waybar
content alone.

## Update

`agent-bar update` detects the install type:

- **Managed `~/.agent-bar` checkout** (`install.sh` path): fetches upstream,
  resets, then re-applies full `setup`.
- **Standalone binary**: downloads the GitHub Release tarball, replaces the
  binary, refreshes share data assets, and re-copies **icons + terminal helper**
  into the Waybar asset dirs (no config patch / no module rewrite).
- **AUR / system package**: defers to the package manager.
- **Dev checkout**: refuses and points you to `git pull`.

## Removal

- `agent-bar uninstall`: interactive cleanup
- `agent-bar remove`: forced cleanup without prompt

Both remove managed Waybar entries and owned files.

## Backups

Before first live Waybar mutation, integration code creates backups using the
`.agent-bar-backup` suffix. Repeated setup runs should update managed entries
without creating duplicate include/import lines.

## Manual Integration

Normal users should use `setup`. The export commands exist for tests, packagers,
or unusual manual wiring:

```bash
agent-bar assets install --waybar-dir <path> --scripts-dir <path>
agent-bar export waybar-modules --app-bin <path> --terminal-script <path>
agent-bar export waybar-css --icons-dir <path>
```

## Omarchy 4 (omarchy-shell)

Omarchy 4 replaced Waybar with omarchy-shell (Quickshell).
`agent-bar setup` detects it and installs the native plugin — see
[`omarchy-shell.md`](omarchy-shell.md).
