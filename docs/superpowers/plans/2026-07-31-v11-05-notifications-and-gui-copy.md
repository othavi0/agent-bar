# v11 Notifications and GUI Copy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the notification say what is running out, in the unit the user picked, with the same humanised countdown the popup shows — and rewrite the Settings and Maintenance copy to the approved voice.

**Architecture:** The countdown humaniser moves from QML-only to both languages: a new `src/support/countdown.rs` mirrors `CoreView.countdownText`, and one shared fixture table pins the two implementations to identical output from opposite sides. The notification body stops hardcoding `used`: `PendingNotification` gains the remaining percentage, the display metric already loaded in the coordinator, and the countdown precomputed by the evaluator that owns the clock, so `build_spec` stays pure. GUI copy changes are literal-for-literal in `SettingsView.qml`, `MaintenanceView.qml`, and `CoreMaintenance.js`, and a new guard test bans internal vocabulary from GUI strings so the leak cannot return.

**Tech Stack:** Rust (`time` 0.3, serde_json for the fixture), QML (Qt 6, Quickshell/Quattro host at `/usr/share/omarchy/shell`), qmltestrunner (Qt 6 binary path only).

## Global Constraints

- Contract: `CLAUDE.md` at repo root; product contract: `docs/specs/v10/` plus the approved design `docs/superpowers/specs/2026-07-30-copy-and-language-design.md` (§4 voice, §5.5, §5.6, §5.7, §6.1, §6.2, §8). The visual design's severity rules are already shipped by plan 04 and are not reopened.
- **The CLI and `install.sh` are NOT in this plan.** Copy design §7 governs them, §9 phases them last, and they are plan 06. Do not touch `src/cli/`, `src/main.rs`, or `install.sh`.
- All shipped UI copy is English. The language gate flags alphabetic non-ASCII only; `·`, `—`, `…`, `→` and Nerd Font glyphs pass by design. The gate is blind to unaccented Portuguese — read the words, never "translate until green".
- Rust: no production `unwrap()`/`expect()`. Test code may use them, following the existing test style.
- A11Y-013 / TEST-029: no plugin-authored `Behavior`/`Transition`/`Animation`/`Animator`. The closed `Util.alpha` opacity set is {0.55, 0.72} plus the single named `0.12` exception in `UsageWindow.qml`; `SettingsView.qml` and `MaintenanceView.qml` are both in `tst_Tokens.tokenScannedFiles()`, so any new text colour must be 0.55 or 0.72.
- Status JSON stays frozen at schema v2: no new field. Notification triggers are unchanged — copy design §10 says "no change to what triggers a notification, only to what it says". Thresholds stay on `usedPercent` even though the body follows the display metric (§6.2).
- Test files must NEVER `import qs.Commons` or `qs.Ui` — the pure Qt 6 runner cannot resolve the module and the whole file silently stops compiling.
- `qmltestrunner` from `PATH` is Qt 5 and fails SILENTLY. Always:
  ```bash
  QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
    /usr/lib/qt6/bin/qmltestrunner -input tests/qml \
    -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
  ```
  The bar is **0 failed**; never chase an exact total.
- `qmllint` from `PATH` is a stub reporting version `1.0` that stays silent even on an undefined type. Use `/usr/lib/qt6/bin/qmllint -I /usr/share/omarchy/shell`, judge it by its OUTPUT not its exit code, and expect pre-existing repo-wide `qs.*` unresolved-import noise on every plugin file. The contract now says so (`CLAUDE.md`, amended 2026-07-31).
- `cargo test` accepts ONE filter per invocation. Baseline at plan-05 start: **292 Rust tests / 16 suites, 228 QML / 0 failed**. Known flake: `binary_interactive_update_rejects_non_tty` (`ExecutableFileBusy`) — retry once, pre-existing test-isolation bug.
- Checkpoint gates: `cargo fmt --check` · `cargo test` · `cargo clippy --all-targets -- -D warnings` · `git diff --check` · Qt 6 `qmllint` · `omarchy plugin validate assets/omarchy` · the qmltestrunner line above · `scripts/verify-v10-ui`.
- Commits: English Conventional Commit subject ≤ 50 chars. Never any AI-attribution text in any commit, PR body, or comment.
- The plan-01…04 defect pattern applies: *source that reads correctly and behaves differently at runtime.* Resolve every token and every host property to its real value before trusting it — this plan already caught the spec asking for a host property that does not exist (measured fact 4).

## Measured facts (2026-07-31, this machine)

