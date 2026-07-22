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

Three supported paths. The first two converge on the same
`~/.local/bin/agent-bar` symlink that generated Waybar modules invoke; the AUR
binary lives at `/usr/bin/agent-bar` (in PATH) and its generated module invokes
`agent-bar` directly.

| Path | Source | Update |
| --- | --- | --- |
| Hosted installer (primary) | `curl -fsSL .../install.sh \| bash` installs binary to `~/.local/bin/agent-bar` | `agent-bar update` (managed-git) |
| AUR `-bin` (Arch) | `yay -S agent-bar-bin` → standalone binary at `/usr/bin/agent-bar`, assets in `/usr/share/agent-bar/` | package manager (`paru -Syu`); `agent-bar update` defers to it |
| Dev checkout | Manual `git clone` anywhere + `cargo build && ./target/debug/agent-bar setup` | `git pull` (update refuses) |

For the first two, `agent-bar setup` creates `~/.local/bin/agent-bar` as the
stable command path (target depends on which install ran setup). The AUR binary
is owned by the package manager; `setup` only writes the Waybar integration and
reads assets from `/usr/share/agent-bar/`.

## Settings

Settings schema version: `3`.

Defaults:

- providers: `claude`, `codex`, `amp`, `grok`
- provider order: `claude`, `codex`, `amp`, `grok`
- separator style: `gap`
- display mode: `remaining`
- Codex window policy: `both`
- menu animations: `true`
- menu font family: `IBM Plex Mono`
- menu font size: `12`

Normalization:

- unknown providers are dropped
- duplicates are collapsed
- enabled providers missing from `providerOrder` are appended
- invalid separator, display, or window-policy values fall back to defaults
- if the normalized form differs from the stored file, it is written back to disk

Older settings files are normalized to schema version 2 on load: the version is
stamped and unknown providers are dropped (e.g. a `copilot` entry from a previous
version is removed).

### Menu Font (`menu.animations`/`menu.fontFamily`/`menu.fontSize`)

`agent-bar menu` launches inside a terminal spawned by
`scripts/agent-bar-open-terminal`, which passes `menu.fontFamily`/
`menu.fontSize` to the terminal emulator's own font flag so the TUI opens
with the configured font. This only works for emulators the script knows a
font flag for (Alacritty, kitty, foot, Ghostty); the generic `xdg-terminal-exec`
fallback path (used when none of those are found, or via `uwsm-app`) has no
such flag, so the configured font silently does not apply there — the
terminal's own default font is used instead. Also: if `$TERMINAL` is set to
something other than `alacritty`, the script skips the Alacritty-specific
font injection and respects the user's terminal choice as-is.

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
