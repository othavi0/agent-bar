# New Provider Guide

A provider is one source of quota/usage data. It usually reads a local auth file,
calls a provider CLI, parses local session data, or fetches an API.

## Contract

Implement `Provider` from `src/providers/types.ts`:

| Field | Purpose |
| --- | --- |
| `id` | stable lowercase identifier, for example `claude` |
| `name` | display name |
| `cacheKey` | unique cache key using only letters, numbers, `_`, and `-` |
| `isAvailable()` | fast local availability check, no network and no expensive work |
| `getQuota()` | returns `ProviderQuota` |

`getQuota()` should normally return errors in the `error` field instead of
throwing. Keep messages stable; tests assert exact strings in several places.

## Add A Provider

1. Create `src/providers/<name>.ts`.
2. Register it at module scope with `registerProvider(new <Name>Provider())`.
3. Export and side-effect import it from `src/providers/index.ts`.
4. Add an icon under `icons/`.
5. Add the provider ID to `WAYBAR_PROVIDERS` in `src/waybar-contract.ts`.
6. Add provider styling/icon CSS in `exportWaybarCss()`.
7. Add provider color entries in `src/theme.ts` when needed.
8. Check TUI surfaces:
   - `src/tui/login.ts`
   - `src/tui/login-single.ts`
   - `src/tui/list-all.ts`
   - `src/tui/configure-layout.ts`
9. Add a dedicated builder in `src/formatters/builders/<name>.ts` and register
   it in the `terminal.ts` and `waybar.ts` dispatchers. An unregistered provider
   falls back to the `generic` builder.
10. Add tests in `tests/providers/<name>.test.ts`.

## Data Rules

- percentages are remaining values from `0` to `100`
- `primary` drives the compact Waybar module
- `secondary` is for additional windows in tooltips
- `models` is the legacy/simple per-model map
- `modelsDetailed` is the canonical multi-window model structure
- `extraUsage` is for spend, credits, or budget-like data
- `meta` is provider-specific string-only display data
- `resetsAt` is ISO 8601 or `null`

## Cache Rules

- use `cache.getOrFetch()` unless the provider needs finer control
- default TTL is `CONFIG.cache.ttlMs`
- failed fetches must not poison the cache
- cache keys cannot contain dots, spaces, slashes, or traversal characters

## Availability Rules

`isAvailable()` should be cheap:

- file existence and small JSON checks are fine
- `command -v` style binary checks are fine
- network requests are not fine
- long-running CLI calls are not fine

## Current Provider Patterns

| Provider | Pattern |
| --- | --- |
| Claude | reads Claude Code OAuth credentials and fetches Anthropic usage API |
| Codex | prefers `codex app-server`, falls back to recent session `.jsonl` rate-limit events |
| Copilot | uses the official Copilot CLI account quota endpoint |
| Amp | runs `amp usage` and parses stdout |

## Standard Not Logged In Message

Use this when a provider exists but auth is missing:

```text
Not logged in. Open `agent-bar menu` and choose Provider login.
```
