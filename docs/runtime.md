# Runtime

## Owned Paths

| Path | Purpose |
| --- | --- |
| `~/.agent-bar` | Managed install checkout used by README install/update flow. |
| `~/.config/agent-bar-omarchy/settings.json` | User settings. Normalized on load and written atomically. |
| `~/.cache/agent-bar-omarchy/` | Provider quota cache. |
| `~/.local/bin/agent-bar-omarchy` | Symlink created by setup. |
| `~/.config/waybar/agent-bar-omarchy/icons/` | Installed provider icons. |
| `~/.config/waybar/agent-bar-omarchy/modules.jsonc` | Generated Waybar module include. |
| `~/.config/waybar/agent-bar-omarchy/style.css` | Generated Waybar stylesheet. |
| `~/.config/waybar/scripts/agent-bar-omarchy-open-terminal` | Terminal helper used by Waybar click actions. |

## Patched Waybar Files

| File | Managed change |
| --- | --- |
| `~/.config/waybar/config.jsonc` | Adds the generated include and `custom/agent-bar-omarchy-*` modules to `modules-right`. |
| `~/.config/waybar/style.css` | Adds one managed import for `./agent-bar-omarchy/style.css`. |

The app does not rewrite full Waybar files.

## Settings

Settings schema version: `2`.

Defaults:

- providers: `claude`, `codex`, `copilot`, `amp`
- provider order: `claude`, `codex`, `copilot`, `amp`
- separator style: `gap`
- display mode: `remaining`
- Codex window policy: `both`

Normalization:

- unknown providers are dropped
- duplicates are collapsed
- enabled providers missing from `providerOrder` are appended
- invalid separator, display, or window-policy values fall back to defaults
- normalized async loads are saved back to disk when the stored file differs

## Cache

- default TTL: 5 minutes
- cache keys allow only letters, numbers, `_`, and `-`
- concurrent cache misses for the same key are deduplicated
- failed fetches do not poison the cache

## Provider Credentials

Credentials stay owned by each provider. `agent-bar-omarchy` reads or invokes
them; it does not store provider tokens.

| Provider | Source |
| --- | --- |
| Claude | `~/.claude/.credentials.json` |
| Codex | `~/.codex/auth.json`, recent `~/.codex/sessions/**` rate-limit events, or `codex app-server` |
| Copilot | official Copilot CLI/config |
| Amp | official `amp` CLI |