1. **Settings already reach the notification path; only the value does not.** `src/status/coordinator.rs:112-115` loads `settings` before anything else in `collect`, and `:206-216` passes `&settings` into `NotificationEvaluator`. `NotificationEvaluator` holds `pub settings: &'a SettingsDocument` and already reads `self.settings.notifications.enabled` and `self.settings.providers`. What is missing is only the last hop: `PendingNotification` (`src/notifications/mod.rs:21-29`) carries `used_percent` and no metric, and `build_spec` (`:54-91`) never consults settings. So §6.2 is a threading job, not a plumbing job.
2. **`remaining_percent` already exists and is validated.** `src/status/schema.rs` stores it on `UsageWindow` with the invariant `usedPercent + remainingPercent == 100 ± tolerance`, and exposes `window.remaining_percent()`. Nothing in `src/notifications/` reads it today.
3. **The evaluator has no clock, the coordinator does.** `src/status/coordinator.rs:111` computes `let requested_at = self.clock.now_utc();` before the notification block. `NotificationEvaluator` has no time source at all, so a countdown must either receive `now` or be precomputed. This plan adds `now` to the evaluator and precomputes the countdown string there, keeping `build_spec` a pure function of `PendingNotification`.
4. **The host `NumberField` has no suffix property.** It exposes exactly nine properties (`label`, `value`, `from`, `to`, `stepSize`, `foreground`, `accent`, `fontFamily`, `fontSize`, `fieldWidth`, `hasCursor`, `_hovered`) plus `property alias field: spin`. Copy design §5.7 asks for "`seconds` as the field suffix", which no host property provides. It is a `Column` of a label `Text` and a `QQC.SpinBox`, so the unit becomes a sibling `Text` positioned against `intervalField.field`. **Anchors cannot be used**: the spin box is a child of a sibling, not a sibling, and QML rejects anchoring to it. A plain `y` binding is the working form.
5. **Rust cannot execute the QML side, so parity is pinned by a shared fixture.** The repo bans Node and every JS runtime (`CLAUDE.md` hard rule), so a Rust test cannot call `countdownText`. `tests/severity_parity.rs` solves an easier problem by parsing two constants out of the JS. A function body cannot be parsed that way. The working seam is one JSON table read by both sides: `tests/countdown_parity.rs` asserts the Rust implementation against it and `tst_Format.qml` asserts the QML implementation against the same file, so either side drifting fails its own suite.
6. **`countdownText` truncates, it never rounds.** `Math.floor(diffMs / 60000)` then integer arithmetic: 181 minutes is `3h 1m`, 1439 is `23h 59m`, 1440 is `1d 0h`, 3960 is `2d 18h`. The Rust port must truncate identically; `time::Duration::whole_minutes()` truncates toward zero and matches for non-negative inputs.
7. **A past reset must not read "Resets in now."** `CoreView.resetCountdownText` returns the literal `"now"` once the reset has passed, and `resetPhrase` switches the lead-in to `resets` for exactly that case. The notification body needs the same branch.
8. **The notification canary is `notify_send_argv_shape`** (`src/notifications/mod.rs:355-374`). It pins `"Claude usage warning"` exactly and asserts the body contains `"91% used"` and `"Resets"`. Copy design §8 names it as the canary for §6.2; it is rewritten, not deleted.
9. **`Also delete saved settings and backups` exists and is pinned.** `MaintenanceView.qml:206`, asserted by `tst_Maintenance.qml:174`, mirrored in `tst_Screenshots.qml:178`, and required by `UX-046`. It is unchanged by this plan. (An earlier survey claimed it was absent; it is not.)
10. **`Plugin bundle` is hardcoded three times and pinned once.** `CoreMaintenance.js` sets `installType: "Plugin bundle"` in both `maintenanceUiIdle` and `cloneMaintenanceUi` (the clone re-asserts it, ignoring any prior value), `MaintenanceView.qml:59` renders it, and `tst_Maintenance.qml:171` pins it. §5.6 deletes the row, which makes the field dead — and the v10 contract forbids leaving it dormant.
11. **Copy points with no test pin today**: `Chip number`, `Loading settings…`, `Saving…`, `Danger zone`, `Confirm update`, both `Final confirmation:` strings, `This removes the Agent Bar plugin bundle…`, and `Standard uninstall preserves settings.` Changing them breaks nothing, which is exactly why the new strings must arrive with pins of their own.
12. **The vocabulary guard is red before Task 4 and green after, measured, not assumed.** Running §4 rule 2's full word list (`adapter`, `schema`, `payload`, `envelope`, `bundle`, `collect`, `clause`, `snapshot`) over the 20 GUI source files under `assets/omarchy` — quoted literals containing a space, comment lines skipped — returns exactly **six** violations today, all of them the word `bundle`, and all six inside strings Task 4 deletes or rewrites: `CoreMaintenance.js:90` and `:113` (`installType: "Plugin bundle"`), `CoreMaintenance.js:188` (the old update-confirm sentence), `MaintenanceView.qml:59` (the installation-type row), and `MaintenanceView.qml:188`/`:189` (the two uninstall messages). `collect`, `clause`, and the other five words return zero. So the guard is falsifiable by construction: it fails on the tree that exists when Task 5 starts and passes only because Task 4 landed.

## File Structure

- Create: `src/support/countdown.rs` — `countdown_text` and `reset_countdown`, mirroring `CoreView.countdownText`/`resetCountdownText`.
- Modify: `src/support/mod.rs` — declare and re-export the new module.
- Create: `tests/fixtures/countdown-table.json` — the shared input/output table, read from both languages.
- Create: `tests/countdown_parity.rs` — Rust side of the seam, plus a guard that the QML function still exists.
- Modify: `tests/qml/tst_Format.qml` — QML side of the same seam.
- Modify: `src/notifications/mod.rs` — `PendingNotification` gains `remaining_percent`, `metric`, `reset_in`; `NotificationEvaluator` gains `now`; `build_spec` rewritten to §5.5; canary test rewritten.
- Modify: `src/status/coordinator.rs` — pass `now: requested_at` into the evaluator.
- Modify: `assets/omarchy/SettingsView.qml` — §5.7 labels, plus the `seconds` suffix the host cannot provide.
- Modify: `assets/omarchy/MaintenanceView.qml` — §5.6 labels, uninstall copy, installation-type row deleted.
- Modify: `assets/omarchy/CoreMaintenance.js` — update-confirm message, duplicate error string, `installType` deleted.
- Modify: `tests/qml/tst_Settings.qml`, `tests/qml/tst_Maintenance.qml`, `tests/qml/tst_Screenshots.qml`, `tests/qml/TestPalette.js` — pins for the new copy and the mirror sync.
- Create: `tests/gui_vocabulary.rs` — §8 guard banning internal vocabulary from GUI strings.
- Modify: `docs/specs/v10/05-settings-cache-and-notifications.md` — `NOTIFY-009` and the notification copy block.
- Modify: `docs/specs/v10/04-quickshell-ux-and-accessibility.md` — `UX-040`, `UX-044`.
- Modify: `docs/specs/v10/01-product-contract.md` — the Maintain journey's `Uninstall agent-bar` line.

## Seams with plan 06 (do not cross)

- **The CLI is plan 06.** `src/cli/`, `src/main.rs`, and every string in them stay exactly as they are, including `clause` vocabulary, the duplicated `plugins-dir` message, and the interactive `update` prompts.
- **`install.sh` is plan 06**, all 45 messages and the help block.
- The §8 vocabulary guard added here scans **GUI surfaces only** (`assets/omarchy/**`). Plan 06 decides whether and how to extend it to the CLI, which copy design §7 deliberately exempts from the GUI voice.
- Notification triggering, dedup, re-arm, and persistence are untouched: `NOTIFY-001`…`NOTIFY-008`, `NOTIFY-010`…`NOTIFY-012` keep their current meaning.

---

### Task 1: The countdown humaniser in Rust, pinned to QML by a shared table

**Files:**
- Create: `src/support/countdown.rs`
- Modify: `src/support/mod.rs` (module list :3-7, re-exports :9-17)
- Create: `tests/fixtures/countdown-table.json`
- Create: `tests/countdown_parity.rs`
- Modify: `tests/qml/tst_Format.qml`

