# qbar - Omarchy LLM Monitor on Waybar!

Shows remaining usage for **Claude**, **Codex** and **Amp** in your bar.

## Installation

```bash
# Clone the repository
git clone https://github.com/othavioquiliao/qbar.git
cd qbar

# Install dependencies
bun install

# Configure everything automatically (copies icons, edits waybar config/css, creates symlink)
bun src/setup.ts
```

Done. The modules appear in Waybar.

## Usage

| Action | Description |
|--------|-------------|
| **Hover** | Shows tooltip with quota details |
| **Left click** | Opens interactive menu |
| **Right click** | Refresh (or login if disconnected) |

### Commands

```bash
qbar              # JSON output for Waybar
qbar status       # Show quotas in terminal
qbar menu         # Interactive menu
qbar setup        # (Re)configure Waybar automatically
qbar update       # Update qbar to latest version
qbar uninstall    # Remove qbar from system
```

### Options

| Flag | Description |
|------|-------------|
| `-t, --terminal` | Terminal output (ANSI colors) |
| `-p, --provider` | Filter: `claude`, `codex`, or `amp` |
| `-r, --refresh` | Force cache refresh |
| `-h, --help` | Show help |

### Examples

```bash
qbar                    # JSON output for Waybar
qbar menu               # Open interactive menu
qbar status             # Colored quota display
qbar -t -p claude       # Claude only (terminal)
qbar -r                 # Force refresh all providers
```

## Provider Login

Use `qbar menu` → **Provider login**. qbar installs CLIs automatically via `yay`:

| Provider | Description |
|----------|-------------|
| Claude | Uses your Claude.ai account (claude-code CLI) |
| Codex | Uses your OpenAI Codex account (codex CLI) |
| Amp | Uses your Amp account (amp CLI) |

## Colors

| Remaining | Color |
|-----------|-------|
| ≥60% | 🟢 Green |
| ≥30% | 🟡 Yellow |
| ≥10% | 🟠 Orange |
| <10% | 🔴 Red |

## Troubleshooting

**Waybar doesn't start after setup?**
```bash
# Restore backup (created automatically)
ls ~/.config/waybar/*.qbar-backup-*
cp ~/.config/waybar/config.jsonc.qbar-backup-XXXXX ~/.config/waybar/config.jsonc
```

**Provider shows disconnected icon (󱘖)?**
- Right-click the module to start login

**Refresh doesn't update value?**
- Cache lasts 2 minutes. Right-click forces immediate refresh.

## Architecture

```
~/.config/waybar/
├── config.jsonc              # qbar-claude, qbar-codex, qbar-amp modules
├── style.css                 # Module styles and colors
├── qbar/icons/               # Provider PNG icons
└── scripts/
    └── qbar-open-terminal    # Helper for floating terminal

~/.config/qbar/
└── settings.json             # User preferences

~/.config/waybar/qbar/cache/
└── *.json                    # Quota cache (2min TTL)
```

## License

MIT
