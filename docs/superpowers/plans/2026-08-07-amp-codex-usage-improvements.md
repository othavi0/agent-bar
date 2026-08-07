# Amp/Codex Usage Collection Improvements — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Collect the Amp subscription windows and the new Codex app-server fields the adapters currently drop, fix inherited F4 hygiene, and surface Codex rate-limit resets in the popup — delivered as two sequential PRs.

**Architecture:** All parsing stays in the pure mappers (`src/providers/v2_map.rs`); the Codex app-server bridge (`codex_app_server.rs`) keeps normalizing wire JSON into the mapper's input shape. One new optional field (`rateLimitResetsAvailable`) flows domain → schema v2 → QML. Spec: `docs/superpowers/specs/2026-08-07-amp-codex-usage-improvements-design.md`.

**Tech Stack:** Rust (serde, regex, time, tokio), QML/Quickshell (Omarchy Quattro), JSON Schema.

## Global Constraints

- No production `unwrap()`/`expect()` (repo hard rule).
- No monetary data in domain results: `assert_no_money` bans the substrings `spend`, `credits`, `balance`, `currency`, `usd`, `BRL` in `Debug` output — new ids/labels/fields must avoid them (hence `rate_limit_resets_available`, `workspace-limit`).
- Window labels are English with stable ids (`JSON-020`).
- Commit subjects: English Conventional Commits, ≤50 chars.
- Checkpoint battery after every task group: `cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings && git diff --check`.
- QML battery (PR2 only, after QML tasks): Qt6 qmllint + `omarchy plugin validate assets/omarchy` + Qt6 qmltestrunner exactly as written in `CLAUDE.md` (PATH binaries are silent traps).
- Never push/publish without owner authorization; PR creation happens at the explicit checkpoint steps only.
- PR1 = Tasks 1–4 on branch `feat/amp-codex-usage-improvements` (already holds the spec commit). PR2 = Tasks 5–11 on a branch created from `master` **after PR1 merges** (stacked-PR merge gotcha: do not stack).

---

## PR1 — Amp subscription + F4 hygiene

### Task 1: Amp subscription windows + plan

**Files:**
- Create: `tests/fixtures/amp/usage-subscription-pct.txt`
- Modify: `src/providers/v2_map.rs:30-95` (`amp_from_usage_text`) and its `#[cfg(test)]` module
- Modify: `src/providers/v2_map.rs:20-24` (label consts)

**Interfaces:**
- Produces: `amp_from_usage_text` now emits up to 3 windows (`daily`, `plan-other`, `plan-orb`) and `plan: Option<Plan>`; consumed unchanged by `AmpAdapter`.
- Produces: `pub(crate) fn format_plan_label(raw: &str) -> String` (used by Task 3 for Grok and by PR2 Task 9 for Codex).

- [ ] **Step 1: Create the fixture** (values chosen non-uniform so swapped captures fail):

```text
Signed in as user@email.com (nick)
Amp Free: 97% remaining today (resets daily) - https://ampcode.com/settings#amp-free
Subscription Megawatt: 92% other usage and 100% orb usage remaining
Individual credits: $4.19 remaining (replenishes automatically) - https://ampcode.com/settings
```

- [ ] **Step 2: Write the failing tests** in the `tests` module of `v2_map.rs`:

```rust
#[test]
fn amp_subscription_fixture_emits_plan_windows_and_plan() {
    let fixture = include_str!("../../tests/fixtures/amp/usage-subscription-pct.txt");
    let result = amp_from_usage_text(fixture, datetime!(2026-08-07 12:00:00 UTC));
    assert_no_money(&result);
    match result {
        ProviderResult::Ready { windows, plan, .. } => {
            let ids: Vec<&str> = windows.iter().map(|w| w.id()).collect();
            assert_eq!(ids, vec!["daily", "plan-other", "plan-orb"]);
            assert_eq!(windows[1].label(), "Plan · other");
            assert!((windows[1].remaining_percent() - 92.0).abs() < 0.01);
            assert!((windows[1].used_percent() - 8.0).abs() < 0.01);
            assert_eq!(windows[2].label(), "Plan · orb");
            assert!((windows[2].remaining_percent() - 100.0).abs() < 0.01);
            // Amp exposes no subscription reset timestamp ("monthly" only).
            assert!(windows[1].resets_at().is_none());
            assert!(windows[2].resets_at().is_none());
            let plan = plan.expect("plan from Subscription line");
            assert_eq!(plan.id, "megawatt");
            assert_eq!(plan.label, "Megawatt");
        }
        other => panic!("expected ready, got {other:?}"),
    }
}

#[test]
fn amp_free_only_fixture_still_has_no_plan() {
    let fixture = include_str!("../../tests/fixtures/amp/usage-free-pct.txt");
    let result = amp_from_usage_text(fixture, datetime!(2026-08-07 12:00:00 UTC));
    match result {
        ProviderResult::Ready { windows, plan, .. } => {
            assert_eq!(windows.len(), 1);
            assert!(plan.is_none());
        }
        other => panic!("expected ready, got {other:?}"),
    }
}

#[test]
fn amp_individual_credits_line_never_emits_window() {
    // Only the monetary line plus account: Ready with zero windows, no money.
    let text = "Signed in as user@email.com (nick)\nIndividual credits: $4.19 remaining (replenishes automatically)\n";
    let result = amp_from_usage_text(text, datetime!(2026-08-07 12:00:00 UTC));
    assert_no_money(&result);
    match result {
        ProviderResult::Ready { windows, plan, .. } => {
            assert!(windows.is_empty());
            assert!(plan.is_none());
        }
        other => panic!("expected ready, got {other:?}"),
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test amp_ -- --nocapture`
Expected: FAIL — `amp_subscription_fixture_emits_plan_windows_and_plan` (ids `["daily"]`, plan `None`). The other two pass already (guard tests).