**Interfaces:**
- Produces, consumed by Task 2:
  - `agent_bar::support::countdown::countdown_text(remaining: time::Duration) -> String`
  - `agent_bar::support::countdown::reset_countdown(now: OffsetDateTime, reset_at: OffsetDateTime) -> String` — returns the literal `"now"` when the reset has already passed.
- Produces the fixture path `tests/fixtures/countdown-table.json`, whose shape is `[{ "minutes": <int>, "text": "<string>" }]`.

- [ ] **Step 1: Write the shared fixture**

Create `tests/fixtures/countdown-table.json`. Every row is a duration in whole minutes and the exact string both implementations must produce. The rows cover each branch and both sides of every boundary:

```json
[
  { "minutes": 0, "text": "0m" },
  { "minutes": 1, "text": "1m" },
  { "minutes": 59, "text": "59m" },
  { "minutes": 60, "text": "1h 0m" },
  { "minutes": 61, "text": "1h 1m" },
  { "minutes": 181, "text": "3h 1m" },
  { "minutes": 1439, "text": "23h 59m" },
  { "minutes": 1440, "text": "1d 0h" },
  { "minutes": 1500, "text": "1d 1h" },
  { "minutes": 3960, "text": "2d 18h" },
  { "minutes": 10079, "text": "6d 23h" },
  { "minutes": 10080, "text": "7d 0h" }
]
```

- [ ] **Step 2: Write the failing Rust test**

Create `tests/countdown_parity.rs`:

```rust
//! `CoreView.countdownText` humanises a reset countdown in QML; the
//! notification path humanises the same durations in Rust. The repo bans every
//! JS runtime, so a Rust test cannot call the QML function — instead both
//! sides are pinned to one shared table of inputs and expected outputs.
//! `tests/qml/tst_Format.qml` asserts the QML implementation against the same
//! file, so either side drifting fails its own suite.

use agent_bar::support::countdown::countdown_text;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Row {
    minutes: i64,
    text: String,
}

fn table() -> Vec<Row> {
    let raw = std::fs::read_to_string("tests/fixtures/countdown-table.json")
        .expect("read countdown-table.json");
    serde_json::from_str(&raw).expect("parse countdown-table.json")
}

#[test]
fn countdown_matches_the_shared_table() {
    let rows = table();
    assert!(rows.len() >= 12, "the table must keep covering both sides of every branch boundary");
    for row in rows {
        assert_eq!(
            countdown_text(time::Duration::minutes(row.minutes)),
            row.text,
            "minutes = {}",
            row.minutes
        );
    }
}

// The seam is only real while the QML side still exists under that name.
#[test]
fn qml_countdown_function_still_exists() {
    let js = std::fs::read_to_string("assets/omarchy/CoreView.js").expect("read CoreView.js");
    assert!(
        js.contains("function countdownText(diffMs)"),
        "CoreView.countdownText is the other half of this seam"
    );
    assert!(
        js.contains("function resetCountdownText(iso, nowMs)"),
        "CoreView.resetCountdownText is the other half of reset_countdown"
    );
}
```

- [ ] **Step 3: Run it and confirm the intended failure**

Run: `cargo test --test countdown_parity`
Expected: FAIL to compile — `agent_bar::support::countdown` does not exist.

- [ ] **Step 4: Implement**

Create `src/support/countdown.rs`:

```rust
//! Human countdown formatting, shared with the popup.
//!
//! `CoreView.countdownText` renders the same durations in QML on a
//! 30-second timer and must produce byte-identical strings. Neither side can
//! call the other (the product ships no JS runtime), so both are pinned to
//! `tests/fixtures/countdown-table.json`; see `tests/countdown_parity.rs`.

use time::{Duration, OffsetDateTime};

/// `"2d 18h"` | `"3h 1m"` | `"5m"`. Truncates like the QML original: whole
/// minutes first, then whole days and hours out of those. Never rounds up, so
/// a countdown never claims more time than remains.
pub fn countdown_text(remaining: Duration) -> String {
    let total_minutes = remaining.whole_minutes().max(0);
    let days = total_minutes / 1440;
    let hours = (total_minutes % 1440) / 60;
    let minutes = total_minutes % 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

/// Mirrors `CoreView.resetCountdownText` for a window that has a timestamp:
/// the literal `"now"` once the reset has passed, otherwise the countdown.
/// A window with no timestamp never reaches here — the caller renders nothing.
pub fn reset_countdown(now: OffsetDateTime, reset_at: OffsetDateTime) -> String {
    let remaining = reset_at - now;
    if !remaining.is_positive() {
        return "now".to_owned();
    }
    countdown_text(remaining)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn reset_in_the_past_reads_now() {
        let now = datetime!(2026-07-28 15:00:00 UTC);
        assert_eq!(reset_countdown(now, datetime!(2026-07-28 14:00:00 UTC)), "now");
        assert_eq!(reset_countdown(now, now), "now");
    }

    #[test]
    fn reset_in_the_future_counts_down() {
        let now = datetime!(2026-07-28 15:00:00 UTC);
        assert_eq!(reset_countdown(now, datetime!(2026-07-28 18:01:00 UTC)), "3h 1m");
        assert_eq!(reset_countdown(now, datetime!(2026-07-31 09:00:00 UTC)), "2d 18h");
    }

    #[test]
    fn seconds_never_round_the_minute_up() {
        // 59 seconds short of an hour is 59m, not 1h — the popup would
        // otherwise promise a reset that has not arrived.
        let now = datetime!(2026-07-28 15:00:00 UTC);
        assert_eq!(reset_countdown(now, datetime!(2026-07-28 15:59:59 UTC)), "59m");
    }
}
```

In `src/support/mod.rs`, add `pub mod countdown;` to the module list, keeping the existing alphabetical-ish grouping, and do **not** add a blanket re-export — the two call sites use the full path, matching how `redact` is reached in `src/notifications/mod.rs`.

- [ ] **Step 5: Run the Rust test to verify it passes**

Run: `cargo test --test countdown_parity` — expected 2 passed. Then `cargo test --lib support::countdown` — expected 3 passed.

- [ ] **Step 6: Write the QML half of the seam**

`tests/qml/tst_Format.qml` currently has no file-reading helper. Add the canonical one used by every other suite (`repoRoot` derived from `Qt.resolvedUrl(".")`, popping two path segments, plus a synchronous `XMLHttpRequest` `read(rel)` taking a **repo-root-relative** path), then add:

