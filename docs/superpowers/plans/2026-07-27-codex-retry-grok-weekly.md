# Codex Retry + Grok Weekly Usage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Codex Retry return real Session/Weekly rate limits when the CLI is authenticated, and make Grok show SuperGrok weekly usage with reset instead of session Context.

**Architecture:** Complete the stubbed v10 `CodexAdapter` with app-server JSON-RPC plus bounded session-log fallback (restore pre-v10 behavior into current seams). Switch `GrokAdapter` from `signals.json` context to authenticated HTTPS billing (`/billing?format=credits`) producing a single `weekly` window. Pure parsers live in `v2_map`; adapters orchestrate I/O only.

**Tech Stack:** Rust 2021, tokio process/HTTP, serde_json, time, existing `CollectionContext` / `HttpClient` / `FileSystem` seams, cargo test.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-27-codex-retry-grok-weekly-design.md`
- v10 catalog policy: `docs/specs/v10/02-target-architecture.md` (Codex row already correct; Grok row must be updated in Task 6)
- No production `unwrap()` / `expect()`
- No monetary fields in `ProviderResult` / cache / logs / UI
- Never log credentials, JWTs, Authorization headers, or account emails
- QML must not parse raw provider payloads (no QML change required if schema-v2 is correct)
- Status stdout remains one schema-v2 object + newline
- Conventional Commits English, subject ≤ 50 characters
- Zero AI attribution in commits
- Tests: fake process/HTTP/filesystem/clock only; no live network in unit tests
- Do not mutate live Omarchy/Hyprland paths
- Read each file before Edit; re-Read after other agents/git changes
- Implementers: after git pull/checkout, previous Read is dead

## File map

```text
src/providers/v2_map.rs              # Codex duration-based windows; Grok billing parser; drop Context primary
src/providers/codex_session_log.rs   # NEW: bounded walk + extract rate_limits from token_count events
src/providers/codex_app_server.rs    # NEW: JSON-RPC protocol + spawn app-server (bidirectional stdio)
src/providers/adapters.rs            # Wire Codex/Grok collect order; Grok token extraction for HTTP
src/providers/catalog.rs             # GROK.retry_policy → OneTransient; constants if needed
src/providers/mod.rs                 # mod codex_session_log; mod codex_app_server
tests/fixtures/providers/codex/      # session-log jsonl + appserver-shaped JSON samples
tests/fixtures/providers/grok/       # billing-weekly.json (no secrets)
docs/specs/v10/02-target-architecture.md
docs/troubleshooting.md              # Codex retry / Grok weekly notes
README.md / PRODUCT.md               # only if they still claim Grok context-only
```

Optional: keep all Codex helpers in one file if under ~400 LOC; prefer the two new modules above for reviewability.

---

### Task 1: Codex window normalization by duration

**Files:**
- Modify: `src/providers/v2_map.rs` (`codex_from_rate_limits_json`, `codex_window`, helpers)
- Test: unit tests in `src/providers/v2_map.rs` (`#[cfg(test)]`)
- Fixture (optional): `tests/fixtures/providers/codex/primary-weekly-only.json`

**Interfaces:**
- Consumes: existing `UsageWindow::try_new`, `ProviderResult::Ready`
- Produces: `codex_from_rate_limits_json(bytes, now) -> ProviderResult` where each raw window is labeled by `window_minutes`, not by primary/secondary slot order
- Produces: helper `fn codex_window_identity(window_minutes: Option<i64>, ordinal: usize) -> (String, String)`  
  - `Some(10080)` → `("weekly", "Weekly")`  
  - `Some(300)` → `("session", "Session")`  
  - `Some(n)` other → `(format!("other:{n}:{ordinal}"), format!("{n}m"))`  
  - `None` on primary slot → treat as session; on secondary → weekly (compat with incomplete payloads)

- [ ] **Step 1: Write the failing test for primary-only weekly**

