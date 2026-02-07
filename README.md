# qbar

LLM quota monitor for Waybar. Displays remaining usage for Claude, Codex, and Antigravity.

> **Note:** Designed for Omarchy (Arch + yay). May work on other Waybar setups.

## Features

- Waybar integration with tooltips and status colors
- Interactive TUI menu
- Smart right-click: refresh when connected, login when disconnected
- File-based cache (2 min TTL)
- Catppuccin Mocha color scheme

## Requirements

- [Bun](https://bun.sh)
- [Waybar](https://github.com/Alexays/Waybar)
- [yay](https://github.com/Jguer/yay) (for auto-installing provider CLIs)

## Installation

```bash
git clone https://github.com/othavioquiliao/qbar.git
cd qbar
bun install
ln -sf "$(pwd)/scripts/qbar" ~/.local/bin/qbar
```

## Provider Setup

qbar auto-installs CLIs via `yay` when you run the login flow. Just use `qbar menu` → Provider login.

| Provider | Package | Credentials |
|----------|---------|-------------|
| Claude | `aur/claude-code` | `~/.claude/.credentials.json` |
| Codex | `aur/openai-codex-bin` | `~/.codex/auth.json` |
| Antigravity | `bun -g antigravity-usage` | `~/.config/antigravity-usage/accounts/*/tokens.json` |

## Waybar Configuration

### 1. Copy assets

```bash
mkdir -p ~/.config/waybar/qbar
cp -r ./icons ~/.config/waybar/qbar/
cp ./scripts/qbar-open-terminal ~/.config/waybar/scripts/
chmod +x ~/.config/waybar/scripts/qbar-open-terminal
```

### 2. Add modules to config

In `~/.config/waybar/config.jsonc`, add to `modules-right`:

```jsonc
"custom/qbar-claude",
"custom/qbar-codex",
"custom/qbar-antigravity"
```

Then add the module definitions:

```jsonc
"custom/qbar-claude": {
  "exec": "$HOME/.local/bin/qbar --provider claude",
  "return-type": "json",
  "interval": 120,
  "tooltip": true,
  "on-click": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar menu",
  "on-click-right": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar action-right claude"
},

"custom/qbar-codex": {
  "exec": "$HOME/.local/bin/qbar --provider codex",
  "return-type": "json",
  "interval": 120,
  "tooltip": true,
  "on-click": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar menu",
  "on-click-right": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar action-right codex"
},

"custom/qbar-antigravity": {
  "exec": "$HOME/.local/bin/qbar --provider antigravity",
  "return-type": "json",
  "interval": 120,
  "tooltip": true,
  "on-click": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar menu",
  "on-click-right": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar action-right antigravity"
}
```

### 3. Add CSS styles

Append to `~/.config/waybar/style.css`:

```css
#custom-qbar-claude,
#custom-qbar-codex,
#custom-qbar-antigravity {
  padding-left: 22px;
  padding-right: 6px;
  background-size: 16px 16px;
  background-repeat: no-repeat;
  background-position: 4px center;
}

#custom-qbar-claude { background-image: url("qbar/icons/claude-code-icon.png"); }
#custom-qbar-codex { background-image: url("qbar/icons/codex-icon.png"); }
#custom-qbar-antigravity { background-image: url("qbar/icons/antigravity-icon.png"); }

#custom-qbar-claude.ok, #custom-qbar-codex.ok, #custom-qbar-antigravity.ok { color: #a6e3a1; }
#custom-qbar-claude.low, #custom-qbar-codex.low, #custom-qbar-antigravity.low { color: #f9e2af; }
#custom-qbar-claude.warn, #custom-qbar-codex.warn, #custom-qbar-antigravity.warn { color: #fab387; }
#custom-qbar-claude.critical, #custom-qbar-codex.critical, #custom-qbar-antigravity.critical { color: #f38ba8; }
```

### 4. Reload Waybar

```bash
pkill -USR2 waybar
```

## Usage

| Command | Description |
|---------|-------------|
| `qbar` | JSON output for Waybar |
| `qbar status` | Terminal output with colors |
| `qbar menu` | Interactive TUI |
| `qbar --provider <name>` | Single provider output |

**Waybar interactions:**
- Left-click → TUI menu
- Right-click → Refresh (or login if disconnected)

## Color Thresholds

| Remaining | Color | Hex |
|-----------|-------|-----|
| ≥60% | Green | `#a6e3a1` |
| ≥30% | Yellow | `#f9e2af` |
| ≥10% | Orange | `#fab387` |
| <10% | Red | `#f38ba8` |

## License

MIT
