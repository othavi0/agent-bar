# Architecture

How agent-bar turns a fetch into something on screen. Omarchy 4
(omarchy-shell) is the primary target — `Widget.qml` renders the popup
straight from `--format json`. Waybar remains supported as a **legacy
tier** (works, gets fixes, no new features) via the classic Pango module
contract. `src/` is the source of truth and wins on any disagreement.

## Data Flow

Two consumers read the same provider/formatter pipeline; they diverge
only at the very end.

```text
Omarchy Widget.qml (native plugin, primary)      Waybar module (legacy tier)
   │  polls                                          │  polls (interval from settings, default 60s)
   ▼                                                  ▼
agent-bar --format json                       agent-bar --provider <id>
   │                                                  │
   └──────────────────┬───────────────────────────────┘
                       ▼
        parse_args → CliOptions                  src/cli.rs
                       ▼
        get_quota_for(id) / get_all_quotas()      src/providers/   fan-out + timeout/retry
                       │  parallel · 10s timeout · 1 retry on timeout
                       ▼
        Provider::get_quota()
           ├─ ClaudeProvider    (implements Provider direct) src/providers/claude.rs
           └─ Codex/Amp/Grok   (extend BaseProvider)        src/providers/base.rs
                       │  is_available() gate → credentials check
                       ▼
        Quota cache  (file · 5 min TTL · cross-process)    src/cache.rs
                       │  hit → cached raw   miss → fetch_raw() → provider API/CLI → cache.set()
                       ▼
        ProviderQuota  (normalized, windowKind classified) src/providers/types.rs
                       │
           ┌───────────┴────────────┐
           ▼                        ▼
    to_json_output            formatters::waybar
    (schemaVersion, no Pango)  format_for_waybar / format_provider_for_waybar
    src/formatters/json.rs     builders → segments → render_pango() (XML escape)
           │                        │
           ▼                        ▼
    stdout NDJSON/JSON envelope    stdout Waybar JSON {text,tooltip,class}
           │
           ▼
    Widget.qml applyPayload() — collapses duplicate windows
    (same windowKind/resetsAt/remaining), renders countdown from
    resetsAt/fetchedAt, hero %, per-provider panel, extra line.
```

`stdout` is reserved for the machine-readable payload the shell/Waybar
parses; all logging goes to `stderr` via `logger`. Breaking that contract
breaks both consumers.

`agent-bar menu` (the TUI) and `agent-bar status` read the exact same
`ProviderQuota` through their own formatter (`terminal.rs`) — a third,
interactive consumer of the same pipeline, not shown above for brevity.

## Entry And Dispatch — `src/main.rs`

`parse_args` (`src/cli.rs`) turns argv into `CliOptions`. `main()`
dispatches:

- **Subcommands** (`menu`, `setup`, `doctor`, `update`, `config
  show|apply`, `action-right`, …) are dispatched by pattern match and
  handled, then the process exits. `config show|apply` is implemented in
  `src/config_cmd.rs` (editable settings subset only, plus the read-only
  `menuAnimations` flag on `show`; no Waybar reload). `action-right`
  remains the Waybar right-click path; Omarchy uses the native popup
  settings mode instead (see [omarchy-shell.md](omarchy-shell.md)).
- **`--watch`** hands off to `start_watch` (`src/watch.rs`) and streams
  NDJSON.
- Otherwise it resolves quotas and prints a single payload. The default
  command is `waybar`; `status` (`-t`/`--terminal` alias) prints the ANSI
  view; `--format json` prints the versioned envelope Widget.qml
  consumes.

`setup`, `update`, and TUI Config Save all gate their Waybar-directory
writes behind `platform::detect()` (`src/platform.rs`) — a single
`Platform { omarchy: bool, waybar: bool }` that composes
`omarchy_integration::detect_omarchy_shell()` and `setup::waybar_present()`.
On an Omarchy-only desktop, none of the three paths create
`~/.config/waybar/` from scratch.

Two Waybar render shapes share the formatter (legacy tier only):