```qml
  // The other half of the Rust seam: same table, same expectations, read from
  // the same file. See tests/countdown_parity.rs.
  function test_countdown_matches_the_shared_table() {
    var rows = JSON.parse(read("tests/fixtures/countdown-table.json"))
    verify(rows.length >= 12, "the shared table must not shrink")
    for (var i = 0; i < rows.length; i++) {
      compare(Core.countdownText(rows[i].minutes * 60000), rows[i].text,
              "minutes = " + rows[i].minutes)
    }
  }
```

- [ ] **Step 7: Run the QML suite** with the Qt 6 command from Global Constraints. Expected 0 failed, with `test_countdown_matches_the_shared_table` printing `PASS`.

- [ ] **Step 8: Prove the seam can fail**

Temporarily change one `text` value in `tests/fixtures/countdown-table.json`, run BOTH `cargo test --test countdown_parity` and the QML suite, and confirm **both** fail. Restore the file exactly (`git diff --stat tests/fixtures/countdown-table.json` must show no change) and re-run both to confirm green. Record the evidence in the task report. Do not commit the temporary change.

- [ ] **Step 9: Full Rust gates** — `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`. Expected **297 across 17 suites**: 292 baseline, plus 2 in the new `countdown_parity` suite, plus the 3 `src/support/countdown.rs` unit tests that land inside the existing lib suite. Report the real number; if it disagrees, say so rather than adjusting the claim.

- [ ] **Step 10: Commit**

```bash
git add src/support/countdown.rs src/support/mod.rs tests/countdown_parity.rs \
  tests/fixtures/countdown-table.json tests/qml/tst_Format.qml
git commit -m "feat: humanise reset countdowns in rust"
```

---

### Task 2: The notification says what is running out, in the user's unit

**Files:**
- Modify: `src/notifications/mod.rs` (`PendingNotification` :21-29, `build_spec` :54-91, `NotificationEvaluator` :120-124, the pending push :159-167, `notify_send_argv_shape` :355-374)
- Modify: `src/status/coordinator.rs` (evaluator construction :206-216)
- Modify: `docs/specs/v10/05-settings-cache-and-notifications.md` (`NOTIFY-009` :190-191, copy block :242-248)

