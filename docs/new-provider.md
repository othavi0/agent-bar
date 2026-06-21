# New Provider Guide

A provider is one source of quota/usage data. It usually reads a local auth file,
calls a provider CLI, parses local session data, or fetches an API.

## Contract

Implement the `Provider` trait from `src/providers/types.rs`:

| Field | Purpose |
| --- | --- |
| `id` | stable lowercase identifier, for example `claude` |
| `name` | display name |
| `cache_key` | unique cache key using only letters, numbers, `_`, and `-` |
| `is_available()` | fast local availability check, no network and no expensive work |
| `get_quota()` | returns `ProviderQuota` |

`get_quota()` should normally return errors in the `error` field instead of
panicking. Keep messages stable; tests assert exact strings in several places.

## Add A Provider

1. Create `src/providers/<name>.rs`.
2. Register it in `src/providers/mod.rs`.
3. Add an icon under `icons/`.
4. Add the provider ID to `WAYBAR_PROVIDERS` in `src/waybar_contract.rs`.
5. Add provider styling/icon CSS in `export_waybar_css()`.
6. Add provider color entries in `src/theme.rs` when needed.
7. Check TUI surfaces:
   - `src/tui/login.rs`
   - `src/tui/list_all.rs`
   - `src/tui/configure_layout.rs`
8. Add a dedicated builder in `src/formatters/builders/<name>.rs` and register
   it in the `terminal.rs` and `waybar.rs` dispatchers. An unregistered provider
   falls back to the `generic` builder.
9. Add tests in `tests/` covering the new provider.

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

- use `cache.get_or_fetch()` unless the provider needs finer control
- default TTL is `CONFIG.cache.ttl_ms`
- failed fetches must not poison the cache
- cache keys cannot contain dots, spaces, slashes, or traversal characters

## Availability Rules

`is_available()` should be cheap:

- file existence and small JSON checks are fine
- `command -v` style binary checks are fine
- network requests are not fine
- long-running CLI calls are not fine

## Current Provider Patterns

| Provider | Pattern |
| --- | --- |
| Claude | reads Claude Code OAuth credentials and fetches Anthropic usage API |
| Codex | prefers `codex app-server`, falls back to recent session `.jsonl` rate-limit events |
| Amp | runs `amp usage` and parses stdout |

## Standard Not Logged In Message

Use this when a provider exists but auth is missing:

```text
Not logged in. Open `agent-bar menu` and choose Provider login.
```
