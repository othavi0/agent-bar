# Architecture

How a Waybar poll becomes a rendered module. This is the map of the runtime;
`src/` is the source of truth and wins on any disagreement.

## Data Flow

Each Waybar poll spawns **one short-lived `agent-bar` process** that walks this
pipeline and exits. There is no long-running daemon (except `--watch`, which
keeps one process alive and emits NDJSON).

```text
Waybar  (interval configurable via settings, default 60s · exec-on-event · left/right click)
   │   one process per poll
   ▼
agent-bar --provider <id>                          src/main.rs       entry / dispatch
   │   parse_args                                  src/cli.rs        flags → CliOptions
   ▼
get_quota_for(id) / get_all_quotas()              src/providers/    fan-out + timeout/retry
   │   parallel · 10s timeout · 1 retry on timeout
   ▼
Provider::get_quota()
   ├─ ClaudeProvider   (implements Provider direct) src/providers/claude.rs
   └─ Codex / Amp      (extends BaseProvider)       src/providers/base.rs
   │   is_available() gate → credentials check
   ▼
Quota cache  (file · 5 min TTL · cross-process)    src/cache.rs
   │   hit → cached raw      miss → fetch_raw() → provider API/CLI → cache.set()
   ▼
ProviderQuota  (normalized, provider-agnostic)     src/providers/types.rs
   ▼
Formatter
   ├─ waybar    format_for_waybar / format_provider_for_waybar  src/formatters/waybar.rs
   │               builders → segments → render_pango()          src/formatters/render_pango.rs  (XML escape)
   ├─ terminal  output_terminal (ANSI)                           src/formatters/terminal.rs
   └─ json      to_json_output (schemaVersion, no Pango)         src/formatters/json.rs
   ▼
stdout :  Waybar JSON {text,tooltip,class}  |  NDJSON envelope  |  ANSI terminal view
```

`stdout` is reserved for the machine-readable payload Waybar parses; all logging
goes to `stderr` via `logger`. Breaking that contract breaks the bar.

## Entry And Dispatch — `src/main.rs`

`parse_args` (`src/cli.rs`) turns argv into `CliOptions`. `main()` dispatches:

- **Subcommands** (`menu`, `setup`, `doctor`, `update`, `action-right`, …) are
  dispatched by pattern match and handled, then the process exits.
- **`--watch`** hands off to `start_watch` (`src/watch.rs`) and streams NDJSON.
- Otherwise it resolves quotas and prints a single payload. The default command
  is `waybar`; `status`/`terminal` print the ANSI view; `--format json` prints
  the versioned envelope.

Two Waybar render shapes share the formatter:

- **Single-provider module** — `agent-bar --provider <id>` →
  `formatProviderForWaybar`. This is what generated Waybar modules call (one
  module per provider). A provider disabled in settings never reaches the
  formatter — `main.rs` / gate `is_hidden_module` short-circuits first and prints
  the hidden-module payload (`class: agent-bar-hidden`) so Waybar collapses it.
- **Aggregate** — `agent-bar` with no provider → `outputWaybar` /
  `formatForWaybar`, joining every enabled provider into one module.

After Waybar output, low/critical desktop notifications fire best-effort
(`src/notify.rs`) — only when `notify.enabled` is set (default on) and stdout is
piped (i.e. real Waybar polling), never on an interactive terminal run, and never
in json/terminal/watch modes.

## Provider Layer — `src/providers/`

Providers are registered in `src/providers/mod.rs`. `get_all_quotas` runs every
registered provider in parallel behind `fetch_with_retry` (10s timeout, one retry
on timeout); a failing provider degrades to an `available: false` quota with an
error string instead of taking down the bar. `get_quota_for` is the
single-provider variant.

Every provider returns a normalized **`ProviderQuota`** (`src/providers/types.rs`):
`primary`/`secondary` quota windows (each `{ remaining, resetsAt, … }`), optional
`extra` (per-provider, pre-render data like Claude `weeklyModels` or Codex
`modelsDetailed`), `plan`/`account`, or an `error`. Everything downstream speaks
this shape — formatters never see provider-specific API responses.

### `BaseProvider` vs `ClaudeProvider`

`BaseProvider` (`src/providers/base.rs`) owns the `get_quota()` orchestration so
concrete providers implement only what differs:

```text
get_quota():
  base = build_base()
  if !is_available()       → return ProviderQuota { error: unavailable_error(), .. }
  raw = cache.get_or_fetch(cache_key, fetch_raw, 5min)  // cached
  return build_quota(raw, base)                          // pure transform
  (any error)              → return ProviderQuota { error: to_user_facing_error(e), .. }
```

`CodexProvider` and `AmpProvider` extend it — they supply `is_available`,
`fetch_raw`, `build_quota`, and `unavailable_error`, and inherit the availability
gate, cache wrapper, and error handling.