- **Single-provider module** — `agent-bar --provider <id>` →
  `formatProviderForWaybar`. This is what generated Waybar modules call
  (one module per provider). A provider disabled in settings never
  reaches the formatter — `main.rs` / gate `is_hidden_module`
  short-circuits first and prints the hidden-module payload (`class:
  agent-bar-hidden`) so Waybar collapses it.
- **Aggregate** — `agent-bar` with no provider → `outputWaybar` /
  `formatForWaybar`, joining every enabled provider into one module.

After Waybar output, low/critical desktop notifications fire best-effort
(`src/notify.rs`) — only when `notify.enabled` is set (default on) and
stdout is piped (i.e. real Waybar polling), never on an interactive
terminal run, and never in json/terminal/watch modes.

## Provider Layer — `src/providers/`

Providers are registered in `src/providers/mod.rs`. `get_all_quotas` runs
every registered provider in parallel behind `fetch_with_retry` (10s
timeout, one retry on timeout); a failing provider degrades to an
`available: false` quota with an error string instead of taking down the
bar. `get_quota_for` is the single-provider variant.

Every provider returns a normalized **`ProviderQuota`**
(`src/providers/types.rs`): `primary`/`secondary` quota windows (each
`{ remaining, resetsAt, windowKind, … }`), optional `extra` (per-provider,
pre-render data like Claude `weeklyModels`, Codex `modelsDetailed`, Amp
credits, or Grok session/turn counts), `plan`/`account`, or an `error`.
Everything downstream speaks this shape — formatters never see
provider-specific API responses.

Each provider sets `windowKind` at the source instead of downstream code
guessing from the number: Claude sets `fiveHour`/`sevenDay` directly; Amp
sets `daily`; Grok sets `context`; Codex classifies via
`classify_window` (below) with no fallback — a window outside tolerance
is `other`, not force-mapped onto `fiveHour`/`sevenDay`.

### `BaseProvider` vs `ClaudeProvider`

`BaseProvider` (`src/providers/base.rs`) owns the `get_quota()`
orchestration so concrete providers implement only what differs:

```text
get_quota():
  base = build_base()
  if !is_available()       → return ProviderQuota { error: unavailable_error(), .. }
  raw = cache.get_or_fetch(cache_key, fetch_raw, 5min)  // cached
  return build_quota(raw, base)                          // pure transform
  (any error)              → return ProviderQuota { error: to_user_facing_error(e), .. }
```

`CodexProvider`, `AmpProvider`, and `GrokProvider` extend it — they
supply `is_available`, `fetch_raw`, `build_quota`, and
`unavailable_error`, and inherit the availability gate, cache wrapper,
and error handling.

`ClaudeProvider` **implements `Provider` directly** and does not extend
`BaseProvider`. Its flow doesn't fit the template: it reads
`~/.claude/.credentials.json` for the OAuth token, distinguishes "no
file" / "invalid file" / "no token" / "token expired" as separate
user-facing errors, calls `cache.get_or_fetch` inline, and parses several
quota windows (`five_hour`, `seven_day`, per-model weeklies, extra
usage). Forcing it into `BaseProvider` would mean fighting the
abstraction, so it manages its own cache call. Do not "normalize" it back
into the template (see repo `CLAUDE.md`).

From the same credentials it also reads `expiresAt` — to short-circuit a
locally-expired access token (the API would reject it anyway, and
agent-bar must never refresh it: the single-use refresh token races
Claude Code) — and `rateLimitTier`, to surface the Max multiplier in the
plan label (e.g. `Max 5x`).

## Window Classification & Display Dedup — `src/formatters/shared.rs`

`classify_window(window_minutes) -> WindowKind` is the single place
tolerance math lives (`fiveHour` = 300±90min, `sevenDay` = 10080±1440min,
else `other`) — providers call it once at the source; QML and the TUI
never re-derive a label from magic numbers. `format_eta`/
`format_reset_time` render the countdown (`Xh Ym`/`Nd Nh`) and the
localized reset clock (`Clock.local_offset`) from the same `resetsAt`
string — the TUI's `fmt_reset` (`src/tui/render/detail/format.rs`) calls
these instead of slicing the raw ISO string.

