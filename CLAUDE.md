# agent-bar-omarchy

LLM quota monitor for Waybar. Tracks Claude, Codex, and Amp usage.

## Running

```bash
bun install
bun run start          # or: ./scripts/agent-bar-omarchy
bun run dev            # watch mode
bun test               # tests
bun run typecheck      # tsc --noEmit
```

**Do NOT** run `bun ./scripts/agent-bar-omarchy` — that file is a bash shim and Bun will try to parse it as JavaScript. Use `./scripts/agent-bar-omarchy` (shell) or `bun run start` instead.

## Architecture

- `src/index.ts` — CLI entry point and command dispatcher
- `src/providers/` — Claude, Codex, Amp; each implements `Provider` from `src/providers/types.ts` (id, name, cacheKey, isAvailable, getQuota)
- `src/settings.ts` — `~/.config/agent-bar-omarchy/settings.json`, normalize-on-load, atomic write (tmp+rename)
- `src/waybar-contract.ts` — Waybar module/CSS export contract (icons, JSON modules, CSS JSON)
- `src/tui/` — Interactive menu and login flows (clack/prompts)
- `src/formatters/` — Terminal and Waybar output formatting
- `scripts/agent-bar-omarchy` — Bash wrapper (`#!/usr/bin/env bash`) used as `bin` entry in package.json. Do not convert to TS.

## Ownership boundary

agent-bar-omarchy owns: providers, auth flows, settings, cache, icons, and Waybar integration (`config.jsonc` + `style.css` wiring).

## Key paths

| Path                                        | Purpose                                             |
| ------------------------------------------- | --------------------------------------------------- |
| `~/.config/agent-bar-omarchy/settings.json` | User settings (versioned, validated, atomic writes) |
| `~/.cache/agent-bar-omarchy/`               | Cache directory                                     |
| `~/.local/bin/agent-bar-omarchy`            | Symlink created by `agent-bar-omarchy setup`        |
| `~/.config/waybar/agent-bar-omarchy/icons/` | Provider icons installed by setup                   |

## Settings

Settings use schema versioning (`version: 1`). Fields are validated on load:

- `waybar.separators` must be one of: pill, gap, bare, glass, shadow, none (default: gap)
- `windowPolicy` values must be: both, five_hour, seven_day (default: both)
- Invalid values silently fall back to defaults

## Providers

Each provider declares a `cacheKey` used for cache invalidation:

- Claude: `claude-usage`
- Codex: `codex-quota`
- Amp: `amp-quota`

## Runtime

- Bun is the only supported runtime
- Cache TTL: 5 minutes (configurable in `src/config.ts`)
- Tests use `bun:test` runner with coverage via `bunfig.toml`