`ClaudeProvider` **implements `Provider` directly** and does not extend
`BaseProvider`. Its flow doesn't fit the template: it reads
`~/.claude/.credentials.json` for the OAuth token, distinguishes "no file" /
"invalid file" / "no token" / "token expired" as separate user-facing errors,
calls `cache.get_or_fetch` inline, and parses several quota windows
(`five_hour`, `seven_day`, per-model weeklies, extra usage). Forcing it into
`BaseProvider` would mean fighting the abstraction, so it manages its own cache
call. Do not "normalize" it back into the template (see repo `CLAUDE.md`).

From the same credentials it also reads `expiresAt` — to short-circuit a
locally-expired access token (the API would reject it anyway, and agent-bar must
never refresh it: the single-use refresh token races Claude Code) — and
`rateLimitTier`, to surface the Max multiplier in the plan label (e.g. `Max 5x`).

## The Quota Cache

`src/cache.rs` is a file-based cache (`$XDG_CACHE_HOME/agent-bar/<key>.json`,
default `~/.cache`) with a **5-minute TTL** (`CONFIG.cache.ttl_ms`) and
**cross-process** lifetime — it survives between polls, which is what matters
here: Waybar polls at the configured interval (`waybar.interval`, default 60s;
see [Waybar contract](waybar-contract.md)) but each poll is a *separate
process*, so an in-memory cache would never hit. The file-based cache means
most polls within a 5-minute window read the cached response from disk and
one triggers a fresh fetch — the provider API is hit at most once per
5-minute window regardless of the configured poll interval. `get_or_fetch`
reads the cache, and on a miss runs the fetcher once, writing atomically
(temp file + `rename`) because concurrent provider processes can write the
same key. An in-flight dedup map deduplicates concurrent fetches *within* one
process; failed fetches are never cached (a non-200 errors before `set`);
cache keys are validated against path traversal.

There is no separate settings cache — `settings::load` reads `settings.json`
directly on every call; no in-process memoization layer exists in the Rust
codebase (an in-memory settings cache existed in the pre-rewrite TS version,
not here).

## Formatter Layer — `src/formatters/`

Quotas are rendered three ways from the same `ProviderQuota`:

- **Waybar** (`waybar.rs`) → `{ text, tooltip, class }` JSON. `text` is the bar
  percentage; `tooltip` is multi-line Pango built per provider; `class` is a
  provider-scoped compound carrying a health state for CSS —
  `agent-bar-<provider> <status>` for a single module, or `agent-bar` plus
  `<provider>-<status>` tokens in aggregate (see [Waybar contract](waybar-contract.md)).
  States: `ok`/`low`/`warn`/`critical`/`disconnected`.
- **Terminal** (`terminal.rs`) → ANSI view for `status`. `action-right` no
  longer uses this path — it resolves focus and opens the TUI instead (see
  `src/action_right.rs` below).
- **JSON** (`json.rs`) → versioned, Pango-free envelope
  (`{ schemaVersion, fetchedAt, providers[] }`) for non-Waybar bars. See
  [`json-output.md`](json-output.md).

Builders (`formatters/builders/`) describe output as `Vec<Line>` of typed
`Segment`s (text + color token). **`render_pango.rs` is the single XML-escape
boundary** — `span()` / `render_pango()` are the only places provider data gets
Pango markup, and they escape every non-`raw` segment. Builders never escape;
a `raw` segment opts out of both color-wrap *and* escape and must already be
safe. Routing untrusted provider strings around this boundary is a tooltip
injection bug, so all Pango output goes through it.

## Module Map

| File | Role |
| --- | --- |
| `src/main.rs` | Entry point; dispatch by command/flags. |
| `src/cli.rs` | argv → `CliOptions`; `--help` rendering; command suggestions. |
| `src/config.rs` | Paths (XDG), cache TTL, API endpoints, color thresholds. |
| `src/cache.rs` | File-based quota cache (5 min, cross-process, atomic writes). |
| `src/providers/mod.rs` | Registration, parallel fan-out, timeout/retry. |
| `src/providers/base.rs` | `BaseProvider` `get_quota()` orchestration. |
| `src/providers/{claude,codex,amp,grok}.rs` | Concrete providers. Claude is direct; others extend `BaseProvider`. |
| `src/providers/types.rs` | `ProviderQuota`, `QuotaWindow`, `Provider`, `AllQuotas`. |
| `src/formatters/waybar.rs` | Waybar JSON assembly ({text,tooltip,class}). |
| `src/formatters/render_pango.rs` | Single XML-escape boundary for Pango. |
| `src/formatters/terminal.rs` | ANSI terminal view. |
| `src/formatters/json.rs` | Versioned JSON envelope (`schemaVersion`). |
| `src/waybar_contract.rs` | Generated Waybar modules/CSS/asset install (the integration contract). |
| `src/notify.rs` | Best-effort low/critical desktop notifications. |
| `src/action_right.rs` | Right-click handler — resolves TUI focus (provider detail or Login) from connection state. |

## See Also

- [Commands](commands.md) — public CLI surface, flags, and `action-right`.
- [Runtime](runtime.md) — owned paths, settings, cache, credentials.
- [Waybar contract](waybar-contract.md) — generated modules, classes, click actions.
- [JSON output](json-output.md) — `--format json` / `--watch` schema.
- [New provider guide](new-provider.md) — implementing a provider on `BaseProvider`.
