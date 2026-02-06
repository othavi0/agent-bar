# qbar

LLM quota monitor for Waybar. Shows usage for Claude, Codex, and Antigravity.

## Features

- **Waybar integration**: JSON output with Pango markup
- **Terminal output**: ANSI colored quota display
- **Interactive TUI**: Configure what shows where
- **Smart caching**: Reduces API calls with configurable TTL
- **Catppuccin Mocha**: Beautiful color scheme

## Requirements

- [Bun](https://bun.sh) runtime
- Provider credentials (see Setup)

## Installation

```bash
# Clone the repo
git clone https://github.com/othavioquiliao/qbar.git
cd qbar

# Install dependencies
bun install

# Create symlink
ln -s $(pwd)/scripts/qbar ~/.local/bin/qbar
```

## Setup

### Claude

```bash
claude login
```

Credentials: `~/.claude/.credentials.json`

### Codex

```bash
codex auth login
```

Quota: parsed from `~/.codex/sessions/`

### Antigravity

Requires Codeium Language Server (VS Code/Cursor extension).

## Usage

```bash
# Waybar JSON output (default)
qbar

# Terminal output with colors
qbar status
qbar -t

# Interactive menu
qbar menu

# Single provider
qbar -t -p claude

# Force cache refresh
qbar --refresh
```

## Waybar Configuration

Add to `~/.config/waybar/config`:

```jsonc
"custom/qbar": {
  "exec": "~/.local/bin/qbar",
  "return-type": "json",
  "interval": 60,
  "tooltip": true
}
```

Add to `~/.config/waybar/style.css`:

```css
#custom-qbar {
  font-family: "JetBrains Mono", monospace;
  font-size: 13px;
  padding: 0 8px;
}
```

## Interactive Menu

Run `qbar menu` for an interactive TUI:

```
┌  qbar v3.0.0
│
◆  What would you like to do?
│  ● List all
│  ○ Configure Waybar
│  ○ Configure Tooltip
│  ○ Cancel
└
```

- **List all**: View quotas for all logged providers
- **Configure Waybar**: Select which providers show in the bar
- **Configure Tooltip**: Select what appears on hover

Settings are saved to `~/.config/qbar/settings.json`.

## Color Thresholds (Catppuccin Mocha)

| Remaining | Color   | Hex       |
|-----------|---------|-----------|
| ≥60%      | Green   | `#a6e3a1` |
| ≥30%      | Yellow  | `#f9e2af` |
| ≥10%      | Peach   | `#fab387` |
| <10%      | Red     | `#f38ba8` |

## Architecture

```
src/
├── index.ts           # Entry point
├── cli.ts             # Argument parsing
├── config.ts          # Paths, colors, thresholds
├── settings.ts        # User preferences
├── cache.ts           # File-based caching
├── logger.ts          # Structured logging
├── providers/
│   ├── types.ts
│   ├── claude.ts
│   ├── codex.ts
│   └── antigravity.ts
├── formatters/
│   ├── waybar.ts
│   └── terminal.ts
└── tui/
    ├── index.ts       # Main menu
    ├── colors.ts      # Catppuccin palette
    ├── list-all.ts
    ├── configure-waybar.ts
    └── configure-tooltip.ts
```

## License

MIT
