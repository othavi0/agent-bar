# Design — Codex retry + Grok weekly usage

Date: 2026-07-27  
Status: approved (brainstorming)  
Scope: fix Codex collection so Retry works when CLI-authenticated; replace Grok
Context window with SuperGrok weekly usage + reset.

## 1. Problem statement

### 1.1 Codex Retry appears to do nothing

Observed on a host with Codex CLI logged in (`codex login status` → ChatGPT;
`~/.codex/auth.json` present):

- `agent-bar status provider codex` → `provider_error`:
  `Codex rate limits were not available.` + action `Retry`
- `~/.codex/rate-limits.json` does **not** exist
- `~/.codex/sessions/**/*.jsonl` **does** contain `token_count` events with
  `rate_limits` (`used_percent`, `window_minutes`, `resets_at`)

Root cause: the v10 `CodexAdapter` is a stub. It only reads
`$HOME/.codex/rate-limits.json`. If missing, it ignores the resolved `codex`
executable (`let _ = exe`) and returns a retryable provider error. Retry in
QML correctly force-refreshes, but re-runs the same incomplete path.

This contradicts the locked v10 collection policy in
`docs/specs/v10/02-target-architecture.md`:

> Codex: `codex app-server` JSON-RPC `rateLimits/read`, then newest valid
> rate-limit event below `$HOME/.codex/sessions`

The pre-v10 implementation (`src/providers/codex/{app_server,session_log}.rs`)
already implemented that pipeline and was removed during the v10 legacy cut.

### 1.2 Grok shows Context instead of weekly usage

Current behavior is intentional under the 2026-07-17 Grok provider design:
primary metric is session **context** remaining from `signals.json`.

Product request: show SuperGrok **weekly usage limit** with correct reset.

Evidence that the weekly metric exists for the Build CLI OAuth path:

- Grok CLI logs `billing: fetched credits config` with:
  - `creditUsagePercent` (used %)
  - `currentPeriod.type = USAGE_PERIOD_TYPE_WEEKLY`
  - `currentPeriod.start` / `currentPeriod.end` (reset bound)
  - `billingPeriodStart` / `billingPeriodEnd`
  - `subscriptionTiers` (e.g. SuperGrok)
- Binary strings include path `/billing?format=credits`, Bearer auth, and
  `x-grok-client-mode`
- Local signals.json has no plan-quota fields (only context tokens)

So Context is not a UI bug; it is the wrong product metric. Weekly must come
from the same billing surface the CLI already calls, not from inventing a
percentage from session signals.

## 2. Goals and non-goals

### Goals

1. **Codex:** when CLI-authenticated and any live source can supply rate
   limits, status is `ready` with Session/Weekly (or `other:*`) windows and
   correct `resetsAt`. Retry force-refreshes and surfaces updated data or a
   clear typed failure.
2. **Grok:** chip and popup show a single **Weekly** window (not Context),
   with used/remaining percent and reset at period end.
3. Preserve v10 hard rules: no monetary fields in status/cache/UI/logs; never
   log credentials/tokens; provider failures stay typed inside schema-v2;
   QML does not parse raw provider payloads.

### Non-goals

- SuperGrok chat-web limits as a separate source
- Amp/Claude collection changes
- Emitting prepaid/on-demand balances or dollar amounts
- Keeping Context as a parallel Grok window
- Reading Grok weekly from `unified.jsonl` (not a contract)
- QML redesign beyond what status data already drives
- Live desktop mutation during unit tests

## 3. Approach (selected)

**Approach A — restore Codex composite collection + Grok billing HTTPS.**

Rejected alternatives:

| Approach | Why not |
| --- | --- |
| B — Grok weekly from `unified.jsonl` only | Stale unless CLI recently ran; log is not a public contract |
| C — Codex session-log only (no app-server) | Stale without recent sessions; weaker than v10; Retry can still look dead |

## 4. Architecture

### 4.1 Data flow

```text
Service.qml (Retry / poll)
  → agent-bar status [force bypass when Retry]
      → CodexAdapter
          1) codex app-server JSON-RPC rateLimits/read
          2) fallback session-log walk under ~/.codex/sessions
          3) optional ~/.codex/rate-limits.json if present (fixtures/tests)
          → ProviderResult → schema v2
      → GrokAdapter
          1) auth.json gate (key present; same login semantics as today)
          2) HTTPS GET /billing?format=credits (CLI-equivalent)
          → weekly UsageWindow only
```

### 4.2 Codex collection

**Source order (mandatory):**