**Interfaces:**
- Consumes: `countdown::reset_countdown` from Task 1; `DisplayMetric` from `crate::settings::schema`.
- Produces: `PendingNotification` with three new public fields — `remaining_percent: f64`, `metric: DisplayMetric`, `reset_in: Option<String>` — and `NotificationEvaluator` with a new `now: time::OffsetDateTime` field. Both are constructed in exactly two places (the evaluator's own pending push, and `src/status/coordinator.rs`), plus tests.

- [ ] **Step 1: Rewrite the canary test first**

In `src/notifications/mod.rs`, replace `notify_send_argv_shape` (`:355-374`) with the version below plus its two siblings. The metric-following body and the countdown are the assertions §8 names this test the canary for:

```rust
    #[test]
    fn notify_send_argv_shape() {
        let pending = PendingNotification {
            provider_id: ProviderId::Claude,
            provider_name: "Claude".into(),
            window_id: "session".into(),
            window_label: "Session (5h)".into(),
            used_percent: 91.4,
            remaining_percent: 8.6,
            metric: DisplayMetric::Remaining,
            reset_at: Some(datetime!(2026-07-26 22:00:00 UTC)),
            reset_in: Some("3h 1m".to_owned()),
            level: NotificationLevel::Warning,
        };
        let spec = NotifySendDispatcher::<ScriptedNotify>::build_spec(&pending);
        assert_eq!(spec.program, PathBuf::from("notify-send"));
        assert_eq!(spec.args[0], "--app-name=Agent Bar");
        assert_eq!(spec.args[1], "--urgency=normal");
        assert_eq!(spec.args[2], "Claude Session (5h) is running low");
        assert_eq!(spec.args[3], "9% left. Resets in 3h 1m.");
    }

    #[test]
    fn notification_body_follows_the_display_metric() {
        // The trigger is always usedPercent, but the sentence is not: the
        // notification must not be the one surface speaking a different unit.
        let mut pending = PendingNotification {
            provider_id: ProviderId::Claude,
            provider_name: "Claude".into(),
            window_id: "session".into(),
            window_label: "Session (5h)".into(),
            used_percent: 96.0,
            remaining_percent: 4.0,
            metric: DisplayMetric::Remaining,
            reset_at: None,
            reset_in: None,
            level: NotificationLevel::Critical,
        };
        let spec = NotifySendDispatcher::<ScriptedNotify>::build_spec(&pending);
        assert_eq!(spec.args[1], "--urgency=critical");
        assert_eq!(spec.args[2], "Claude Session (5h) is almost out");
        // No timestamp: the reset clause is omitted entirely, not filled with
        // a placeholder.
        assert_eq!(spec.args[3], "4% left.");

        pending.metric = DisplayMetric::Used;
        let spec = NotifySendDispatcher::<ScriptedNotify>::build_spec(&pending);
        assert_eq!(spec.args[3], "96% used.");
    }

    #[test]
    fn elapsed_reset_never_reads_resets_in_now() {
        let pending = PendingNotification {
            provider_id: ProviderId::Claude,
            provider_name: "Claude".into(),
            window_id: "session".into(),
            window_label: "Session (5h)".into(),
            used_percent: 96.0,
            remaining_percent: 4.0,
            metric: DisplayMetric::Remaining,
            reset_at: Some(datetime!(2026-07-26 22:00:00 UTC)),
            reset_in: Some("now".to_owned()),
            level: NotificationLevel::Critical,
        };
        let spec = NotifySendDispatcher::<ScriptedNotify>::build_spec(&pending);
        assert_eq!(spec.args[3], "4% left. Resets now.");
    }
```

Add `use crate::settings::schema::DisplayMetric;` to the test module's imports if it is not already reachable there; the production `use` is added in Step 3.

- [ ] **Step 2: Run and confirm the intended failure**

Run: `cargo test --lib notifications`
Expected: FAIL to compile — `PendingNotification` has no `remaining_percent`, `metric`, or `reset_in` field.

- [ ] **Step 3: Implement the struct and the copy**

In `src/notifications/mod.rs`, add `use crate::settings::schema::DisplayMetric;` beside the existing `use crate::settings::schema::Settings as SettingsDocument;` (or extend that import), then replace `PendingNotification` (`:21-29`) with:

```rust
/// Planned notification before dispatch.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingNotification {
    pub provider_id: ProviderId,
    pub provider_name: String,
    pub window_id: String,
    pub window_label: String,
    /// What fired the notification. Always the trigger, never the sentence.
    pub used_percent: f64,
    pub remaining_percent: f64,
    /// The unit the user chose in Settings (copy design §6.2).
    pub metric: DisplayMetric,
    pub reset_at: Option<time::OffsetDateTime>,
    /// Humanised countdown at evaluation time; `None` when the window carries
    /// no reset timestamp. Precomputed by the evaluator, which owns the clock,
    /// so `build_spec` stays a pure function of this struct.
    pub reset_in: Option<String>,
    pub level: NotificationLevel,
}
```

Replace the title and body of `build_spec` (`:54-91`), keeping the surrounding `ProcessSpec` construction, the sanitizer calls, the timeout, and the output cap exactly as they are:

```rust
        // Copy design §5.5: the title names what is running out. The old
        // "{Name} usage warning" said the category, not the thing.
        let title = match pending.level {
            NotificationLevel::Warning => format!(
                "{} {} is running low",
                pending.provider_name, pending.window_label
            ),
            NotificationLevel::Critical => format!(
                "{} {} is almost out",
                pending.provider_name, pending.window_label
            ),
        };
        // §6.2: one unit across the product. The threshold that fired this is
        // always usedPercent; the sentence follows the user's chosen metric.
        let (value, unit) = match pending.metric {
            DisplayMetric::Used => (pending.used_percent, "used"),
            DisplayMetric::Remaining => (pending.remaining_percent, "left"),
        };
        let value = value.round() as i64;
        let body = match pending.reset_in.as_deref() {
            // "Resets in now." is not English; the popup avoids it the same way.
            Some("now") => format!("{value}% {unit}. Resets now."),
            Some(countdown) => format!("{value}% {unit}. Resets in {countdown}."),
            // §5.5: with no timestamp the clause is omitted, not filled in.
            None => format!("{value}% {unit}."),
        };
```

- [ ] **Step 4: Thread the clock and the new fields**

Add the clock to the evaluator (`:120-124`):

```rust
/// Evaluate envelope windows, dispatch escalations, persist per success.
pub struct NotificationEvaluator<'a, D: NotificationDispatcher> {
    pub store: &'a NotificationStateStore,
    pub dispatcher: &'a D,
    pub settings: &'a SettingsDocument,
    /// Supplied by the caller that already read the clock for this collect
    /// cycle, so the countdown agrees with the rest of the envelope.
    pub now: time::OffsetDateTime,
}
```

and populate the new fields in the pending push (`:159-167`), leaving every other field untouched:

```rust
                    pending.push(PendingNotification {
                        provider_id: id,
                        provider_name: provider.name().to_owned(),
                        window_id: window.id().to_owned(),
                        window_label: window.label().to_owned(),
                        used_percent: used,
                        remaining_percent: window.remaining_percent(),
                        metric: self.settings.display.metric,
                        reset_at: window.resets_at(),
                        reset_in: window
                            .resets_at()
                            .map(|ts| crate::support::countdown::reset_countdown(self.now, ts)),
                        level,
                    });
```

In `src/status/coordinator.rs` (`:206-216`), add `now: requested_at,` to the `NotificationEvaluator` literal. `requested_at` is already bound at `:111` for this cycle — do not call the clock a second time.

Every other `NotificationEvaluator` construction in tests must gain the same field; find them with `rg -n 'NotificationEvaluator \{' src tests`.

- [ ] **Step 5: Run the Rust tests** — `cargo test --lib notifications`, expected all passing including the three canaries. Then the full `cargo test`.

- [ ] **Step 6: Amend the specification**

In `docs/specs/v10/05-settings-cache-and-notifications.md`, replace `NOTIFY-009` (`:190-191`) with:

```markdown
- `NOTIFY-009`: Notification copy is safe English. The title names the
  provider and the window; the body states the percentage in the metric
  selected in Settings and, when the reset is known, the humanised time until
  it. Trigger thresholds stay on `usedPercent` regardless of the displayed
  metric.
```

and replace the copy paragraph (`:242-248`) with:

```markdown
Warning title is `<Provider> <Window> is running low`; critical title is
`<Provider> <Window> is almost out`. Body is `<value>% <unit>. Resets in
<countdown>.` when the reset is known and still ahead, `<value>% <unit>.
Resets now.` once it has passed, and `<value>% <unit>.` when the window
carries no reset timestamp. `<unit>` is `left` or `used`, following the
Settings display metric; `<countdown>` is the same humanised form the popup
renders, shared with QML through one pinned table. Values pass the normal
plain-text sanitizer. Spawn failure, timeout, signal, or nonzero exit is a
dispatch failure: report it on stderr, leave that key unadvanced, continue no
later notifications in that evaluation, and still return the valid status
envelope.
```

Preserve the dispatch-failure sentence exactly — it is a separate requirement that happens to share the paragraph.

- [ ] **Step 7: Full gates** — `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`, `cargo test --test active_docs`, `cargo test --test active_language`.

- [ ] **Step 8: Commit**

```bash
git add src/notifications/mod.rs src/status/coordinator.rs \
  docs/specs/v10/05-settings-cache-and-notifications.md
git commit -m "feat: say what is running out, in your unit"
```

---

### Task 3: Settings copy

**Files:**
- Modify: `assets/omarchy/SettingsView.qml` (`Loading settings…` :82, `Chip number` :160, the interval Column :201-222, the notifications `description` :233)
- Modify: `tests/qml/tst_Settings.qml` (`test_settings_view_source_contracts` :133-142)

**Interfaces:**
- Consumes nothing from earlier tasks. Produces no interface; this task is literal-for-literal copy plus one layout addition the host cannot provide.

- [ ] **Step 1: Write the failing test**

In `tests/qml/tst_Settings.qml`, extend `test_settings_view_source_contracts` (`:133-142`) with the new strings and a ban on the old ones, keeping every existing assertion:

```qml
    // Copy design §5.7. The old labels are banned by name so a revert fails
    // here rather than silently shipping.
    verify(src.indexOf("Bar shows") >= 0)
    verify(src.indexOf("Chip number") < 0)
    verify(src.indexOf("Refresh every") >= 0)
    verify(src.indexOf("Refresh interval (seconds)") < 0)
    verify(src.indexOf("Warn me before a quota runs out.") >= 0)
    verify(src.indexOf("Usage threshold alerts") < 0)
    verify(src.indexOf('text: "Loading\\u2026"') >= 0)
    verify(src.indexOf("Loading settings") < 0)
    // The host NumberField has no suffix property, so the unit is a sibling
    // label positioned against the spin box.
    verify(src.indexOf('text: "seconds"') >= 0)
```

- [ ] **Step 2: Run the QML suite — confirm the new asserts fail**

- [ ] **Step 3: Implement the four literal changes**

- `:82` — `text: "Loading…"`
- `:160` — `text: "Bar shows"`
- `:233` — `description: "Warn me before a quota runs out."`
- `:210` — `label: "Refresh every"`

- [ ] **Step 4: Add the `seconds` suffix**

Replace the interval `Column`'s body (`:202-222`) so the field and its unit sit on one row, leaving the `NumberField`'s own bindings untouched:

```qml
    // Refresh interval — native NumberField (UX-035)
    Column {
      width: parent.width
      spacing: Style.space(4)
      opacity: root.locked ? 0.55 : 1.0
      enabled: !root.locked

      Row {
        spacing: Style.spacing.lg

        NumberField {
          id: intervalField
          label: "Refresh every"
          value: root.intervalSec
          from: 30
          to: 3600
          stepSize: 5
          foreground: root.foreground
          fontFamily: root.fontFamily
          onModified: function (v) {
            if (root.agentService)
              root.agentService.setRefreshInterval(v)
          }
        }

        // The host NumberField exposes no suffix property (measured), so the
        // unit is a sibling label. It aligns with the spin box, not with the
        // field's own label above it — and it cannot use anchors, because the
        // spin box is a child of a sibling, which QML refuses to anchor to.
        Text {
          y: intervalField.field.y + (intervalField.field.height - height) / 2
          text: "seconds"
          color: Util.alpha(root.foreground, 0.72)
          font.family: root.fontFamily
          font.pixelSize: Style.font.bodySmall
          textFormat: Text.PlainText
          Accessible.ignored: true
        }
      }
    }
```

`intervalField.field` is the `QQC.SpinBox` the host exposes as `property alias field: spin`. `Accessible.ignored` is correct here: the field's own label already carries the control's accessible name, and a stray "seconds" node would be read as a separate control.

- [ ] **Step 5: Run the full QML gate** (Qt 6 `qmllint`, `omarchy plugin validate`, qmltestrunner). Expected 0 failed. Read the `qmllint` output for anything naming `intervalField` — an unresolvable `field` alias would surface there and nowhere else.

- [ ] **Step 6: Commit**

```bash
git add assets/omarchy/SettingsView.qml tests/qml/tst_Settings.qml
git commit -m "feat: rewrite settings labels in plain words"
```

---

### Task 4: Maintenance copy, and the installation-type row that only ever said one thing

**Files:**
- Modify: `assets/omarchy/MaintenanceView.qml` (installation-type row :52-62, `Uninstall agent-bar` :146/:152/:184, confirmation messages :185-189)
- Modify: `assets/omarchy/CoreMaintenance.js` (`installType` in `maintenanceUiIdle` :86-99 and `cloneMaintenanceUi` :109-122, `updateConfirmMessage` :184-189, duplicate error :166)
- Modify: `tests/qml/tst_Maintenance.qml` (`test_update_confirm_message_names_versions` :95-103, `test_maintenance_view_ux_copy` :169-178)
- Modify: `tests/qml/tst_Accessibility.qml` (`Uninstall agent-bar` assertion :101)
- Modify: `docs/specs/v10/04-quickshell-ux-and-accessibility.md` (`UX-040` :139, `UX-044` :145)
- Modify: `docs/specs/v10/01-product-contract.md` (Maintain journey :107)

**Interfaces:**
- Produces: `maintenanceUiIdle`/`cloneMaintenanceUi` no longer carry `installType`; `updateConfirmMessage(ui)` returns the new sentence. No signature changes.

- [ ] **Step 1: Write the failing tests**

In `tests/qml/tst_Maintenance.qml`, rewrite `test_maintenance_view_ux_copy` (`:169-178`) and extend the confirm-message test (`:95-103`):

```qml
  function test_maintenance_view_ux_copy() {
    var src = read("assets/omarchy/MaintenanceView.qml")
    verify(src.indexOf("Check for updates") >= 0)
    verify(src.indexOf("Uninstall Agent Bar") >= 0)
    verify(src.indexOf("Also delete saved settings and backups") >= 0)
    verify(src.indexOf("ConfirmDialog") >= 0)
    verify(src.indexOf("Release notes") >= 0)
    verify(src.indexOf("Text.RichText") < 0)
    // §5.6: the package name is `agent-bar`; every surface says `Agent Bar`.
    verify(src.indexOf("Uninstall agent-bar") < 0)
    // The installation-type row is gone — it only ever had one value.
    verify(src.indexOf("Installation type") < 0)
    verify(src.indexOf("Plugin bundle") < 0)
    // Ceremony removed: no "Final confirmation:", no "Click Uninstall again."
    verify(src.indexOf("Final confirmation") < 0)
    verify(src.indexOf("Deletes Agent Bar, your settings and every backup.") >= 0)
    verify(src.indexOf("Deletes Agent Bar. Your settings stay.") >= 0)
    verify(src.indexOf("Removes Agent Bar. Your settings stay.") >= 0)
  }

  function test_install_type_is_gone_from_the_model() {
    var src = read("assets/omarchy/CoreMaintenance.js")
    // Dead the moment the row was deleted; the contract forbids keeping it.
    verify(src.indexOf("installType") < 0)
  }

  function test_update_confirm_message_names_versions() {
    var ui = Core.maintenanceUiIdle("10.0.0")
    ui.targetVersion = "10.2.0"
    var msg = Core.updateConfirmMessage(ui)
    verify(msg.indexOf("10.0.0") >= 0)
    verify(msg.indexOf("10.2.0") >= 0)
    verify(msg.toLowerCase().indexOf("settings") >= 0)
    verify(msg.toLowerCase().indexOf("roll back") >= 0 || msg.toLowerCase().indexOf("rollback") >= 0)
    // §5.6 shortened it; the old sentence must not come back.
    verify(msg.indexOf("This replaces the plugin bundle") < 0)
  }

  function test_update_check_failure_has_one_string() {
    var src = read("assets/omarchy/CoreMaintenance.js")
    verify(src.indexOf("Update check returned an unusable response.") < 0)
    var first = src.indexOf("Update check failed.")
    verify(first >= 0)
    verify(src.indexOf("Update check failed.", first + 1) > first,
           "both failure branches must use the one string")
  }
```

In `tests/qml/tst_Accessibility.qml:101`, change the assertion to `verify(maint.indexOf("Uninstall Agent Bar") >= 0)`.

- [ ] **Step 2: Run the QML suite — confirm the new asserts fail**

- [ ] **Step 3: Implement `MaintenanceView.qml`**

- Delete the installation-type row entirely (`:52-62` — the `Text` at `:59` and its wrapping element; leave the installed-version row at `:46-49` alone).
- `:146`, `:152`, `:184` — `Uninstall Agent Bar`.
- `:185-189` — replace the three-way message with:

```qml
        message: ui.uninstallArmed
            ? (ui.purgeSettings
                ? "Deletes Agent Bar, your settings and every backup."
                : "Deletes Agent Bar. Your settings stay.")
            : "Removes Agent Bar. Your settings stay."
```

Leave `confirmText: ui.uninstallArmed ? "Uninstall now" : "Uninstall"` (`:191`) untouched — the second click is still required (`UX-047`), and the button text is what now carries that, since the sentence no longer says "Click Uninstall again."

- [ ] **Step 4: Implement `CoreMaintenance.js`**

- Delete the `installType` field from both `maintenanceUiIdle` and `cloneMaintenanceUi`. Trace every reader first (`rg -n 'installType' assets tests`) — the deletion is invisible to all three QML gates if one survives.
- `:166` — replace `"Update check returned an unusable response."` with `"Update check failed."`, which the sibling branch at `:132` already uses.
- `:184-189` — replace `updateConfirmMessage` with:

```js
function updateConfirmMessage(ui) {
  var current = ui && ui.installedVersion ? String(ui.installedVersion) : "current"
  var target = ui && ui.targetVersion ? String(ui.targetVersion) : "new"
  return "Updates " + current + " → " + target
      + ". Settings stay. Rolls back if it fails."
}
```

`→` is the arrow the copy design writes as `→`; it is a symbol, not a letter, so the language gate passes it. Write it escaped to match how this file already writes `…`.

- [ ] **Step 5: Run the full QML gate.** Expected 0 failed.

- [ ] **Step 6: Amend the specification**

`docs/specs/v10/04-quickshell-ux-and-accessibility.md`:

```markdown
- `UX-040`: Show the installed version.
```
```markdown
- `UX-044`: `Uninstall Agent Bar` is visually separated as a danger action.
```

`docs/specs/v10/01-product-contract.md:107` — change `Uninstall agent-bar` to `Uninstall Agent Bar` in the Maintain journey, leaving the rest of the sentence alone.

Then run `cargo test --test active_docs` and `cargo test --test active_language`.

- [ ] **Step 7: Commit**

```bash
git add assets/omarchy/MaintenanceView.qml assets/omarchy/CoreMaintenance.js \
  tests/qml/tst_Maintenance.qml tests/qml/tst_Accessibility.qml \
  docs/specs/v10/04-quickshell-ux-and-accessibility.md \
  docs/specs/v10/01-product-contract.md
git commit -m "feat: plain words for update and uninstall"
```

---

### Task 5: The internal-vocabulary guard

**Files:**
- Create: `tests/gui_vocabulary.rs`

**Interfaces:**
- Consumes the copy shipped by Tasks 3 and 4. Produces no interface.

- [ ] **Step 1: Write the test**

Copy design §4 rule 2 bans internal vocabulary from GUI strings, and §8 asks for a test so the leak cannot return. Create `tests/gui_vocabulary.rs`:

```rust
//! Copy design §4 rule 2: GUI strings name what the user sees, not what the
//! code calls it. Twenty-one strings leaked internal vocabulary before the
//! rewrite; this test is what stops the twenty-second.
//!
//! Scope is the GUI only. The CLI is deliberately exempt (§7): its stderr is
//! read while debugging and Unix convention there is different.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Copy design §4 rule 2's list, complete. `provider` is deliberately absent
/// — decision 3 keeps it on screen. `bundle` is the one that was actually
/// leaking: six GUI strings carried it until the maintenance rewrite.
const BANNED: &[&str] = &[
    "adapter", "schema", "payload", "envelope", "bundle", "collect", "clause", "snapshot",
];

fn gui_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.join("assets/omarchy")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let is_source = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e == "qml" || e == "js");
            if is_source {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Double-quoted string literals, minus the ones that are not user copy:
/// property lookups, enum values, and glyphs never reach a label.
fn user_facing_literals(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        // A literal is copy only when it contains a space: single-token
        // strings in this codebase are ids, enum values, and glyphs.
        for piece in line.split('"').skip(1).step_by(2) {
            if piece.contains(' ') {
                out.push(piece.to_owned());
            }
        }
    }
    out
}

#[test]
fn gui_copy_has_no_internal_vocabulary() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut violations: BTreeSet<String> = BTreeSet::new();
    let files = gui_files(&root);
    assert!(
        files.len() >= 15,
        "expected the GUI source tree, found {} files",
        files.len()
    );
    for path in files {
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let rel = path.strip_prefix(&root).unwrap_or(&path).display().to_string();
        for literal in user_facing_literals(&source) {
            let lowered = literal.to_lowercase();
            for word in BANNED {
                if lowered.split(|c: char| !c.is_ascii_alphabetic()).any(|t| t == *word) {
                    violations.insert(format!("{rel}: {word} in {literal:?}"));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "GUI copy leaks internal vocabulary ({}):\n  - {}",
        violations.len(),
        violations.into_iter().collect::<Vec<_>>().join("\n  - ")
    );
}
```

- [ ] **Step 2: Run it and confirm it is green — for the right reason**

Run: `cargo test --test gui_vocabulary`

Expected: PASS, and the scan must have covered **20 files** (assert `files.len() >= 15` guards the floor; report the real count). Measured fact 12 says this exact scan returned six `bundle` violations before Task 4 and zero after, so a green run here confirms Task 4 actually removed them rather than the scanner missing them.

If it fails, **read every violation before changing anything**: a real leak is fixed in the copy, but a false positive — a banned word inside a code identifier the literal scanner mistook for prose, or inside a comment the line filter missed — is fixed in the scanner, never by weakening `BANNED`. Record which of the two you hit, with the exact violation list, in the task report.

- [ ] **Step 3: Prove it was really red before Task 4**

Check out the pre-Task-4 version of the two files the measurement names, run the test, and confirm it fails naming `bundle`:

```bash
git stash list   # expect empty; you are about to touch the worktree
git checkout HEAD~1 -- assets/omarchy/MaintenanceView.qml assets/omarchy/CoreMaintenance.js
cargo test --test gui_vocabulary   # expect FAIL, six `bundle` violations
git checkout HEAD -- assets/omarchy/MaintenanceView.qml assets/omarchy/CoreMaintenance.js
cargo test --test gui_vocabulary   # expect PASS
git diff --stat                    # must be empty
```

`HEAD~1` is Task 4's commit only if Task 5 is the very next commit — verify with `git log --oneline -3` first and use the right ref if not. Record the failing output in the task report; do not commit anything from this step.

- [ ] **Step 4: Commit**

```bash
git add tests/gui_vocabulary.rs
git commit -m "test: ban internal vocabulary from gui copy"
```

---

### Task 6: Screenshot mirror, full checkpoint, execution record

**Files:**
- Modify: `tests/qml/tst_Screenshots.qml` (`paintState` strings)
- Modify: `docs/superpowers/plans/2026-07-31-v11-05-notifications-and-gui-copy.md` (this file)

**Interfaces:** consumes everything above; produces no shipped interface.

- [ ] **Step 1: Sync the mirror**

`tst_Screenshots.qml` is a hand-maintained mirror that follows nothing automatically. Update the strings the copy changed, leaving the 17 capture names and the inventory lists alone (no capture is added or removed by this plan, so `TestPalette.js` and `scripts/verify-v10-ui` are NOT touched):

- `settings-clean-dark` body — replace `Providers · Remaining · Interval 60 · Notifications on` with `Providers · Remaining · Refresh every 60 seconds · Notifications on`.
- `settings-invalid-dark` body — replace `Refresh interval out of range · Save changes disabled` with `Refresh every out of range · Save changes disabled`.
- `maintenance-update-dark` body — replace `Plugin bundle · Check for updates · Release notes` with `Check for updates · Release notes`, since the installation-type row is gone.
- `uninstall-confirmation-dark` title — `Uninstall Agent Bar`.

Then run `scripts/verify-v10-ui` and confirm `17 PNGs ok` with exit 0.

- [ ] **Step 2: Hygiene greps (all must return nothing outside `docs/superpowers/`)**

```bash
rg -n 'usage warning|usage critical' src assets
rg -n 'Chip number|Refresh interval \(seconds\)|Usage threshold alerts|Loading settings' assets
rg -n 'Uninstall agent-bar|Installation type|installType' assets tests docs/specs
rg -n 'Final confirmation|Click Uninstall again' assets
rg -n 'Update check returned an unusable response' assets
```

- [ ] **Step 3: Full checkpoint**

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
git diff --check
find assets/omarchy -type f -name '*.qml' -exec \
  /usr/lib/qt6/bin/qmllint -I /usr/share/omarchy/shell {} +
omarchy plugin validate assets/omarchy
QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
  /usr/lib/qt6/bin/qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
scripts/verify-v10-ui
```

Expected: Rust **300 across 18 suites** — 292 baseline, plus 3 `countdown` unit tests in the lib suite, plus 2 in `countdown_parity`, plus 2 new notification tests in the lib suite, plus 1 in `gui_vocabulary`. Verify the real number rather than trusting this arithmetic and report what it actually is; a mismatch means a test was lost somewhere, which is worth finding. QML 0 failed, everything else clean.

- [ ] **Step 4: Execution record** — append to this file in the shape plans 01–04 use: commit list mapped to tasks, final test counts, "What the plan got wrong", "Deferred minors carried forward". Then:

```bash
git add docs/superpowers/plans/2026-07-31-v11-05-notifications-and-gui-copy.md \
  tests/qml/tst_Screenshots.qml
git commit -m "docs: record execution outcome in plan"
```

---

## Done when

- `cargo test` green, with the new `countdown_parity` and `gui_vocabulary` suites present and the three rewritten notification canaries passing.
- Changing one row of `tests/fixtures/countdown-table.json` fails BOTH `cargo test --test countdown_parity` and the QML suite (verified by hand once, then reverted).
- `qmltestrunner` 0 failed, with `test_countdown_matches_the_shared_table`, `test_settings_view_source_contracts`, `test_maintenance_view_ux_copy`, `test_install_type_is_gone_from_the_model`, `test_update_check_failure_has_one_string`, and `test_update_confirm_message_names_versions` printing `PASS`.
- A notification for a Claude session window at 96% used with a reset three hours out reads `Claude Session (5h) is almost out` / `4% left. Resets in 3h 1m.` when the display metric is `remaining`, and `96% used. Resets in 3h 1m.` when it is `used`.
- Every hygiene grep in Task 6 Step 2 returns nothing; `installType` is gone from the model, not merely unrendered.
- `NOTIFY-009` and the notification copy block, `UX-040`, `UX-044`, and the product contract's Maintain journey all match shipped behaviour.
- Nothing installed into `~/.config/omarchy/plugins/` — live QA remains the owner's gate.

## Not in this plan

| Plan | Covers |
| --- | --- |
| 06 — CLI and installer copy | Copy design §7: `clause` and other non-CLI jargon, the duplicated `plugins-dir` message, problems stated without their fix, the 45 `install.sh` messages plus its help block, and the interactive `update` prompts. Also decides whether the §8 vocabulary guard extends past the GUI. |

Deliberately untouched here:

- Notification triggering, dedup, re-arm, and persistence (`NOTIFY-001`…`008`, `010`…`012`).
- `ProviderHeader.showStale`, dead since plan 03 and still carried in `headerModel`.
- The `Installed version` row, `Danger zone`, `Release notes`, and the confirm-button texts, which the copy design does not change.