- [ ] **Step 4: Implement** in `amp_from_usage_text`, after the `daily` window block (before `ProviderResult::Ready`):

```rust
// Subscription line (2026-07-18 Amp subscriptions): two percentage buckets.
// "orb usage" = included orb-hours allowance; "other usage" = included agent
// usage. The plan name doubles as the Plan badge. The "Individual credits: $"
// line is monetary and intentionally never parsed into a window (JSON-022B).
let mut plan = None;
if let Some(caps) = Regex::new(
    r"Subscription\s+(\S+):\s*([0-9.]+)%\s*other usage and\s*([0-9.]+)%\s*orb usage remaining",
)
.ok()
.and_then(|re| re.captures(&text))
{
    if let Some(name) = caps.get(1).map(|m| m.as_str()) {
        plan = Some(Plan {
            id: name.to_ascii_lowercase(),
            label: name.to_owned(),
        });
    }
    for (idx, id, label) in [(2usize, "plan-other", "Plan · other"), (3usize, "plan-orb", "Plan · orb")] {
        if let Some(rem) = caps.get(idx).and_then(|m| m.as_str().parse::<f64>().ok()) {
            let rem = rem.clamp(0.0, 100.0);
            let used = (100.0 - rem).clamp(0.0, 100.0);
            // No resets_at: Amp documents only "replenishes at the end of
            // each monthly period", with no timestamp exposed.
            if let Ok(w) = UsageWindow::try_new(id, label, used, rem, None) {
                windows.push(w);
            }
        }
    }
}
```

and change the `Ready` construction from `plan: None` to `plan`. Add the shared helper (module scope, near `sanitize_account_label`):

```rust
/// Title-case a raw plan/tier id for display: "pro" → "Pro",
/// "self_serve_business_usage_based" → "Self Serve Business Usage Based".
pub(crate) fn format_plan_label(raw: &str) -> String {
    raw.split('_')
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
```

(Amp keeps the label exactly as printed by the CLI; `format_plan_label` is for Grok/Codex raw tiers in later tasks.)

- [ ] **Step 5: Run tests to verify pass**

Run: `cargo test amp_ && cargo test assert_no_money`
Expected: PASS, including `assert_no_money` on the new fixture result.

- [ ] **Step 6: Commit**

```bash
git add tests/fixtures/amp/usage-subscription-pct.txt src/providers/v2_map.rs
git commit -m "feat: parse amp subscription usage windows"
```

### Task 2: Amp failure classifier — explicit markers only

**Files:**
- Modify: `src/providers/adapters.rs:53-64` (Amp non-zero-exit branch)
- Test: same file's `#[cfg(test)]` module (search for existing Amp adapter tests near `amp` to co-locate)

**Interfaces:**
- Consumes: existing `unauthenticated(...)` helper and `ProviderResult` variants — signatures unchanged.

