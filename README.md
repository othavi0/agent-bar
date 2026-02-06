# llm-usage-waybar

Waybar module showing LLM provider quota usage for Claude, Codex, and Antigravity.

![Screenshot](docs/screenshot.png)

## Features

- **Multi-provider support**: Claude (Anthropic), Codex (OpenAI), Antigravity (Codeium LSP)
- **Two output modes**: Waybar JSON (Pango markup) or Terminal (ANSI colors)
- **Smart caching**: Reduces API calls with configurable TTL
- **Color-coded**: Catppuccin Mocha palette with threshold-based colors
- **Lightweight**: Written in TypeScript, runs on Bun

## Requirements

- [Bun](https://bun.sh) runtime
- Provider credentials (see Setup)

## Installation

```bash
# Clone the repo
git clone https://github.com/quiliao/llm-usage-waybar.git
cd llm-usage-waybar

# Install dependencies
bun install

# Create symlink (optional, for global access)
ln -s $(pwd)/scripts/llm-usage ~/.local/bin/llm-usage
```

## Setup

### Claude

Login with the Claude CLI:
```bash
claude login
```
Credentials are read from `~/.claude/.credentials.json`.

### Codex

Login with the Codex CLI:
```bash
codex auth login
```
Quota data is parsed from session files in `~/.codex/sessions/`.

### Antigravity

Requires the Codeium Language Server running (typically via VS Code/Cursor/Windsurf extension).

## Usage

```bash
# Waybar JSON output (default)
llm-usage

# Terminal output with colors
llm-usage --terminal
llm-usage -t

# Single provider
llm-usage -t -p claude
llm-usage -t -p codex

# Force cache refresh
llm-usage --refresh
llm-usage -r

# Verbose logging (to stderr)
llm-usage -v
```

## Waybar Configuration

Add to `~/.config/waybar/config`:

```jsonc
"custom/llm-usage": {
  "exec": "~/.local/bin/llm-usage",
  "return-type": "json",
  "interval": 60,
  "tooltip": true
}
```

Add to `~/.config/waybar/style.css`:

```css
#custom-llm-usage {
  font-family: "JetBrains Mono", monospace;
  font-size: 13px;
  padding: 0 8px;
}
```

## Color Thresholds

| Remaining | Color   | Hex       |
|-----------|---------|-----------|
| ≥60%      | Green   | `#a6e3a1` |
| ≥30%      | Yellow  | `#f9e2af` |
| ≥10%      | Orange  | `#fab387` |
| <10%      | Red     | `#f38ba8` |

## Architecture

```
src/
├── index.ts           # CLI entry point
├── cli.ts             # Argument parsing
├── config.ts          # Configuration (paths, colors, thresholds)
├── cache.ts           # File-based caching with TTL
├── logger.ts          # Structured logging
├── providers/
│   ├── types.ts       # Shared interfaces
│   ├── index.ts       # Provider registry
│   ├── claude.ts      # Anthropic API
│   ├── codex.ts       # Session file parsing
│   └── antigravity.ts # Codeium LSP
└── formatters/
    ├── waybar.ts      # Pango markup output
    └── terminal.ts    # ANSI color output
```

## License

MIT
