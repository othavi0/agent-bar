# Troubleshooting

## Claude login doesn’t update Waybar
- The login menu watches `~/.claude/.credentials.json`
- If it doesn’t update, run:
```bash
pkill -USR2 waybar
```

## Claude Code: “Unable to connect to API (ConnectionRefused)”
- Network connectivity is usually OK, but Claude Code can get stuck with a bad session.
- Fix:
```bash
# In Claude Code: /login and re-authenticate

# If still broken, reset local state (will log you out)
rm -rf ~/.claude
```

## Codex logout still shows
- Codex output is cached in `/tmp/codex-quota.json`
- Logout removes it, but if it persists:
```bash
rm -f /tmp/codex-quota.json
pkill -USR2 waybar
```

## Antigravity missing
- If IDE is closed, Cloud fallback must be logged in
```bash
~/.config/waybar/scripts/antigravity-waybar-usage-login
```

## Tooltip alignment off
- Your Waybar font may be proportional
- Use a monospaced font in your own CSS if needed