`is_duplicate_window(a, b) -> bool` collapses two windows that share
`(windowKind, resetsAt, remaining-rounded)` — the display-level fix for
the Codex Plus duplicate-Weekly bug (the JSON envelope still emits
`primary`/`secondary`/`models` untouched; only consumers that render
multiple windows side by side dedup). The TUI consumes it directly; QML
ports the same predicate in JS since it has no Rust runtime.

## The Quota Cache

`src/cache.rs` is a file-based cache (`$XDG_CACHE_HOME/agent-bar/<key>.json`,
default `~/.cache`) with a **5-minute TTL** (`CONFIG.cache.ttl_ms`) and
**cross-process** lifetime — it survives between polls, which is what
matters here: both Widget.qml and Waybar poll at their own configured
interval but each poll is a *separate process*, so an in-memory cache
would never hit. The file-based cache means most polls within a
5-minute window read the cached response from disk and one triggers a
fresh fetch — the provider API is hit at most once per 5-minute window
regardless of the configured poll interval. `get_or_fetch` reads the
cache, and on a miss runs the fetcher once, writing atomically (temp
file + `rename`) because concurrent provider processes can write the
same key. An in-flight dedup map deduplicates concurrent fetches
*within* one process; failed fetches are never cached (a non-200 errors
before `set`); cache keys are validated against path traversal.

There is no separate settings cache — `settings::load` reads
`settings.json` directly on every call; no in-process memoization layer
exists.

## Formatter Layer — `src/formatters/`

Quotas are rendered three ways from the same `ProviderQuota`:

- **JSON** (`json.rs`) → versioned, Pango-free envelope
  (`{ schemaVersion, fetchedAt, providers[] }`), the contract Widget.qml
  and any other native bar consumes. See [`json-output.md`](json-output.md).
- **Waybar** (`waybar.rs`) → `{ text, tooltip, class }` JSON, legacy
  tier. `text` is the bar percentage; `tooltip` is multi-line Pango
  built per provider; `class` is a provider-scoped compound carrying a
  health state for CSS — `agent-bar-<provider> <status>` for a single
  module, or `agent-bar` plus `<provider>-<status>` tokens in aggregate
  (see [Waybar contract](waybar-contract.md)). States:
  `ok`/`low`/`warn`/`critical`/`disconnected`.
- **Terminal** (`terminal.rs`) → ANSI view for `status`. `action-right`
  no longer uses this path — it resolves focus and opens the TUI instead
  (see `src/action_right.rs` below).

Builders (`formatters/builders/`) describe output as `Vec<Line>` of
typed `Segment`s (text + color token). **`render_pango.rs` is the single
XML-escape boundary** — `span()` / `render_pango()` are the only places
provider data gets Pango markup, and they escape every non-`raw`
segment. Builders never escape; a `raw` segment opts out of both
color-wrap *and* escape and must already be safe. Routing untrusted
provider strings around this boundary is a tooltip injection bug, so all
Pango output goes through it. Since the Codex fallback mapping is gone,
`formatters/builders/codex.rs` renders an `other`-kind window with a
label built from its real duration (e.g. `"1h window"`) instead of
assuming it must be the 5h or 7d bucket.

## TUI — `src/tui/`

`agent-bar menu` shares the provider/cache/formatter layers above; its
own render tree (`src/tui/render/`) is a fourth consumer of
`ProviderQuota`, independent of Waybar/Omarchy. Detail's window rows use
the same `classify_window`/`is_duplicate_window`/`format_eta`/
`format_reset_time` helpers as Widget.qml, so the two surfaces never
drift on labels, dedup, or timezone. Config screens hide Waybar-only
fields (separators, signal, interval) when `platform::detect()` reports
Omarchy-only — the same gate `setup`/`update` use.

## Omarchy Shell — `Widget.qml` (primary integration)