1. **App-server (preferred live)**  
   - Resolve `codex` via catalog discovery.  
   - Spawn app-server over stdio; run JSON-RPC handshake + `rateLimits/read`
     (port v9 `run_appserver_protocol` into current process seams).  
   - Timeout: catalog timeout (10s).  
   - One additional attempt only on app-server **timeout**, then filesystem
     fallback (v10 retry table).  
   - Auth / malformed / non-timeout failures do not count as that retry.

2. **Session-log fallback**  
   - Bounded walk under `$HOME/.codex/sessions` matching v10 limits:
     no link follow, depth ≤ 8, ≤ 4096 directory entries, 1 MiB per file,
     ≤ 256 candidates, sort mtime desc then path bytes asc.  
   - Valid event: `payload.type == "token_count"` with `rate_limits`.  
   - Prefer newest by event timestamp, then path/line tie-break as in ARCH.

3. **Optional static file**  
   - If `$HOME/.codex/rate-limits.json` exists and parses, may be used as a
     source (tests/fixtures). Must not be the only production path.

**Normalization:**

- Accept camelCase and snake_case window fields
  (`usedPercent`/`used_percent`, `windowDurationMins`/`window_minutes`,
  `resetsAt`/`resets_at`).
- Label by duration, **not** by primary/secondary slot:

  | window_minutes | id | label |
  | --- | --- | --- |
  | 300 (or session-like primary when duration matches catalog norms) | `session` | Session |
  | 10080 | `weekly` | Weekly |
  | other finite | `other:<mins>:<ordinal>` | short plain label (`{mins}m`) |

- Critical regression: host samples show **primary** with
  `window_minutes = 10080` and `secondary = null`. Mapping primary always to
  Session is wrong.
- Discard `credits` / balance / monetary fields.
- `plan_type` → optional `Plan { id, label }` as plain text when present.

**States:**

| Situation | Result |
| --- | --- |
| Not authenticated (no usable auth / app-server auth failure) | `unauthenticated` + login when available |
| Transient network/timeout after policy | `network_error` retryable |
| All sources miss rate limits | `provider_error` retryable, clear message |
| Malformed payload | `provider_error` non-retryable |
| Success | `ready` with windows |

**Why Retry works after the fix:** force refresh re-runs app-server (live) or
re-reads session logs that already contain weekly/session buckets on this host.

### 4.3 Grok collection (Weekly replaces Context)

**Source order:**

1. Auth gate from `$GROK_HOME/auth.json` (or `$HOME/.grok`): require non-empty
   `key` under the selected entry (existing multi-entry selection rules).
2. Authenticated HTTPS GET to the CLI billing endpoint:
   - Path: `/billing?format=credits` (literal from Grok CLI binary).
   - Host base: CLI proxy host (`cli-chat-proxy.grok.com` family).  
     **Implementation must pin the exact absolute URL via equality tests**
     after a controlled discovery probe; no free-form URL construction at
     runtime beyond that constant.
   - Headers: `Authorization: Bearer <key>`, plus the CLI’s
     `x-grok-client-mode` header (literal value fixed in tests).
   - Transport: existing `HttpClient` (HTTPS only, no redirects, body cap
     1 MiB, timeout from catalog).
3. Do **not** emit a Context window from `signals.json` in the ready path.
4. Do **not** parse `~/.grok/logs/unified.jsonl`.

**Mapping (from observed CLI billing config):**

```text
creditUsagePercent              → used_percent
remaining                       → (100 - used).clamp(0, 100)
currentPeriod.type == WEEKLY
  (or period span ≈ 7 days)     → window id weekly
currentPeriod.end
  or billingPeriodEnd           → resetsAt
subscriptionTiers / plan label  → Plan plain text when safe
```

Discard: `prepaidBalance`, `onDemandCap`, `onDemandUsed`, any currency fields.

**Single window product decision (approved):**

| id | label | resets |
| --- | --- | --- |
| `weekly` | Weekly | period end |

**States:**

| Situation | Result |
| --- | --- |
| No/invalid auth | `unauthenticated` + Connect when login available |
| HTTP timeout / transport failure | `network_error` retryable (one retry) |
| 401/403 after auth file present | `unauthenticated` or typed provider error; never silent Context fallback |
| 5xx / unparseable body | `provider_error` retryable where appropriate |
| Success without usable percent | `ready` with **empty** windows (chip `—`), not Context |

### 4.4 Catalog / contract updates

Update locked collection table for Grok in `docs/specs/v10/02-target-architecture.md`
and any active docs that claim Grok primary is context:

| Field | Before | After |
| --- | --- | --- |
| Grok sources | auth + signals walk | auth + billing HTTPS |
| Window IDs | `context` | `weekly` |
| Labels | Context | Weekly |
| Retry | none | one network/timeout retry |
| TTL | 90s | 90s |

Codex row stays as already specified; implementation must match it.

### 4.5 UI layer

No QML contract change required if schema-v2 windows update correctly.

- Retry continues to call `retryProvider` → `refreshProvider(id, true)`.
- Chip primary metric follows existing remaining/used display settings.
- Optional loading affordance on Retry is out of scope (YAGNI unless an
  existing spinner pattern is trivial to reuse).

### 4.6 Privacy and security

- Never log Authorization headers, JWT/`key`, refresh tokens, account emails.
- Cache only normalized schema-v2 (percentages, resetsAt, sanitized plan).
- Fixtures synthetic only.
- Redaction on HTTP errors must strip URL query auth and header echoes
  (existing Claude HTTP client rules apply to Grok billing).

## 5. Module sketch

Keep adapters thin; parsers pure.

```text
src/providers/adapters.rs     # Codex/Grok collect orchestration
src/providers/v2_map.rs       # pure parsers (codex windows by duration;
                              # grok_from_billing_json; remove context-primary path)
src/providers/codex/          # optional split if app-server+session_log grow:
                              #   app_server.rs, session_log.rs
src/providers/http.rs         # reuse GET; no secrets in errors
src/providers/catalog.rs      # Grok timeout/retry metadata if needed
tests/fixtures/providers/codex/
tests/fixtures/providers/grok/  # billing-weekly.json, billing-unauth shapes
```

Prefer porting proven v9 Codex protocol code over inventing a new RPC dialect.

## 6. Testing and acceptance

### Codex unit/integration

- Scripted app-server success → Session + Weekly with resets.
- Session-log snake_case `token_count.rate_limits` fallback.
- Primary-only weekly (`window_minutes=10080`) → id `weekly` (mislabel regression).
- All sources missing → retryable provider_error.
- Auth failure → unauthenticated when applicable.
- Walk bounds (depth/entry caps) enforced.
- Credits absent from domain result.

### Grok unit/integration

- Billing fixture → single `weekly` window; remaining = 100 − used; resetsAt = end.
- No `context` window in ready result.
- Monetary fields discarded.
- 401 → unauthenticated path.
- Timeout → network_error retryable.
- Literal URL + required headers equality tests (ARCH-018 style).
- Auth missing → unauthenticated without HTTP call.

### Manual / QA smoke (authorized host only)

1. `status provider codex cache bypass` while CLI logged in → ready with windows
   or a non-stub typed state that changes after a real Codex session event.
2. Popup Retry on Codex updates `lastSuccessAt` / windows when data available.
3. `status provider grok cache bypass` → Weekly % near CLI-observed
   `creditUsagePercent`, reset matching period end.
4. Chip Grok shows weekly remaining/used, not Context label.

### Verification gate

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
git diff --check
```

QML suite only if assets change.

### Docs to update in the same implementation plan

- `docs/specs/v10/02-target-architecture.md` (Grok collection row)
- `docs/new-provider.md` / `docs/troubleshooting.md` as needed
- `README.md` / `PRODUCT.md` if they still claim Grok context-only
- Historical Grok design (2026-07-17) remains historical; this doc supersedes
  the product metric decision for v10 follow-up

## 7. Risks

| Risk | Mitigation |
| --- | --- |
| Codex app-server protocol drift vs CLI 0.145+ | Port v9, fixture against current CLI strings; session-log fallback |
| Grok billing URL/header drift | Literal constants + equality tests; fail typed on parse/HTTP change |
| Undocumented billing API | Same surface the official CLI uses; no credential install; degrade typed |
| Primary weekly mislabel | Duration-based window ids + explicit regression fixture |
| Silent Context fallback reintroduced | Tests assert zero `context` windows on Grok ready path |

## 8. Implementation order (plan seed)

1. Codex parsers + session-log fallback (unblocks Retry using local data).
2. Codex app-server path + timeout retry policy.
3. Grok billing parser + HTTP collect; remove Context primary path.
4. Catalog/docs contract updates.
5. Full verification gate + optional live smoke.

## 9. Success criteria

- Codex Retry no longer loops on the stub-only message when rate limits exist
  via app-server or session log.
- Grok chip/popup show Weekly usage with correct reset, not Context.
- No money fields, no token leakage, schema-v2 only on stdout.
- Tests and clippy gate green.