Add to `v2_map` tests:

```rust
#[test]
fn codex_primary_only_weekly_is_labeled_weekly() {
    let body = br#"{
      "primary": {"used_percent": 1.0, "window_minutes": 10080, "resets_at": 1785628013},
      "plan_type": "plus"
    }"#;
    let now = time::macros::datetime!(2026-07-26 18:00:00 UTC);
    let result = codex_from_rate_limits_json(body, now);
    match result {
        ProviderResult::Ready { windows, plan, .. } => {
            assert_eq!(windows.len(), 1);
            assert_eq!(windows[0].id(), "weekly");
            assert_eq!(windows[0].label(), "Weekly");
            assert!((windows[0].used_percent() - 1.0).abs() < 0.01);
            assert!(plan.is_some());
        }
        other => panic!("expected ready, got {other:?}"),
    }
}
```

Also update/extend the existing camelCase fixture test so dual windows still map session + weekly when durations are 300 / 10080.

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test codex_primary_only_weekly_is_labeled_weekly -- --nocapture
```

Expected: FAIL — current code labels primary as `session`.

- [ ] **Step 3: Implement duration-based labeling**

In `codex_from_rate_limits_json`:

1. Build windows from `primary` and `secondary` independently.
2. Call `codex_window_identity` for each (ordinal 1, then 2; if collision on `other:…` bump ordinal).
3. Keep camelCase + snake_case aliases already on `CodexWindowRaw`.
4. Continue discarding `credits`.

Minimal helper:

```rust
fn codex_window_identity(window_minutes: Option<i64>, ordinal: usize) -> (String, String) {
    match window_minutes {
        Some(10080) => ("weekly".into(), "Weekly".into()),
        Some(300) => ("session".into(), "Session".into()),
        Some(n) if n > 0 => (format!("other:{n}:{ordinal}"), format!("{n}m")),
        _ => {
            if ordinal == 1 {
                ("session".into(), "Session".into())
            } else {
                ("weekly".into(), "Weekly".into())
            }
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test codex_ -- --nocapture
```

Expected: PASS for all codex parser tests.

- [ ] **Step 5: Commit**

```bash
git add src/providers/v2_map.rs tests/fixtures/providers/codex
git commit -m "fix: label Codex windows by duration"
```

---

### Task 2: Codex session-log fallback

**Files:**
- Create: `src/providers/codex_session_log.rs`
- Modify: `src/providers/mod.rs` (`mod codex_session_log;`)
- Modify: `src/providers/adapters.rs` (`CodexAdapter::collect` order)
- Create: `tests/fixtures/providers/codex/session-token-count.jsonl`
- Test: unit tests in `codex_session_log.rs`

**Interfaces:**
- Consumes: `FileSystem` (for optional direct reads in tests) + std walk for directory listing (same pattern as Grok signals walk)
- Produces:
  - `pub fn extract_rate_limits_from_jsonl(bytes: &[u8]) -> Option<Vec<u8>>`  
    reverse-scan lines; first `payload.type == "token_count"` with `payload.rate_limits` object → re-serialize that object as JSON bytes for `codex_from_rate_limits_json`
  - `pub fn find_latest_rate_limits(sessions_dir: &Path) -> Option<Vec<u8>>`  
    bounded walk: no symlinks, depth ≤ 8, visits ≤ 4096, candidates ≤ 256 jsonl files, sort mtime desc then path asc; scan each candidate reverse for token_count; return first hit’s rate_limits JSON bytes

**Adapter order after this task (still without app-server):**

1. `rate-limits.json` if present  
2. `find_latest_rate_limits(home.join(".codex/sessions"))`  
3. else if no collection exe → `cli_missing`  
4. else retryable `provider_error` (“Codex rate limits were not available.”)

- [ ] **Step 1: Write fixture + failing tests**

`tests/fixtures/providers/codex/session-token-count.jsonl` (synthetic, no secrets):

```json
{"timestamp":"2026-07-25T23:00:00.000Z","type":"event","payload":{"type":"message"}}
{"timestamp":"2026-07-25T23:01:00.000Z","type":"event","payload":{"type":"token_count","rate_limits":{"primary":{"used_percent":12.5,"window_minutes":10080,"resets_at":1785628013},"plan_type":"plus"}}}
```

Test:

```rust
#[test]
fn extract_token_count_rate_limits_from_jsonl() {
    let bytes = include_bytes!("../../tests/fixtures/providers/codex/session-token-count.jsonl");
    let raw = extract_rate_limits_from_jsonl(bytes).expect("limits");
    let now = time::macros::datetime!(2026-07-26 18:00:00 UTC);
    match crate::providers::v2_map::codex_from_rate_limits_json(&raw, now) {
        ProviderResult::Ready { windows, .. } => {
            assert_eq!(windows[0].id(), "weekly");
            assert!((windows[0].used_percent() - 12.5).abs() < 0.01);
        }
        other => panic!("{other:?}"),
    }
}
```

Add a temp-dir walk test that writes the fixture under `sessions/2026/07/25/rollout.jsonl` and asserts `find_latest_rate_limits` returns the same weekly window.

- [ ] **Step 2: Run tests — expect FAIL**

```bash
cargo test codex_session_log -- --nocapture
```

Expected: FAIL (module missing).

- [ ] **Step 3: Implement `codex_session_log.rs` and wire adapter**

Implementation notes:

- Parse each non-empty line as `serde_json::Value`.
- Match `payload.type == "token_count"` and `payload.rate_limits` is object.
- Scan reverse so the latest line wins inside a file.
- Walk mirrors Grok’s bounds in `adapters.rs` (`find_latest_signals` / `walk_signals_std`).
- Prefer newest candidate that yields extractable limits (not merely newest file without limits).

Wire in `CodexAdapter::collect` after `rate-limits.json` miss.

- [ ] **Step 4: Run tests — expect PASS**

```bash
cargo test codex_session_log -- --nocapture
cargo test providers::adapters -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add src/providers/codex_session_log.rs src/providers/mod.rs src/providers/adapters.rs tests/fixtures/providers/codex
git commit -m "feat: Codex session-log rate limits"
```

---

### Task 3: Codex app-server JSON-RPC collection

**Files:**
- Create: `src/providers/codex_app_server.rs`
- Modify: `src/providers/mod.rs`
- Modify: `src/providers/adapters.rs` (prefer app-server before session-log when exe present)
- Test: unit tests with in-memory duplex / scripted lines in `codex_app_server.rs`

**Interfaces:**
- Consumes: resolved `codex` executable path, version string for `clientInfo.version` (use crate version / `app_identity` if available; else `"agent-bar"`)
- Produces:
  - `pub async fn run_appserver_protocol<R, W>(reader: R, writer: W, version: &str, timeout: Duration) -> Option<Vec<u8>>`  
    where `R: AsyncRead + Unpin`, `W: AsyncWrite + Unpin`  
    Returns JSON bytes of a document acceptable to `codex_from_rate_limits_json` (primary/secondary/plan_type), or `None`
  - `pub async fn fetch_rate_limits_via_appserver(exe: &Path, version: &str, timeout: Duration) -> Option<Vec<u8>>`  
    spawns `exe app-server` with piped stdio, stderr null, kill_on_drop, runs protocol, kills child

**Protocol (from pre-v10, method names verified in history):**

1. Write `{"method":"initialize","id":0,"params":{"clientInfo":{"name":…,"title":…,"version":…}}}`
2. On id=0 result → write `initialized` notification, then  
   `account/read` id=1 `{refreshToken:false}`, then  
   `account/rateLimits/read` id=2 `{}`
3. On id=2 error → return `None` immediately (do not wait hard timeout)
4. On id=2 result → parse rate limits; merge plan_type from id=1 when present
5. Normalize app-server camelCase windows into the same shape as session-log / `codex_from_rate_limits_json`

**Adapter final order:**

1. If collection exe present: try app-server once; on **timeout only**, sleep 250ms and try once more; then fall through  
2. `rate-limits.json`  
3. session-log fallback  
4. typed miss / cli_missing

Note: `ProcessRunner::run` is one-shot without interactive stdin — **do not** force app-server through it. Dedicated spawn in `codex_app_server` is correct (matches architecture: “composite process”, not fake command abstraction).

- [ ] **Step 1: Write protocol unit test with scripted reader/writer**

```rust
#[tokio::test]
async fn appserver_protocol_reads_rate_limits() {
    use tokio::io::{duplex, AsyncWriteExt};
    let (client, mut server) = duplex(64 * 1024);
    let (client_read, client_write) = tokio::io::split(client);

    let server_task = tokio::spawn(async move {
        // Read initialize line, respond id=0, then serve id=1/2 after requests
        // (implement with BufReader line loop; keep fixture compact)
    });

    let out = run_appserver_protocol(
        client_read,
        client_write,
        "10.0.0",
        std::time::Duration::from_secs(2),
    )
    .await
    .expect("limits json");
    let now = time::macros::datetime!(2026-07-26 18:00:00 UTC);
    match crate::providers::v2_map::codex_from_rate_limits_json(&out, now) {
        ProviderResult::Ready { windows, .. } => assert!(!windows.is_empty()),
        other => panic!("{other:?}"),
    }
    let _ = server_task.await;
}
```

Include a second test: id=2 error object → `None` quickly (no hang).

- [ ] **Step 2: Run test — expect FAIL**

```bash
cargo test appserver_protocol -- --nocapture
```

- [ ] **Step 3: Port protocol from v9 + wire adapter**

Source to port (read-only reference, do not restore whole v9 tree):

```bash
git show 7556db3:src/providers/codex/app_server.rs
```

Adapt types to return `Vec<u8>` for `codex_from_rate_limits_json` instead of old `CodexRateLimits` domain.

Wire `CodexAdapter::collect`:

```rust
if let Some(exe) = collection_exe(discovery) {
    let timeout = CODEX.timeout;
    let version = env!("CARGO_PKG_VERSION");
    let mut attempt = fetch_rate_limits_via_appserver(exe, version, timeout).await;
    if attempt.is_none() {
        // Only the adapter-level "one transient retry" for timeout is required by
        // catalog; if fetch cannot distinguish timeout vs auth miss, skip second
        // attempt when error path was immediate JSON-RPC error. Prefer having
        // fetch return an enum { Timeout, Failed, Ok(bytes) } if cheap.
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        attempt = fetch_rate_limits_via_appserver(exe, version, timeout).await;
    }
    if let Some(bytes) = attempt {
        return codex_from_rate_limits_json(&bytes, context.clock.now_utc());
    }
}
// then rate-limits.json, session-log, typed miss
```

Refine retry: only second attempt on timeout if you introduce `AppServerOutcome::{Ok, TimedOut, Failed}`.

- [ ] **Step 4: Run tests**

```bash
cargo test codex_ -- --nocapture
cargo test providers:: -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/providers/codex_app_server.rs src/providers/mod.rs src/providers/adapters.rs
git commit -m "feat: Codex app-server rate limits"
```

---

### Task 4: Grok billing parser (weekly only)

**Files:**
- Modify: `src/providers/v2_map.rs`
- Create: `tests/fixtures/providers/grok/billing-weekly.json`
- Test: `v2_map` unit tests
- Remove/replace: `grok_signals_context_window` expectations that require `context` as product primary

**Interfaces:**
- Produces: `pub fn grok_from_billing_json(bytes: &[u8], account_label: Option<String>, now: OffsetDateTime, login_available: bool) -> ProviderResult`
- Mapping:
  - `creditUsagePercent` → used
  - remaining = `(100.0 - used).clamp(0.0, 100.0)`
  - `currentPeriod.end` or `billingPeriodEnd` (RFC3339) → `resetsAt`
  - window id/label always `weekly` / `Weekly`
  - `subscriptionTiers` string → optional `Plan`
- Discards: prepaid/onDemand balances and any money-like objects
- On missing/invalid percent: `Ready` with **empty** windows (not Context)
- Does **not** read signals / emit `context`

Fixture `tests/fixtures/providers/grok/billing-weekly.json`:

```json
{
  "creditUsagePercent": 33.0,
  "currentPeriod": {
    "type": "USAGE_PERIOD_TYPE_WEEKLY",
    "start": "2026-07-24T21:10:59.543182+00:00",
    "end": "2026-07-31T21:10:59.543182+00:00"
  },
  "billingPeriodStart": "2026-07-24T21:10:59.543182+00:00",
  "billingPeriodEnd": "2026-07-31T21:10:59.543182+00:00",
  "prepaidBalance": { "val": 99 },
  "onDemandCap": { "val": 0 },
  "onDemandUsed": { "val": 0 },
  "isUnifiedBillingUser": true,
  "subscriptionTiers": "SuperGrok"
}
```

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn grok_billing_weekly_window_discards_money() {
    let body = include_bytes!("../../tests/fixtures/providers/grok/billing-weekly.json");
    let now = time::macros::datetime!(2026-07-27 12:00:00 UTC);
    match grok_from_billing_json(body, Some("Ada".into()), now, true) {
        ProviderResult::Ready { windows, plan, .. } => {
            assert_eq!(windows.len(), 1);
            assert_eq!(windows[0].id(), "weekly");
            assert_eq!(windows[0].label(), "Weekly");
            assert!((windows[0].used_percent() - 33.0).abs() < 0.01);
            assert!((windows[0].remaining_percent() - 67.0).abs() < 0.01);
            assert!(windows[0].resets_at().is_some());
            assert_eq!(plan.as_ref().map(|p| p.label.as_str()), Some("SuperGrok"));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn grok_billing_ready_never_emits_context() {
    let body = include_bytes!("../../tests/fixtures/providers/grok/billing-weekly.json");
    let now = time::macros::datetime!(2026-07-27 12:00:00 UTC);
    match grok_from_billing_json(body, None, now, true) {
        ProviderResult::Ready { windows, .. } => {
            assert!(windows.iter().all(|w| w.id() != "context"));
        }
        other => panic!("{other:?}"),
    }
}
```

Keep `grok_from_auth_and_signals` only if still needed for unauthenticated helper tests; otherwise delete Context path and update `grok_ready_from_auth_and_signals_fixture` adapter tests in Task 5.

- [ ] **Step 2: Run — expect FAIL**

```bash
cargo test grok_billing -- --nocapture
```

- [ ] **Step 3: Implement parser**

```rust
pub fn grok_from_billing_json(
    bytes: &[u8],
    account_label: Option<String>,
    now: OffsetDateTime,
    login_available: bool,
) -> ProviderResult {
    // parse; on total JSON failure → ProviderError non-retryable
    // build single weekly window when creditUsagePercent finite
    // never include context
}
```

- [ ] **Step 4: Run — expect PASS**

```bash
cargo test grok_billing -- --nocapture
cargo test grok_ -- --nocapture
```

- [ ] **Step 5: Commit**

```bash
git add src/providers/v2_map.rs tests/fixtures/providers/grok/billing-weekly.json
git commit -m "feat: parse Grok weekly billing JSON"
```

---

### Task 5: Grok adapter HTTP billing collect

**Files:**
- Modify: `src/providers/adapters.rs` (`GrokAdapter`, auth token extract)
- Modify: `src/providers/catalog.rs` (`GROK.retry_policy = RetryPolicy::OneTransient`)
- Modify: adapter tests that expect `context` window
- Export constant: `pub const GROK_BILLING_URL: &str = "https://cli-chat-proxy.grok.com/v1/billing?format=credits";`  
  (If a controlled offline probe during implementation shows a different host/path used by the installed CLI, **update the constant and this plan’s equality tests** in the same commit — do not leave a soft default.)

**Headers (literals for equality tests):**

```text
Authorization: Bearer <key>
x-grok-client-mode: cli
```

If the installed CLI uses a different `x-grok-client-mode` value, pin the observed value and document it in the commit body. Do not send tokens to logs.

**Interfaces:**
- Consumes: `context.http.get(url, headers, max_body)`
- Consumes: auth.json key via `parse_grok_auth_token(bytes) -> Option<(String /*token*/, Option<String> /*label*/)>`  
  (extend current `parse_grok_auth` — token used only for the request, never stored in `ProviderResult`)
- Produces: domain result from `grok_from_billing_json`
- Retry: on `HttpError::Network` / timeout-class failure only, one extra attempt after 250ms when `GROK.retry_policy` is `OneTransient`

**Collect algorithm:**

1. Resolve absolute `grok_home` (existing).
2. Read `auth.json`; if missing/invalid/no key → unauthenticated.
3. GET `GROK_BILLING_URL` with Bearer key.
4. Status 401/403 → unauthenticated.
5. Status non-success → provider_error or network_error (retryable for 5xx/timeout).
6. Success body → `grok_from_billing_json`.
7. Do not call signals walk for windows.

- [ ] **Step 1: Write failing adapter test with ScriptedHttpClient**

Pattern from Claude/Grok existing adapter tests in `adapters.rs`:

```rust
#[tokio::test]
async fn grok_ready_from_billing_http() {
    // home with auth.json (synthetic key string, not a real JWT)
    // ScriptedHttpClient returns 200 + billing-weekly.json body
    // assert last_url == GROK_BILLING_URL
    // assert Authorization header present with Bearer prefix
    // assert Ready weekly window; no context
}
```

Also:

- `grok_billing_401_unauthenticated`
- `grok_billing_timeout_network_error` (if ScriptedHttpClient can return `HttpError::Network`)

Update/remove `grok_ready_from_auth_and_signals_fixture` that expects `windows[0].id() == "context"`.

- [ ] **Step 2: Run — expect FAIL**

```bash
cargo test grok_ready_from_billing -- --nocapture
```

- [ ] **Step 3: Implement adapter + catalog retry**

```rust
// catalog.rs
retry_policy: RetryPolicy::OneTransient, // was None for GROK

// adapters.rs GrokAdapter::collect
let token = /* from auth */;
let headers = [
    ("Authorization", bearer.as_str()),
    ("x-grok-client-mode", "cli"),
];
match context.http.get(GROK_BILLING_URL, &headers, GROK.max_output_bytes).await {
    Ok(resp) if resp.status == 200 => grok_from_billing_json(&resp.body, account, now, login_available(discovery)),
    Ok(resp) if resp.status == 401 || resp.status == 403 => unauthenticated(...),
    Ok(_) => ProviderResult::ProviderError { retryable: true, ... },
    Err(HttpError::Network(_)) => { /* optional one retry */ ProviderResult::NetworkError { ... } },
    Err(_) => ProviderResult::NetworkError { ... },
}
```

Never put `token` into error messages.

- [ ] **Step 4: Run suite**

```bash
cargo test providers:: -- --nocapture
cargo test grok_ -- --nocapture
```

Expected: PASS; no test asserts Grok `context` as the ready primary.

- [ ] **Step 5: Commit**

```bash
git add src/providers/adapters.rs src/providers/catalog.rs src/providers/v2_map.rs src/providers/mod.rs
git commit -m "feat: Grok weekly via billing HTTP"
```

---

### Task 6: Docs + active contract alignment

**Files:**
- Modify: `docs/specs/v10/02-target-architecture.md` (Grok collection row)
- Modify: `docs/troubleshooting.md` (Codex rate limits / Grok weekly)
- Modify if needed: `README.md`, `PRODUCT.md`, `docs/architecture.md`, `docs/new-provider.md`
- Modify: any active-doc test that freezes the old Grok “context” string (`tests/active_docs.rs` if present)

**Grok table row target text:**

```text
| `grok` | `$GROK_HOME/auth.json`, then authenticated GET
  `https://cli-chat-proxy.grok.com/v1/billing?format=credits`
  (literal; headers Authorization Bearer + x-grok-client-mode)
  | `weekly` | 90 s | 10 s | one network/timeout retry |
```

Labels line: Grok `Weekly` (not Context).

Troubleshooting bullets:

- Codex Retry loops with “rate limits not available” → ensure CLI logged in; agent-bar uses app-server then session logs under `~/.codex/sessions`; not `rate-limits.json` alone.
- Grok shows `—` when billing returns no percent; Weekly reset comes from billing period end; Context is no longer a product window.

- [ ] **Step 1: Update docs**

Apply the table/label/troubleshooting edits. Grep active docs for “Context” + Grok product claims:

```bash
rg -n "Grok.*[Cc]ontext|context de sessão|Context\`" README.md PRODUCT.md docs/architecture.md docs/new-provider.md docs/specs/v10 docs/troubleshooting.md
```

- [ ] **Step 2: Run active doc tests**

```bash
cargo test --test active_docs -- --nocapture
cargo test --test active_legacy_scan -- --nocapture
```

Expected: PASS (or update freezes if they intentionally locked old Grok copy).

- [ ] **Step 3: Commit**

```bash
git add docs README.md PRODUCT.md tests
git commit -m "docs: Codex collection and Grok weekly"
```

---

### Task 7: Full verification gate

**Files:** none (verification only)

- [ ] **Step 1: Format / test / clippy / diff**

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
git diff --check
```

Expected: all green.

- [ ] **Step 2: Optional authorized live smoke (host with CLIs)**

```bash
PLUGIN="$HOME/.config/omarchy/plugins/agent-bar.usage/bin/agent-bar"
# After local `cargo build` install/replace only if user authorized plugin write.
./target/debug/agent-bar status provider codex format human cache bypass
./target/debug/agent-bar status provider grok format human cache bypass
```

Expected:

- Codex: `ready` with Session and/or Weekly **or** a non-stub typed state after real collection attempt (not instant stub-only without reading sessions).
- Grok: `Weekly` used/remaining + reset; label not `Context`.

Sanitize any logs before pasting (no tokens).

- [ ] **Step 3: Final commit only if smoke required code tweaks; otherwise stop**

Do not push/merge unless explicitly authorized.

---

## Self-review (plan vs spec)

| Spec requirement | Task |
| --- | --- |
| Codex app-server `rateLimits/read` | Task 3 |
| Codex session-log fallback + bounds | Task 2 |
| Duration-based Session/Weekly labels | Task 1 |
| Optional rate-limits.json | Task 2/3 order |
| Retry meaningful with local data | Task 2 alone already; Task 3 improves live |
| Grok weekly from billing HTTPS | Tasks 4–5 |
| No Context window | Tasks 4–5 |
| Discard money fields | Tasks 1, 4 |
| Catalog/docs update Grok | Tasks 5–6 |
| Privacy / no token logs | Tasks 3, 5 notes |
| Verification gate | Task 7 |

No TBD placeholders remain. URL/header literals are pinned with an explicit “update constant if probe differs” rule inside Task 5 (not an open product decision).

---

## Execution handoff

Plan complete and saved to `docs/superpowers/plans/2026-07-27-codex-retry-grok-weekly.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks  
2. **Inline Execution** — same session with executing-plans checkpoints  

Which approach?