The Omarchy 4 (omarchy-shell/Quickshell) plugin is agent-bar's
first-class target: a native bar-widget with per-provider chips and a
popup, driven entirely by `agent-bar --format json` and `agent-bar
config show`/`config apply`. Titlebar actions (refresh, settings mode,
open TUI) are real Unicode icon buttons, not text hints. Settings mode
dual-writes: `config apply` owns `providers`/`providerOrder`/
`displayMode`/`notify.enabled` in `settings.json`; the plugin's own
`refreshIntervalSec` is written inline into Omarchy's `shell.json` via
`bar.shell.updateEntryInline`. Motion (bar fill on open, refresh spin,
hover) is gated by the read-only `menuAnimations` field `config show`
exposes — editing it stays a `settings.json` concern. `agent-bar update`
reinstalls the plugin whenever `detect_omarchy_shell()` is true, so the
binary and the embedded QML never drift apart; `doctor` additionally
checks the installed manifest version against the running binary. Full
plugin contract, click routing, and the dual-write table:
[omarchy-shell.md](omarchy-shell.md).

## Waybar — legacy tier

Waybar keeps working — it gets fixes, not new features. Its contract
(module IDs, CSS classes, click actions, `signal`-based refresh) lives
in [waybar-contract.md](waybar-contract.md) and is implemented by
`src/waybar/contract.rs` and `src/waybar/integration.rs` (grouped under
`src/waybar/`, re-exported at the crate root as `waybar_contract` /
`waybar_integration` so the existing `cargo test waybar_contract` /
`cargo test waybar_integration` filters in `CLAUDE.md` keep working).
`platform::detect()` is what lets every write path (`setup`, `update`,
TUI Config Save) skip touching `~/.config/waybar/` entirely on an
Omarchy-only machine.

## Module Map

| File | Role |
| --- | --- |
| `src/main.rs` | Entry point; dispatch by command/flags. |
| `src/cli.rs` | argv → `CliOptions`; `--help` rendering; command suggestions. |
| `src/platform.rs` | `Platform { omarchy, waybar }` + `detect()` — the single gate `setup`/`update`/TUI-save use before touching Waybar paths. |
| `src/config.rs` | Paths (XDG), cache TTL, API endpoints, color thresholds. |
| `src/config_cmd.rs` | `config show` / `config apply` — editable settings subset (JSON) + read-only `menuAnimations` on `show`. |
| `src/cache.rs` | File-based quota cache (5 min, cross-process, atomic writes). |
| `src/providers/mod.rs` | Registration, parallel fan-out, timeout/retry. |
| `src/providers/base.rs` | `BaseProvider` `get_quota()` orchestration. |
| `src/providers/{claude,codex,amp,grok}.rs` | Concrete providers. Claude is direct; others extend `BaseProvider`. Each sets `windowKind` at the source. |
| `src/providers/types.rs` | `ProviderQuota`, `QuotaWindow` (incl. `windowKind`), `Provider`, `AllQuotas`. |
| `src/formatters/shared.rs` | `classify_window`/`WindowKind`, `is_duplicate_window`, `format_eta`/`format_reset_time`. |
| `src/formatters/json.rs` | Versioned JSON envelope (`schemaVersion`) — the contract Widget.qml consumes. |
| `src/formatters/waybar.rs` | Waybar JSON assembly ({text,tooltip,class}) — legacy tier. |
| `src/formatters/render_pango.rs` | Single XML-escape boundary for Pango. |
| `src/formatters/terminal.rs` | ANSI terminal view. |
| `src/waybar/contract.rs` | Generated Waybar modules/CSS/asset install (re-exported as `crate::waybar_contract`). |
| `src/waybar/integration.rs` | In-place `config.jsonc`/`style.css` patcher (re-exported as `crate::waybar_integration`). |
| `src/notify.rs` | Best-effort low/critical desktop notifications. |
| `src/action_right.rs` | Waybar right-click handler — resolves TUI focus (provider detail or Login) from connection state. |
| `src/omarchy_integration.rs` | Omarchy drop-in install/remove; embeds `Widget.qml` / manifest; `detect_omarchy_shell()`. |

## See Also

- [Commands](commands.md) — public CLI surface, `config`, flags, and internals.
- [Runtime](runtime.md) — owned paths, settings, cache, credentials.
- [Waybar contract](waybar-contract.md) — generated modules, classes, click actions (legacy tier).
- [Omarchy shell](omarchy-shell.md) — plugin drop-in, settings popup, dual-write.
- [JSON output](json-output.md) — `--format json` / `--watch` schema, incl. `windowKind` and `extra` shapes.
- [New provider guide](new-provider.md) — implementing a provider on `BaseProvider`.