- [ ] **Step 1: Write the failing test** (uses the existing fake process/test harness pattern in the adapters test module — mirror the neighboring Amp test's setup that injects a `ProcessSpec` result):

```rust
#[test]
fn amp_network_flavored_auth_substring_is_not_unauthenticated() {
    // "authorization server unavailable" contains "auth" but is operational.
    let out = fake_process_output(1, "", "authorization server unavailable");
    let result = classify_amp_failure(&out, /* login_available */ true);
    assert!(matches!(result, ProviderResult::ProviderError { .. }));
}

#[test]
fn amp_not_signed_in_is_unauthenticated() {
    let out = fake_process_output(1, "You are not signed in. Run amp login.", "");
    let result = classify_amp_failure(&out, true);
    assert!(matches!(result, ProviderResult::Unauthenticated { .. }));
}
```

If no `fake_process_output`/classifier seam exists yet, extract the branch into a testable function first (that extraction is this task's refactor):

```rust
/// Classify a non-zero `amp usage` exit. Unauthenticated requires an explicit
/// marker; a bare "auth" substring (e.g. "authorization server unavailable")
/// is an operational failure, not a login problem.
fn classify_amp_failure(out: &ProcessOutput, login_available: bool) -> ProviderResult {
    let stdout = out.stdout.to_ascii_lowercase();
    let stderr = out.stderr.to_ascii_lowercase();
    let explicit = ["not signed", "sign in", "unauthorized", "please log in"];
    if explicit.iter().any(|m| stdout.contains(m) || stderr.contains(m)) {
        return unauthenticated(
            ProviderId::Amp,
            AMP.display_name,
            "Amp is not authenticated.",
            login_available,
            AMP.installation_url,
            false,
        );
    }
    ProviderResult::ProviderError {
        id: ProviderId::Amp,
        name: AMP.display_name.to_owned(),
        message: "Amp usage command failed.".into(),
        retryable: false,
    }
}
```

The `collect` branch at `adapters.rs:53-64` becomes `classify_amp_failure(&out, login_available(discovery))`.

- [ ] **Step 2: Run to verify failure** — `cargo test amp_network_flavored` → FAIL (today it returns `Unauthenticated`).
- [ ] **Step 3: Implement** the extraction + marker list above.
- [ ] **Step 4: Run** `cargo test amp_` → PASS.
- [ ] **Step 5: Commit** — `git commit -m "fix: amp auth classifier needs explicit marker"`

### Task 3: Grok period type + formatted plan label

**Files:**
- Modify: `src/providers/v2_map.rs:109-220` (`GrokBillingDoc`, `GrokPeriodRaw`, `grok_from_billing_json`)
- Test: same file's tests module

**Interfaces:**
- Consumes: `format_plan_label` from Task 1.
- Produces: Grok window id/label derived from `currentPeriod.type`; unchanged call signature.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn grok_monthly_period_type_names_the_window() {
    let json = br#"{"creditUsagePercent": 20.0,
        "currentPeriod": {"type": "USAGE_PERIOD_TYPE_MONTHLY", "end": "2026-09-01T00:00:00Z"},
        "subscriptionTiers": "pro"}"#;
    let result = grok_from_billing_json(json, None, datetime!(2026-08-07 12:00:00 UTC), true);
    match result {
        ProviderResult::Ready { windows, plan, .. } => {
            assert_eq!(windows[0].id(), "monthly");
            assert_eq!(windows[0].label(), "Monthly");
            assert_eq!(plan.as_ref().map(|p| p.label.as_str()), Some("Pro"));
        }
        other => panic!("expected ready, got {other:?}"),
    }
}

#[test]
fn grok_missing_period_type_stays_weekly() {
    let json = br#"{"creditUsagePercent": 10.0}"#;
    let result = grok_from_billing_json(json, None, datetime!(2026-08-07 12:00:00 UTC), true);
    match result {
        ProviderResult::Ready { windows, .. } => {
            assert_eq!(windows[0].id(), "weekly");
            assert_eq!(windows[0].label(), "Weekly (7d)");
        }
        other => panic!("expected ready, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test grok_monthly` → FAIL (id is hardcoded `weekly`; plan label is raw `pro`).
- [ ] **Step 3: Implement**: add `#[serde(default, rename = "type")] period_type: Option<String>` to `GrokPeriodRaw`; derive identity before building the window:

```rust
/// USAGE_PERIOD_TYPE_WEEKLY → ("weekly", "Weekly (7d)"); other known-shaped
/// values strip the prefix (lowercase id, title-case label); absent/foreign
/// values keep the historical weekly identity.
fn grok_window_identity(period_type: Option<&str>) -> (String, String) {
    match period_type {
        Some("USAGE_PERIOD_TYPE_WEEKLY") | None => ("weekly".into(), LABEL_WEEKLY.into()),
        Some(other) => match other.strip_prefix("USAGE_PERIOD_TYPE_") {
            Some(rest) if !rest.is_empty() => {
                (rest.to_ascii_lowercase(), format_plan_label(&rest.to_ascii_lowercase()))
            }
            _ => ("weekly".into(), LABEL_WEEKLY.into()),
        },
    }
}
```

Use it in `grok_from_billing_json` (`UsageWindow::try_new(&id, &label, …)`), and change the plan construction to `Plan { label: format_plan_label(&id), id }`.

- [ ] **Step 4: Run** `cargo test grok_` → PASS (existing fixture tests must still pass: the checked-in fixture uses `USAGE_PERIOD_TYPE_WEEKLY`).
- [ ] **Step 5: Commit** — `git commit -m "fix: grok window id follows period type"`

### Task 4: PR1 checkpoint and pull request

**Files:** none new.

- [ ] **Step 1: Full battery** — `cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings && git diff --check` → all green.
- [ ] **Step 2: Review diff** for secrets/legacy leakage/unrelated changes (`git diff master...HEAD`).
- [ ] **Step 3: With owner confirmation**, push and open the PR:

```bash
git push -u origin feat/amp-codex-usage-improvements
gh pr create --title "feat: amp subscription windows + F4 hygiene" --body-file - <<'EOF'
## Summary
- Parse the Amp subscription line (post 2026-07-18 plans): windows `plan-other`/`plan-orb` + Plan badge
- Recognize and intentionally discard the monetary "Individual credits" line
- Amp failure classifier requires explicit auth markers (no bare "auth" substring)
- Grok window identity follows `currentPeriod.type`; Grok/shared plan labels formatted

Spec: docs/superpowers/specs/2026-08-07-amp-codex-usage-improvements-design.md
EOF
```

(Re-read the body before creating: no AI attribution anywhere.)

---

## PR2 — Codex fields, multi-bucket, contract amendment, reset line

Branch: after PR1 merges, `git checkout master && git pull && git checkout -b feat/codex-rate-limit-fields`.

### Task 5: Declare the full app-server payload and pass it through

**Files:**
- Modify: `src/providers/codex_app_server.rs:28-58` (wire structs) and `:85-140` (`window_to_json`, `normalize_to_rate_limits_json`)
- Modify: `src/providers/v2_map.rs:278-345` (`CodexWindowRaw`/`CodexRateLimitsDoc`/`codex_from_rate_limits_json`)
- Test: both files' test modules

**Interfaces:**
- Produces: normalized JSON handed to `codex_from_rate_limits_json` gains optional keys `individualLimit` (`{"remainingPercent": f64, "resetsAt": i64}`), `extraBuckets` (`[{"limitId": String, "primary": {...}, "secondary": {...}}]`), `rateLimitResetsAvailable` (u32). File/session-log payloads simply omit them (serde defaults).
- Produces: `CodexRateLimitsDoc` fields `individual_limit: Option<CodexIndividualLimitRaw>`, `extra_buckets: Vec<CodexExtraBucketRaw>`, `rate_limit_resets_available: Option<u32>`; `ProviderResult` unchanged until Task 7.

- [ ] **Step 1: Write the failing test** in `codex_app_server.rs` tests (normalization is a pure function):

```rust
#[test]
fn normalize_passes_individual_limit_and_reset_count_through() {
    let raw: CodexAppServerRateLimitsReadResult = serde_json::from_value(serde_json::json!({
        "rateLimits": {
            "limitId": "codex",
            "primary": {"usedPercent": 12.0, "windowDurationMins": 10080, "resetsAt": 1791000000},
            "secondary": null,
            "individualLimit": {"remainingPercent": 40.0, "resetsAt": 1791000000},
            "planType": "plus"
        },
        "rateLimitResetCredits": {"availableCount": 2, "credits": []}
    })).expect("wire parse");
    let bytes = normalize_to_rate_limits_json(&raw, Some("plus")).expect("normalized");
    let doc: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(doc["individualLimit"]["remainingPercent"], 40.0);
    assert_eq!(doc["rateLimitResetsAvailable"], 2);
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test normalize_passes_individual` → FAIL (fields don't exist on the wire structs).
- [ ] **Step 3: Implement.** Extend the wire structs (all `#[serde(default)]`, camelCase):

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexAppServerIndividualLimit {
    #[serde(default)]
    remaining_percent: Option<f64>,
    #[serde(default)]
    resets_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexAppServerResetCredits {
    #[serde(default)]
    available_count: Option<u32>,
}
```

`CodexAppServerLimitBucket` gains `#[serde(default)] individual_limit: Option<CodexAppServerIndividualLimit>`; `CodexAppServerRateLimitsReadResult` gains `#[serde(default)] rate_limit_reset_credits: Option<CodexAppServerResetCredits>`. The monetary `credits {hasCredits, unlimited, balance}` object is deliberately **not** declared (serde ignores it); add the comment `// credits{balance,...} is monetary and intentionally undeclared (JSON-022B).` on the bucket struct. `normalize_to_rate_limits_json` inserts, when present:

```rust
if let Some(il) = root.and_then(|r| r.individual_limit.as_ref()) {
    if let Some(rem) = il.remaining_percent {
        doc.insert("individualLimit".into(), serde_json::json!({
            "remainingPercent": rem,
            "resetsAt": il.resets_at.unwrap_or(0),
        }));
    }
}
if let Some(n) = raw.rate_limit_reset_credits.as_ref().and_then(|c| c.available_count) {
    doc.insert("rateLimitResetsAvailable".into(), serde_json::json!(n));
}
```

- [ ] **Step 4: Run** `cargo test -p agent-bar codex` (or `cargo test codex`) → PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat: declare full codex rate-limit payload"`

### Task 6: Multi-bucket iteration with explicit `codex` preference

**Files:**
- Modify: `src/providers/codex_app_server.rs:95-140` (`normalize_to_rate_limits_json`)
- Test: same file's test module

**Interfaces:**
- Produces: normalized JSON `extraBuckets` array; preferred bucket selection order: `rateLimits` root → `rateLimitsByLimitId["codex"]` → first map entry **with window data**. Every other data-carrying bucket lands in `extraBuckets`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn normalize_prefers_codex_bucket_over_alphabetical() {
    let raw: CodexAppServerRateLimitsReadResult = serde_json::from_value(serde_json::json!({
        "rateLimitsByLimitId": {
            "alpha": {"primary": {"usedPercent": 50.0, "windowDurationMins": 300, "resetsAt": 0}},
            "codex": {"primary": {"usedPercent": 12.0, "windowDurationMins": 10080, "resetsAt": 0}}
        }
    })).expect("wire parse");
    let bytes = normalize_to_rate_limits_json(&raw, None).expect("normalized");
    let doc: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(doc["primary"]["usedPercent"], 12.0, "codex bucket must win");
    assert_eq!(doc["extraBuckets"][0]["limitId"], "alpha");
    assert_eq!(doc["extraBuckets"][0]["primary"]["usedPercent"], 50.0);
}

#[test]
fn normalize_skips_null_window_buckets_like_premium() {
    let raw: CodexAppServerRateLimitsReadResult = serde_json::from_value(serde_json::json!({
        "rateLimitsByLimitId": {
            "codex": {"primary": {"usedPercent": 90.0, "windowDurationMins": 10080, "resetsAt": 0}},
            "premium": {"primary": null, "secondary": null}
        }
    })).expect("wire parse");
    let bytes = normalize_to_rate_limits_json(&raw, None).expect("normalized");
    let doc: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(doc["primary"]["usedPercent"], 90.0);
    assert!(doc.get("extraBuckets").is_none(), "empty premium bucket must not appear");
}
```

- [ ] **Step 2: Run to verify failure** — first test FAILS today (`alpha` wins by BTreeMap order and no `extraBuckets` exists).
- [ ] **Step 3: Implement** — replace the fallback loop (`codex_app_server.rs:103-117`):

```rust
// Preferred bucket: explicit `codex` key first (mirrors the upstream
// backend's own preference), then any bucket that actually carries windows.
// Every other data-carrying bucket is preserved as an extra bucket instead
// of being silently dropped.
let mut extra = Vec::new();
if primary.is_none() && secondary.is_none() {
    if let Some(by_id) = raw.rate_limits_by_limit_id.as_ref() {
        let preferred_key = if by_id.get("codex").is_some_and(has_window_data) {
            Some("codex".to_string())
        } else {
            by_id
                .iter()
                .find(|(_, b)| has_window_data(b))
                .map(|(k, _)| k.clone())
        };
        if let Some(key) = preferred_key {
            let bucket = &by_id[&key];
            primary = bucket.primary.as_ref();
            secondary = bucket.secondary.as_ref();
            for (k, b) in by_id.iter() {
                if *k != key && has_window_data(b) {
                    extra.push((k.clone(), b));
                }
            }
        }
    }
}
```

with the helper `fn has_window_data(b: &CodexAppServerLimitBucket) -> bool { b.primary.is_some() || b.secondary.is_some() }`, and after the primary/secondary insertion serialize `extra` as `extraBuckets: [{"limitId", "primary"?, "secondary"?}]` (reusing `window_to_json`, fallbacks 300/10080). Skip the key entirely when `extra` is empty.

- [ ] **Step 4: Run** `cargo test normalize_` → PASS.
- [ ] **Step 5: Commit** — `git commit -m "feat: iterate codex multi-bucket rate limits"`

### Task 7: Domain field `rate_limit_resets_available` + mapper windows

**Files:**
- Modify: `src/status/schema.rs:322-336` (`ProviderStatus`) and the `ProviderResult::Ready` variant + every construction/conversion site the compiler flags (known: `v2_map.rs` amp/grok×2/codex/claude mappers; stale-retention rebuilds in `schema.rs` around lines 380–500; `status/coordinator.rs` clones)
- Modify: `src/providers/v2_map.rs:278-380` (Codex doc + windows)
- Modify: `schemas/status-v2.schema.json` (`providerBase.properties`)
- Modify: `docs/specs/v10/03-cli-and-json-contract.md:185` area + `docs/specs/v10/REQUIREMENTS_MATRIX.md:63`
- Test: `v2_map.rs` + schema round-trip tests near `ProviderStatus`

**Interfaces:**
- Produces: `ProviderResult::Ready { …, rate_limit_resets_available: Option<u32> }`; `ProviderStatus` serializes optional `rateLimitResetsAvailable` (omitted when `None`); consumed by Task 8 (QML).
- Contract: new invariant `JSON-022C` (below).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn codex_maps_extra_buckets_individual_limit_and_resets() {
    let json = serde_json::json!({
        "primary": {"usedPercent": 36.0, "windowDurationMins": 10080, "resetsAt": 1791000000},
        "plan_type": "plus",
        "individualLimit": {"remainingPercent": 40.0, "resetsAt": 1791000000},
        "extraBuckets": [{"limitId": "premium",
            "primary": {"usedPercent": 10.0, "windowDurationMins": 10080, "resetsAt": 0}}],
        "rateLimitResetsAvailable": 2
    });
    let bytes = serde_json::to_vec(&json).expect("fixture json");
    let result = codex_from_rate_limits_json(&bytes, datetime!(2026-08-07 12:00:00 UTC));
    assert_no_money(&result);
    match result {
        ProviderResult::Ready { windows, plan, rate_limit_resets_available, .. } => {
            let ids: Vec<&str> = windows.iter().map(|w| w.id()).collect();
            assert_eq!(ids, vec!["weekly", "codex:premium", "workspace-limit"]);
            assert_eq!(windows[1].label(), "Premium (7d)");
            assert_eq!(windows[2].label(), "Workspace limit");
            assert!((windows[2].remaining_percent() - 40.0).abs() < 0.01);
            assert_eq!(rate_limit_resets_available, Some(2));
            assert_eq!(plan.as_ref().map(|p| p.label.as_str()), Some("Plus"));
        }
        other => panic!("expected ready, got {other:?}"),
    }
}

#[test]
fn provider_status_serializes_reset_count_only_when_present() {
    // ProviderStatus derives Deserialize: build both rows via serde and
    // assert the key round-trips only when present.
    let base = serde_json::json!({
        "id": "codex", "name": "Codex", "state": "ready", "source": "live",
        "plan": null, "account": null, "windows": [],
        "lastSuccessAt": "2026-08-07T12:00:00Z", "error": null, "action": null
    });
    let mut with = base.clone();
    with["rateLimitResetsAvailable"] = serde_json::json!(2);
    let with: ProviderStatus = serde_json::from_value(with).expect("row with resets");
    let text = serde_json::to_string(&with).expect("serialize");
    assert!(text.contains("\"rateLimitResetsAvailable\":2"));
    let without: ProviderStatus = serde_json::from_value(base).expect("row without resets");
    let text = serde_json::to_string(&without).expect("serialize");
    assert!(!text.contains("rateLimitResetsAvailable"));
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test codex_maps_extra` → compile FAIL (`rate_limit_resets_available` unknown).
- [ ] **Step 3: Implement domain plumbing.**
  - `ProviderResult::Ready` gains `rate_limit_resets_available: Option<u32>`.
  - Compiler-driven sweep: every non-Codex `Ready { … }` construction sets `rate_limit_resets_available: None` (amp, grok billing, grok legacy, claude, plus any coordinator/cache rebuild the compiler flags — stale retention must **carry the previous value through**, mirroring how `plan`/`account` are retained).
  - `ProviderStatus` gains `#[serde(default, skip_serializing_if = "Option::is_none")] rate_limit_resets_available: Option<u32>` and the `ProviderResult → ProviderStatus` conversions copy it.
  - `CodexRateLimitsDoc` gains:

```rust
#[serde(default, rename = "individualLimit")]
individual_limit: Option<CodexIndividualLimitRaw>,
#[serde(default, rename = "extraBuckets")]
extra_buckets: Vec<CodexExtraBucketRaw>,
#[serde(default, rename = "rateLimitResetsAvailable")]
rate_limit_resets_available: Option<u32>,
```

```rust
#[derive(Debug, Deserialize)]
struct CodexIndividualLimitRaw {
    #[serde(rename = "remainingPercent")]
    remaining_percent: f64,
    #[serde(default, rename = "resetsAt")]
    resets_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CodexExtraBucketRaw {
    #[serde(rename = "limitId")]
    limit_id: String,
    #[serde(default)]
    primary: Option<CodexWindowRaw>,
    #[serde(default)]
    secondary: Option<CodexWindowRaw>,
}
```

  - In `codex_from_rate_limits_json`, after the primary/secondary loop: for each extra bucket emit its `primary` window with id `format!("codex:{sanitized}")` (sanitize like `weekly_model_id` does: lowercase ASCII alphanumerics/hyphens) and, when a `secondary` also exists, a second window with id `format!("codex:{sanitized}:2")`. Labels: `format!("{} (7d)", format_plan_label(&sanitized))` for 10080-minute windows, else `format!("{} ({}m)", …, mins)`; then the workspace window:

```rust
if let Some(il) = doc.individual_limit.as_ref() {
    if il.remaining_percent.is_finite() {
        let rem = il.remaining_percent.clamp(0.0, 100.0);
        let used = (100.0 - rem).clamp(0.0, 100.0);
        let resets = il
            .resets_at
            .filter(|&ts| ts > 0)
            .and_then(|ts| OffsetDateTime::from_unix_timestamp(ts).ok())
            .map(|ts| ts.to_offset(UtcOffset::UTC));
        if let Ok(w) = UsageWindow::try_new("workspace-limit", "Workspace limit", used, rem, resets) {
            windows.push(w);
        }
    }
}
```

  - Plan: `Plan { label: format_plan_label(&id), id }`.
- [ ] **Step 4: JSON Schema + contract.** In `schemas/status-v2.schema.json`, `providerBase.properties` gains (NOT added to `required` — optional by design):

```json
"rateLimitResetsAvailable": { "type": "integer", "minimum": 0 }
```

  In `03-cli-and-json-contract.md`, immediately after `JSON-022B`:

```markdown
- `JSON-022C`: `rateLimitResetsAvailable`, when present, is a non-negative
  integer count of provider-granted rate-limit resets. It is a quota-reset
  count, not a monetary fact: it never carries balance, price, or currency,
  and `JSON-022B` continues to ban those.
```

  Update `REQUIREMENTS_MATRIX.md:63` row to `JSON-022A`–`JSON-022C`.
- [ ] **Step 5: Run** `cargo test` (full — the semantic validator and schema fixtures must stay green; fix any envelope fixture the validator flags by leaving the field absent).
- [ ] **Step 6: Commit** — `git commit -m "feat: surface codex rate-limit reset count"`

### Task 8: QML reset line

**Files:**
- Modify: `assets/omarchy/CoreView.js` (new function near `headerModel`, ~line 677)
- Modify: `assets/omarchy/ProviderView.qml` (after the windows `Column`, before `StateMessage`, ~line 209)
- Test: `tests/qml/tst_ProviderStates.qml`

**Interfaces:**
- Consumes: `provider.rateLimitResetsAvailable` (integer, may be `undefined`).
- Produces: `Core.rateLimitResetsText(provider)` → `""` or `"↻ N rate-limit resets available"`.

- [ ] **Step 1: Write the failing QML test** (follow the file's existing pattern of building provider objects and instantiating `ProviderView`):

```qml
function test_reset_line_visible_only_when_positive() {
    var withResets = makeReadyProvider({ rateLimitResetsAvailable: 2 })
    compare(CoreView.rateLimitResetsText(withResets), "↻ 2 rate-limit resets available")
    var without = makeReadyProvider({})
    compare(CoreView.rateLimitResetsText(without), "")
    var zero = makeReadyProvider({ rateLimitResetsAvailable: 0 })
    compare(CoreView.rateLimitResetsText(zero), "")
}
```

(`makeReadyProvider` = whatever ready-provider builder the file already uses; do not import `qs.Commons` in tests — Qt6 runner cannot resolve it.)

- [ ] **Step 2: Run to verify failure** — Qt6 qmltestrunner command from `CLAUDE.md`, expect the new function missing.
- [ ] **Step 3: Implement.** `CoreView.js`:

```js
// Codex rate-limit reset count (JSON-022C). Singular/plural, empty when
// absent or zero so the popup stays byte-identical for every other provider.
function rateLimitResetsText(provider) {
  if (!provider)
    return ""
  var n = Number(provider.rateLimitResetsAvailable)
  if (!isFinite(n) || n <= 0)
    return ""
  n = Math.floor(n)
  return "↻ " + n + " rate-limit reset" + (n === 1 ? "" : "s") + " available"
}
```

`ProviderView.qml`, inside the windows-mode `Column` (so it only renders alongside windows), after the compact-rows `Column`:

```qml
Text {
  width: parent.width
  visible: text.length > 0
  text: Core.rateLimitResetsText(root.provider)
  color: Util.alpha(root.foreground, 0.72)
  font.family: root.fontFamily
  font.pixelSize: Style.font.caption
  textFormat: Text.PlainText
  Accessible.name: text
}
```

- [ ] **Step 4: Run the full QML battery** (Qt6 qmllint on all plugin QML, `omarchy plugin validate assets/omarchy`, Qt6 qmltestrunner with both env vars) → PASS; read qmllint output only for newly introduced warnings.
- [ ] **Step 5: Commit** — `git commit -m "feat: popup line for codex reset count"`

### Task 9: Remove dead cascade stage 2 + honest session-log freshness

**Files:**
- Modify: `src/providers/adapters.rs:296-305` (Codex cascade)
- Modify: `src/providers/codex_session_log.rs` (`find_latest_rate_limits` return type)
- Test: both files' test modules

**Interfaces:**
- Produces: `find_latest_rate_limits(dir: &Path) -> Option<(Vec<u8>, Option<OffsetDateTime>)>` — second element is the log event's own timestamp; the adapter passes it (when present) as the `now` argument of `codex_from_rate_limits_json` so `lastSuccessAt` records data generation (`JSON-021`), not collection time.

- [ ] **Step 1: Write the failing tests**
  - `codex_session_log.rs`: a fixture line carrying `"timestamp":"2026-07-28T10:00:00Z"` → returned timestamp parses to that instant; a line without timestamp → `None`.
  - `adapters.rs`: with app-server unavailable and only a session log present, the resulting `Ready.last_success_at` equals the log timestamp, not the fake clock's now.
- [ ] **Step 2: Run to verify failure** — return-type change means compile-fail first; that counts.
- [ ] **Step 3: Implement.** Delete the stage-2 block (`adapters.rs:296-300`) and its `rate-limits.json` path; renumber comments (stages: app-server → session-log → typed miss). Extend the session-log parser to also extract the event timestamp (RFC3339 `timestamp` field on the same JSONL event it already selects; reuse `parse_reset_timestamp`-style leniency only if the existing code already tolerates epochs — otherwise RFC3339 only). Delete now-dead `rate-limits.json` fixtures/tests; keep `codex_from_rate_limits_json`'s file-shape tolerance (aliases) since session-log bytes reuse it.
- [ ] **Step 4: Run** `cargo test codex` → PASS; confirm no test references `rate-limits.json` anymore (`rg -n "rate-limits.json" src tests` → only historical docs).
- [ ] **Step 5: Commit** — `git commit -m "refactor: drop dead codex rate-limits.json stage"`

### Task 10: Handoff corrections

**Files:**
- Modify: `.superpowers/handoff-v11.md:60-63`

- [ ] **Step 1: Edit** the two stale claims: replace the "remover `rate_limits_by_limit_id` especulativo" item with a note that the field is real/populated and is now iterated (done in this PR); mark the 1 MiB cap + anti-symlink item as implemented (tests exist in `codex_session_log.rs`). Keep the file's language (it lives in the excluded `docs/superpowers`-style build record; the file is pt — do not translate the rest).
- [ ] **Step 2: Commit** — `git commit -m "docs: correct stale codex items in handoff"`

### Task 11: PR2 checkpoint and pull request

- [ ] **Step 1: Full battery** — `cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings && git diff --check`, plus the complete QML battery, plus `cargo test --test dist_tree_validate` (bundle contract — QML files changed).
- [ ] **Step 2: Live smoke (read-only):** run the helper binary's status command locally and eyeball the Codex/Amp rows render real data (functional + perceptual + real-data proof before "done").
- [ ] **Step 3: Review diff** for secrets/legacy leakage/unrelated changes.
- [ ] **Step 4: With owner confirmation**, push `feat/codex-rate-limit-fields` and open the PR (English body, spec link, no AI attribution — re-read before creating).

---

## Self-review notes (already applied)

- Spec coverage: Amp windows/plan/discard (T1), classifier (T2), Grok period+plan format (T3), Codex full payload (T5), multi-bucket+codex preference (T6), individualLimit/resets/contract/schema (T7), QML line (T8), dead stage 2 + freshness (T9), handoff (T10). Amp "unknown line never aborts" is guarded by T1's credits-only test.
- Naming consistency: `format_plan_label` (T1→T3→T7), `rate_limit_resets_available` / `rateLimitResetsAvailable` (T5→T7→T8), `extraBuckets`/`individualLimit` normalized keys (T5→T7), window ids `plan-other`, `plan-orb`, `codex:<id>`, `workspace-limit`.
- Banned-word audit: no new id/label/field contains `spend`, `credits`, `balance`, `currency` (checked against `assert_no_money`).
