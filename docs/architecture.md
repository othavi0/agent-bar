# Architecture

How a Waybar poll becomes a rendered module. This is the map of the runtime;
`src/` is the source of truth and wins on any disagreement.

## Data Flow

Each Waybar poll spawns **one short-lived `agent-bar` process** that walks this
pipeline and exits. There is no long-running daemon (except `--watch`, which
keeps one process alive and emits NDJSON).

```text
Waybar  (interval 120s · exec-on-event · left/right click)
   │   one process per poll
   ▼
agent-bar --provider <id>                          src/index.ts      entry / dispatch
   │   parseArgs                                    src/cli.ts        flags → CliOptions
   ▼
getQuotaFor(id) / getAllQuotas()                   src/providers/    fan-out + timeout/retry
   │   parallel · 10s timeout · 1 retry on timeout
   ▼
Provider.getQuota()
   ├─ ClaudeProvider   (implements Provider direct) src/providers/claude.ts
   └─ Codex / Amp      (extends BaseProvider)       src/providers/base.ts
   │   isAvailable() gate → credentials check
   ▼
Quota cache  (file · 5 min TTL · cross-process)    src/cache.ts
   │   hit → cached raw      miss → fetchRaw() → provider API/CLI → cache.set()
   ▼
ProviderQuota  (normalized, provider-agnostic)     src/providers/types.ts
   ▼
Formatter
   ├─ waybar    formatForWaybar / formatProviderForWaybar   src/formatters/waybar.ts
   │               builders → segments → renderPango()       src/formatters/render-pango.ts  (XML escape)
   ├─ terminal  outputTerminal (ANSI)                        src/formatters/terminal.ts
   └─ json      toJsonOutput (schemaVersion, no Pango)       src/formatters/json.ts
   ▼
stdout :  Waybar JSON {text,tooltip,class}  |  NDJSON envelope  |  ANSI terminal view
```

`stdout` is reserved for the machine-readable payload Waybar parses; all logging
goes to `stderr` via `logger`. Breaking that contract breaks the bar.

## Entry And Dispatch — `src/index.ts`

`parseArgs` (`src/cli.ts`) turns argv into `CliOptions`. `main()` dispatches:

- **Subcommands** (`menu`, `setup`, `doctor`, `update`, `action-right`, …) are
  lazily `import()`-ed and handled, then the process exits.
- **`--watch`** hands off to `startWatch` (`src/watch.ts`) and streams NDJSON.
- Otherwise it resolves quotas and prints a single payload. The default command
  is `waybar`; `status`/`terminal` print the ANSI view; `--format json` prints
  the versioned envelope.

Two Waybar render shapes share the formatter:

- **Single-provider module** — `agent-bar --provider <id>` →
  `formatProviderForWaybar`. This is what generated Waybar modules call (one
  module per provider). A provider disabled in settings never reaches the
  formatter — `index.ts` short-circuits first and prints the hidden-module
  payload (`class: agent-bar-hidden`) so Waybar collapses it.
- **Aggregate** — `agent-bar` with no provider → `outputWaybar` /
  `formatForWaybar`, joining every enabled provider into one module.

After Waybar output, low/critical desktop notifications fire best-effort
(`src/notify.ts`) — only when `notify.enabled` is set (default on) and stdout is
piped (i.e. real Waybar polling), never on an interactive terminal run, and never
in json/terminal/watch modes.

## Provider Layer — `src/providers/`

Providers self-register: each file calls `registerProvider(...)` at module scope,
and `src/providers/index.ts` imports them for their side effect. `getAllQuotas`
runs every registered provider in parallel (`Promise.all`) behind
`fetchWithRetry` (10s timeout, one retry on timeout); a thrown provider degrades
to an `available: false` quota with an error string instead of taking down the
bar. `getQuotaFor` is the single-provider variant.

Every provider returns a normalized **`ProviderQuota`** (`src/providers/types.ts`):
`primary`/`secondary` quota windows (each `{ remaining, resetsAt, … }`), optional
`extra` (per-provider, pre-render data like Claude `weeklyModels` or Codex
`modelsDetailed`), `plan`/`account`, or an `error`. Everything downstream speaks
this shape — formatters never see provider-specific API responses.

### `BaseProvider` vs `ClaudeProvider`

`BaseProvider` (`src/providers/base.ts`) owns the `getQuota()` orchestration so
concrete providers implement only what differs:

```text
getQuota():
  base = buildBase()
  if !isAvailable()        → return { ...base, error: unavailableError() }
  raw = cache.getOrFetch(cacheKey, fetchRaw, 5min)   // cached
  return buildQuota(raw, base)                         // pure transform
  (any throw)              → return { ...base, error: toUserFacingError(err) }
```

`CodexProvider` and `AmpProvider` extend it — they supply `isAvailable`,
`fetchRaw`, `buildQuota`, and `unavailableError`, and inherit the availability
gate, cache wrapper, and error handling.

`ClaudeProvider` **implements `Provider` directly** and does not extend
`BaseProvider`. Its flow doesn't fit the template: it reads
`~/.claude/.credentials.json` for the OAuth token, distinguishes "no file" /
"invalid file" / "no token" / "token expired" as separate user-facing errors,
calls `cache.getOrFetch` inline, and parses several quota windows
(`five_hour`, `seven_day`, per-model weeklies, extra usage). Forcing it into
`BaseProvider` would mean fighting the abstraction, so it manages its own cache
call. Do not "normalize" it back into the template (see repo `CLAUDE.md`).

