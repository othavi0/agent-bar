# qbar

LLM quota monitor for Waybar. Tracks Claude, Codex, and Amp usage.

## Running

```bash
bun install
bun run start          # or: ./scripts/qbar
bun run dev            # watch mode
bun test               # tests
bun run typecheck      # tsc --noEmit
```

**Do NOT** run `bun ./scripts/qbar` — that file is a bash shim and Bun will try to parse it as JavaScript. Use `./scripts/qbar` (shell) or `bun run start` instead.

## Architecture

- `src/index.ts` — CLI entry point and command dispatcher
- `src/providers/` — Claude, Codex, Amp; each implements `Provider` from `src/providers/types.ts` (id, name, cacheKey, isAvailable, getQuota)
- `src/settings.ts` — `~/.config/qbar/settings.json`, normalize-on-load, atomic write (tmp+rename)
- `src/waybar-contract.ts` — Waybar module/CSS export contract (icons, JSON modules, CSS JSON)
- `src/tui/` — Interactive menu and login flows (clack/prompts)
- `src/formatters/` — Terminal and Waybar output formatting
- `scripts/qbar` — Bash wrapper (`#!/usr/bin/env bash`) used as `bin` entry in package.json. Do not convert to TS.

## Ownership boundary

qbar owns: providers, auth flows, settings, cache, icons, and Waybar integration (`config.jsonc` + `style.css` wiring).

## Key paths

| Path | Purpose |
|------|---------|
| `~/.config/qbar/settings.json` | User settings (versioned, validated, atomic writes) |
| `~/.cache/qbar/` | Cache directory |
| `~/.local/bin/qbar` | Symlink created by `qbar setup` |
| `~/.config/waybar/qbar/icons/` | Provider icons installed by setup |

## Settings

Settings use schema versioning (`version: 1`). Fields are validated on load:
- `waybar.separators` must be one of: pill, underline, gap, pipe, dot, subtle, none (default: pipe)
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
