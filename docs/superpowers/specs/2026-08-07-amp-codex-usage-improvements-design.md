# Amp/Codex Usage Collection Improvements — Design

Date: 2026-08-07
Status: approved by owner (this session), pending written-spec review
Scope: two sequential PRs improving what the Amp and Codex adapters collect,
plus inherited F4 hygiene and one v10 contract amendment.

## Background

A deep research pass (10 agents; live JSON-RPC probe of `codex app-server`
0.146.1, live `amp usage` on the owner's account, `openai/codex` source clone,
ampcode.com docs) established that both adapters lag their upstream sources:

- **Amp** shipped monthly subscriptions on 2026-07-18 (Megawatt $20/mo,
  Gigawatt $200/mo — ampcode.com/news/subscriptions). `amp usage` now emits a
  line `Subscription <plan>: X% other usage and Y% orb usage remaining` that
  no regex in `amp_from_usage_text` matches. Two ready-made percentage windows
  are silently dropped. Plan semantics: Megawatt includes "750 hours of small
  orbs" and "$20 included agent usage", so `orb usage` = included orb-hours
  allowance and `other usage` = included agent usage. No fixture covers the
  post-subscription format. `plan` is hardcoded `None`.
- **Codex** `account/rateLimits/read` now returns `limitId`, `limitName`,
  `credits`, `individualLimit`, `spendControlReached`, `rateLimitReachedType`,
  `rateLimitsByLimitId` (multi-bucket map), and `rateLimitResetCredits`
  (rate-limit reset banking, openai/codex#28143, merged 2026-06-15). The
  agent-bar structs declare only `primary`/`secondary`/`planType`.
  `~/.codex/rate-limits.json` no longer exists and has **no writer** in the
  current codex source — cascade stage 2 is dead code.
- **Handoff corrections**: `.superpowers/handoff-v11.md` calls
  `rate_limits_by_limit_id` "speculative" (it is real and populated live) and
  lists the 1 MiB session-log cap as pending (it is implemented and tested).

## Approved decisions (owner)

1. Scope: all four blocks (Amp subscription, Codex fields + multi-bucket,
   inherited F4 hygiene, Codex reset credits with contract amendment).
2. Amp window labels: `Plan · other` and `Plan · orb` (visual mockup option B).
3. Codex reset credits UI: a discreet line below the windows in the popup,
   visible only when count > 0; bar chips unchanged (visual mockup option B).
4. Delivery: two sequential PRs (below).
5. Amp hourly replenish rate: **not included** — no programmatic source
   exposes a rate, and the auto-replenishing quantity (Individual credits) is
   monetary and therefore banned by the v10 contract. Recorded as a known
   limitation to re-evaluate if Amp ever exposes it.

## PR1 — Amp subscription + F4 hygiene

No QML changes; no contract changes.

### Amp parser (`src/providers/v2_map.rs::amp_from_usage_text`)

- New regex for `Subscription <plan>: X% other usage and Y% orb usage
  remaining`. Emits two `UsageWindow`s:
  - id `plan-other`, label `Plan · other`
  - id `plan-orb`, label `Plan · orb`
  - No `resets_at` (Amp documents only "replenishes at the end of each
    monthly period"; no timestamp is exposed). Percent semantics follow the
    existing remaining/used display-metric handling.
- Plan extraction from the same line: `Plan { id: lowercased, label: as
  printed }` (e.g. `megawatt` / `Megawatt`). Removes the hardcoded
  `plan: None`.
- `Individual credits: $X remaining …` becomes an explicitly recognized and
  intentionally discarded monetary line (comment-documented, mirroring the
  existing "Explicitly ignored monetary field" pattern) instead of being
  invisible by regex accident.
- An unrecognized line never aborts parsing of the remaining lines.

### F4 hygiene

- **Amp failure classifier** (`src/providers/adapters.rs`): drop the
  substring-`"auth"` rule. `Unauthenticated` requires explicit markers
  ("not signed in", "sign in", "unauthorized"); other operational failures
  classify as typed network/provider errors.
- **Grok period type**: read `currentPeriod.type` (e.g.
  `USAGE_PERIOD_TYPE_WEEKLY`) instead of hardcoding the `weekly` id/label.
  Unknown types map deterministically: strip the `USAGE_PERIOD_TYPE_` prefix,
  lowercase for the id, title-case for the label. The plan-label formatting
  helper (raw tier → presentable label, unknown → title-cased raw) is
  introduced in this PR for Grok; PR2 reuses it for Codex.
- **Fixtures/tests**: new fixture with the real 4-line subscription output;
  cases for free-only accounts, legacy dollar format, subscription line
  present/absent, and classifier behavior on network-flavored stderr.

## PR2 — Codex fields, multi-bucket, contract amendment, reset-credit line

### Schema dedup (F3) and new fields

Unify the two parallel struct families (`codex_app_server.rs` ×
`v2_map.rs`) into one module declaring the full
`GetAccountRateLimitsResponse`: `limitId`, `limitName`,
`credits { hasCredits, unlimited, balance }` (balance explicitly discarded as
monetary, comment-documented), `individualLimit`, `spendControlReached`,
`rateLimitReachedType`, `rateLimitsByLimitId`, `rateLimitResetCredits`.

### Multi-bucket iteration

- Iterate **all** keys of `rateLimitsByLimitId`; prefer bucket `codex`
  explicitly as the primary source (today's first-alphabetical-key behavior
  is correct only by coincidence).
- Extra buckets that carry `primary`/`secondary` data become additional
  windows with id `codex:<limit_id>`. Buckets with null windows (the
  real-world `premium` marker observed in 429 events) are skipped without
  error.
- Tests: map with 2+ keys; a key with null primary; preference for `codex`
  over alphabetically-earlier keys.
- `individualLimit` (spend-control percentage; not monetary) becomes an
  additional window labeled `Workspace limit` when present.

### Cascade cleanup and freshness

- Remove stage 2 (`~/.codex/rate-limits.json`) — dead upstream. Cascade
  becomes app-server → session-log scan.
- Honest freshness (F3): session-log-sourced data must not present as live;
  it carries its age (stale presentation path already exists).

### Contract amendment + reset-credit surface

- Amend `docs/specs/v10` (JSON-022B): monetary credits (balance, spend,
  currency) remain banned; a **non-monetary quota-reset count** is a distinct,
  permitted concept. To keep the banned word out of the wire format, the new
  schema-v2 field is `rate_limit_resets_available: Option<u32>` on the ready
  provider payload.
- QML (`CoreView.js`, `ProviderView.qml`): discreet line below the windows,
  `↻ N rate-limit resets available`, muted foreground, rendered only when
  N > 0. Bar chips unchanged. No notifications. Redemption stays in
  ChatGPT/Codex — agent-bar is read-only.
- `planType` formatting: `codex_plan()` mirroring `claude_plan()` ("plus" →
  "Plus", "business" → "Business"; unknown values render title-cased raw),
  reusing the formatting helper introduced in PR1.

### Docs

- Fix `.superpowers/handoff-v11.md`: `rate_limits_by_limit_id` is real
  (extend, don't remove); 1 MiB cap already implemented.

## Testing and verification

- TDD per track (failing test first, per repository workflow).
- Fixtures derived from the live payloads captured during research (Amp
  4-line output; full `GetAccountRateLimitsResponse`).
- Checkpoints: `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets
  -- -D warnings`, `git diff --check`. PR2 additionally runs the Qt6 QML
  battery (qmllint via Qt6 binary, `omarchy plugin validate`, qmltestrunner
  via Qt6 binary) since it touches plugin QML.
- Operational failures remain typed data; no new field may introduce a crash
  path or leak raw provider text into QML.

## Out of scope (recorded, not included)

- OpenCode provider (gate: anomalyco/opencode#16017 / PR #16513).
- F5 native-widget parity.
- Codex `account/usage/read` (lifetime tokens/streaks/daily buckets) — banned
  by v10 ("Session history and charts").
- Notifications for reset credits.
- Amp hourly replenish rate (no source; monetary if it existed).
