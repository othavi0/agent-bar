# Runtime

## Owned Paths

| Path | Purpose |
| --- | --- |
| `~/.agent-bar` | Managed checkout created by `install.sh` and updated by `agent-bar update`. |
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

## Install Paths

Three supported paths, all converge on the same `~/.local/bin/agent-bar`
symlink that generated Waybar modules invoke:

| Path | Source | Update |
| --- | --- | --- |
| Hosted installer (primary) | `curl -fsSL .../install.sh \| bash` clones into `~/.agent-bar` | `agent-bar update` (managed-git) |
| Bun global | `bun add -g @noctuacore/agent-bar` | `agent-bar update` (npm) |
| Dev checkout | Manual `git clone` anywhere + `bun run start setup` | `git pull` (update refuses) |

`agent-bar setup` always creates `~/.local/bin/agent-bar` as the stable
command path. The symlink target depends on which install ran setup.

## Settings

Settings schema version: `2`.

Defaults:

- providers: `claude`, `codex`, `amp`
- provider order: `claude`, `codex`, `amp`
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

Older settings files are normalized to schema version 2 on load: the version is
stamped and unknown providers are dropped (e.g. a `copilot` entry from a previous
version is removed).

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
| Amp | official `amp` CLI |