## The Two Caches

agent-bar has two independent caches at different layers and lifetimes. They are
not redundant — they solve different problems.

| | Quota cache | Settings cache |
| --- | --- | --- |
| File | `src/cache.ts` | `src/formatters/waybar.ts` |
| Backing | File on disk (`$XDG_CACHE_HOME/agent-bar/<key>.json`, default `~/.cache`) | In-memory module variable |
| TTL | **5 minutes** (`CONFIG.cache.ttlMs`) | **5 seconds** (`SETTINGS_CACHE_TTL_MS`) |
| Lifetime | **Cross-process** — survives between polls | **In-process** — dies when the process exits |
| Protects | Provider APIs / CLIs from being hit every poll | Repeated `settings.json` disk reads within one render |

**Quota cache (5 min, cross-process).** This is the load-bearing one. Waybar
polls every 120s but each poll is a *separate process*, so an in-memory cache
would never hit. The file-based cache means roughly one poll in three serves a
fresh API response and the rest read disk (a 5-min TTL over a 120s interval) —
the provider API is hit at most once per 5-minute window. `getOrFetch` reads the cache, and on a miss runs the fetcher
once, writing atomically (temp file + `rename`) because concurrent provider
processes can write the same key. An in-flight `Map` dedups concurrent fetches
*within* one process; failed fetches are never cached (a non-200 throws before
`set`); cache keys are validated against path traversal.

**Settings cache (5 s, in-process).** `loadSettingsCached` memoizes
`loadSettingsSync()` in a module-level variable. Because each Waybar poll is a
fresh process, this only dedups repeated settings reads *within a single render*
(the Codex tooltip view-model path reads settings); the 5s TTL is a cheap upper
bound on staleness for that window. One-shot entry points (`action-right`,
`refresh`, terminal) deliberately bypass it and call `loadSettingsSync` directly
— caching there is YAGNI. The Waybar hot path (`src/formatters/waybar.ts`) uses
the cached loader because Waybar polls tightly.

The mental model: **the quota cache is what makes polling cheap across processes;
the settings cache is a small intra-render memo.** Confusing the two leads to
wrong assumptions about when a provider API actually gets called.

## Formatter Layer — `src/formatters/`

Quotas are rendered three ways from the same `ProviderQuota`:

- **Waybar** (`waybar.ts`) → `{ text, tooltip, class }` JSON. `text` is the bar
  percentage; `tooltip` is multi-line Pango built per provider; `class` is a
  provider-scoped compound carrying a health state for CSS —
  `agent-bar-<provider> <status>` for a single module, or `agent-bar` plus
  `<provider>-<status>` tokens in aggregate (see [Waybar contract](waybar-contract.md)).
  States: `ok`/`low`/`warn`/`critical`/`disconnected`.
- **Terminal** (`terminal.ts`) → ANSI view for `status` and `action-right`.
- **JSON** (`json.ts`) → versioned, Pango-free envelope
  (`{ schemaVersion, fetchedAt, providers[] }`) for non-Waybar bars. See
  [`json-output.md`](json-output.md).

Builders (`formatters/builders/`) describe output as `Line[]` of typed
`Segment`s (text + color token). **`render-pango.ts` is the single XML-escape
boundary** — `span()` / `renderPango()` are the only places provider data gets
Pango markup, and they escape every non-`raw` segment. Builders never escape;
a `raw` segment opts out of both color-wrap *and* escape and must already be
safe. Routing untrusted provider strings around this boundary is a tooltip
injection bug, so all Pango output goes through it.

## Module Map

| File | Role |
| --- | --- |
| `src/index.ts` | Entry point; dispatch by command/flags. |
| `src/cli.ts` | argv → `CliOptions`; `--help` rendering; command suggestions. |
| `src/config.ts` | Paths (XDG), cache TTL, API endpoints, color thresholds. |
| `src/cache.ts` | File-based quota cache (5 min, cross-process, atomic writes). |
| `src/providers/index.ts` | Registration side-effects, parallel fan-out, timeout/retry. |
| `src/providers/registry.ts` | In-memory provider registry. |
| `src/providers/base.ts` | `BaseProvider` `getQuota()` orchestration. |
| `src/providers/{claude,codex,amp}.ts` | Concrete providers. Claude is direct; others extend `BaseProvider`. |
| `src/providers/types.ts` | `ProviderQuota`, `QuotaWindow`, `Provider`, `AllQuotas`. |
| `src/formatters/waybar.ts` | Waybar JSON + 5s in-process settings cache. |
| `src/formatters/render-pango.ts` | Single XML-escape boundary for Pango. |
| `src/formatters/terminal.ts` | ANSI terminal view. |
| `src/formatters/json.ts` | Versioned JSON envelope (`schemaVersion`). |
| `src/waybar-contract.ts` | Generated Waybar modules/CSS/asset install (the integration contract). |
| `src/notify.ts` | Best-effort low/critical desktop notifications. |
| `src/action-right.ts` | Right-click handler (refresh-or-login). |

## See Also

- [Commands](commands.md) — public CLI surface, flags, and `action-right`.
- [Runtime](runtime.md) — owned paths, settings, cache, credentials.
- [Waybar contract](waybar-contract.md) — generated modules, classes, click actions.
- [JSON output](json-output.md) — `--format json` / `--watch` schema.
- [New provider guide](new-provider.md) — implementing a provider on `BaseProvider`.
