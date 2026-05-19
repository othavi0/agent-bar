# Runtime

## Owned Paths

| Path | Purpose |
| --- | --- |
| `~/.agent-bar` | Optional managed checkout used only by the legacy `agent-bar update` flow. |
| `~/.config/agent-bar/settings.json` | User settings. Normalized on load and written atomically. |
| `~/.cache/agent-bar/` | Provider quota cache. |
| `~/.local/bin/agent-bar` | Symlink created by setup. |
| `~/.config/waybar/agent-bar/icons/` | Installed provider icons. |
| `~/.config/waybar/agent-bar/modules.jsonc` | Generated Waybar module include. |
| `~/.config/waybar/agent-bar/style.css` | Generated Waybar stylesheet. |
| `~/.config/waybar/scripts/agent-bar-open-terminal` | Terminal helper used by Waybar click actions. |

## Patched Waybar Files

| File | Managed change |
| --- | --- |
| `~/.config/waybar/config.jsonc` | Adds the generated include and `custom/agent-bar-*` modules to `modules-right`. |
| `~/.config/waybar/style.css` | Adds one managed import for `./agent-bar/style.css`. |

The app does not rewrite full Waybar files.

## Package Install

The primary install path is the npm package via Bun:

```bash
bun add -g @noctuacore/agent-bar
agent-bar setup
```

Bun owns the global package location. `agent-bar setup` still creates
`~/.local/bin/agent-bar` as the stable command path used by generated Waybar
modules. After the initial install, `agent-bar update` updates the package and
re-applies setup.

## Settings

Settings schema version: `2`.

Defaults:

- providers: `claude`, `codex`, `copilot`, `amp`
- provider order: `claude`, `codex`, `copilot`, `amp`
- separator style: `gap`
- display mode: `remaining`
- show percentage: `true`
- Codex window policy: `both`

Normalization:

- unknown providers are dropped
- duplicates are collapsed
- enabled providers missing from `providerOrder` are appended
- invalid separator, display, or window-policy values fall back to defaults
- if the normalized form differs from the stored file, it is written back to disk

Schema version 1 settings files are migrated to version 2 automatically on
load.

## Cache

- default TTL: 5 minutes
- cache keys allow only letters, numbers, `_`, and `-`
- concurrent cache misses for the same key are deduplicated
- failed fetches do not poison the cache

## Provider Credentials

Credentials stay owned by each provider. `agent-bar` reads or invokes
them; it does not store provider tokens.

| Provider | Source |
| --- | --- |
| Claude | `~/.claude/.credentials.json` |
| Codex | `~/.codex/auth.json`, recent `~/.codex/sessions/**` rate-limit events, or `codex app-server` |
| Copilot | official Copilot CLI/config |
| Amp | official `amp` CLI |
