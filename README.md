# qbar

qbar is a tiny quota/usage monitor for **Waybar**.

**Status:** Omarchy-first (Arch + yay). It will probably work on other Waybar setups, but I’m optimizing for Omarchy.

It shows your remaining usage for:
- **Claude** (Anthropic)
- **Codex** (OpenAI Codex CLI)
- **Antigravity** (Google Antigravity / Codeium-backed quotas via `antigravity-usage`)

I built it to feel good in the bar and in the terminal: clean output, fast cache, Catppuccin Mocha colors.

---

## What you get

- **Waybar JSON output** (with tooltips)
- **Right-click refresh** (opens a small terminal with a spinner)
- **A TUI menu** to view everything + run logins
- **File cache** (default 5 min) so hover doesn’t slam APIs

---

## Requirements (with commands)

### 1) Bun

```bash
curl -fsSL https://bun.sh/install | bash
# restart your shell (or source ~/.bashrc / ~/.zshrc)

bun --version
```

### 2) Waybar

On Arch (Omarchy):
```bash
sudo pacman -S waybar
```

### 3) Provider CLIs

#### Claude
Install + login:
```bash
# Install Claude CLI (pick one)
# If you already have it, skip.

claude login
```

qbar reads credentials from:
- `~/.claude/.credentials.json`

#### Codex
Install + login:
```bash
# If you already have Codex CLI, skip.

codex auth login
```

qbar reads:
- auth: `~/.codex/auth.json`
- sessions: `~/.codex/sessions/` (it parses rate limits from your most recent session)

#### Antigravity
qbar uses the **`antigravity-usage`** CLI (it stores tokens locally, qbar just reads them).

Install + login:
```bash
bun add -g antigravity-usage

antigravity-usage login
```

qbar reads tokens from:
- `~/.config/antigravity-usage/accounts/*/tokens.json`

---

## Install qbar

```bash
git clone https://github.com/othavioquiliao/qbar.git
cd qbar

bun install

# put the command on your PATH
ln -sf "$(pwd)/scripts/qbar" ~/.local/bin/qbar

qbar --help || true
```

---

## Waybar setup (recommended: 3 modules with icons)

This is the setup you’re using now: separate modules per provider so each can have its own PNG icon + tooltip.

### 1) Copy icons

```bash
mkdir -p ~/.config/waybar/qbar
cp -r ./icons ~/.config/waybar/qbar/
```

### 2) Add the terminal helper (recommended)

Omarchy has its own helpers, but to make this repo self-contained we ship one.

```bash
mkdir -p ~/.config/waybar/scripts
cp ./scripts/qbar-open-terminal ~/.config/waybar/scripts/
chmod +x ~/.config/waybar/scripts/qbar-open-terminal
```

### 3) Add modules to Waybar config

Open `~/.config/waybar/config.jsonc` and:

1) Add to `modules-right`:
```jsonc
"custom/qbar-claude",
"custom/qbar-codex",
"custom/qbar-antigravity"
```

2) Add module definitions (copy/paste):

```jsonc
"custom/qbar-claude": {
  "exec": "$HOME/.local/bin/qbar --provider claude",
  "return-type": "json",
  "interval": 60,
  "tooltip": true,
  "on-click": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar menu",
  "on-click-right": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar action-right claude"
},

"custom/qbar-codex": {
  "exec": "$HOME/.local/bin/qbar --provider codex",
  "return-type": "json",
  "interval": 60,
  "tooltip": true,
  "on-click": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar menu",
  "on-click-right": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar action-right codex"
},

"custom/qbar-antigravity": {
  "exec": "$HOME/.local/bin/qbar --provider antigravity",
  "return-type": "json",
  "interval": 60,
  "tooltip": true,
  "on-click": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar menu",
  "on-click-right": "$HOME/.config/waybar/scripts/qbar-open-terminal $HOME/.local/bin/qbar action-right antigravity"
}
```

### 4) Add CSS

Append to `~/.config/waybar/style.css`:

```css
/* qbar icons (expects ~/.config/waybar/qbar/icons/*) */
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

/* status colors */
#custom-qbar-claude.ok, #custom-qbar-codex.ok, #custom-qbar-antigravity.ok { color: #a6e3a1; }
#custom-qbar-claude.low, #custom-qbar-codex.low, #custom-qbar-antigravity.low { color: #f9e2af; }
#custom-qbar-claude.warn, #custom-qbar-codex.warn, #custom-qbar-antigravity.warn { color: #fab387; }
#custom-qbar-claude.critical, #custom-qbar-codex.critical, #custom-qbar-antigravity.critical { color: #f38ba8; }
```

Reload Waybar:
```bash
pkill -USR2 waybar
```

---

## Usage

### Terminal
```bash
qbar status
qbar status --provider claude
```

### TUI
```bash
qbar menu
```

### Refresh / Login (right-click)
Right-click a provider module in the bar:
- If it’s connected: refresh
- If it’s disconnected: start the provider login flow

---

## Notes / gotchas (the honest version)

- **Antigravity refresh**: quotas are cached per account email. `qbar refresh antigravity` now deletes *all* `antigravity-quota-*.json` cache entries so it actually refreshes.
- **Waybar CSS is fragile**: one invalid CSS property can stop Waybar from starting. If Waybar dies after a change, check the logs first.

---

## License

MIT
