# v11 Severity and Lead-Window Election Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the product a severity model whose thresholds are shared with the Rust notifier, elect one lead window deterministically instead of by an id allowlist, and render the popup as one large lead window plus compact rows that each carry their own track.

**Architecture:** Every new rule is a pure function in `CoreView.js` (`severityLevel`, `providerSeverity`, `severityTagText`, `electLeadIndex`, `windowLayout`, `resetCountdownText`), tested by direct JS calls; QML only binds. The two threshold numbers exist twice — in `NotificationLevel::from_used_percent` and in `CoreView.js` — because the status schema is frozen at v2 and must not gain a field, so a new Rust test reads the JS side and fails the build on drift. `PRIMARY_WINDOW_IDS`, `windowGroups`, and `formatResetText` are deleted, not left dormant; the popup shows the countdown only, never an absolute clock.

**Tech Stack:** QML (Qt 6, Quickshell/Quattro host at `/usr/share/omarchy/shell`), qmltestrunner (Qt 6 binary path only), Rust for the shared thresholds and the parity gate.

## Global Constraints

- Contract: `CLAUDE.md` at repo root; product contract: `docs/specs/v10/` plus the approved design `docs/superpowers/specs/2026-07-30-visual-update-design.md` (§3.4, §3.7, §6, §7, §8, §9, §14). The copy design `-copy-and-language-design.md` §6.1/§6.2 is plan 05 and must not be pulled forward.
- All shipped UI copy is English. The language gate flags alphabetic non-ASCII only; `󰅐` (U+F0150), `·`, `—`, `…` pass by design. The gate is blind to unaccented Portuguese — never "translate until green", read the words.
- A11Y-012: no provider state is color-only. Every severity level carries a word (`Critical` / `Low`) in the header tag and in `Accessible.name`; `Color.urgent` never carries meaning alone. A11Y-013 / TEST-029: no plugin-authored `Behavior`/`Transition`/animation, including in new files.
- Test files must never `import qs.Commons` or `qs.Ui` — the pure Qt 6 runner cannot resolve the module and the whole file stops compiling with a misleading error. No test instantiates the real popup components; everything is XHR text-scan or direct JS calls.
- `qmltestrunner` from `PATH` is Qt 5 and fails silently. Always:
  ```bash
  QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
    /usr/lib/qt6/bin/qmltestrunner -input tests/qml \
    -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
  ```
  The bar is **0 failed** plus named tests printing `PASS`; never chase an exact total.
- `cargo test` accepts ONE filter per invocation. Baseline at plan-04 start: **290 Rust tests / 15 suites, 203 QML / 0 failed**. Known flake: `binary_interactive_update_rejects_non_tty` (`ExecutableFileBusy` on a machine with a live plugin) — retry once, it is a pre-existing test-isolation bug, not a regression.
- Checkpoint gates: `cargo fmt --check` · `cargo test` · `cargo clippy --all-targets -- -D warnings` · `git diff --check` · `qmllint -I /usr/share/omarchy/shell` over `assets/omarchy/**/*.qml` · `omarchy plugin validate assets/omarchy` · the qmltestrunner line above · `scripts/verify-v10-ui`.
- Commits: English Conventional Commit subject ≤ 50 chars. Never any AI-attribution text in any commit, PR body, or comment.
- The plan-01/02/03 defect pattern applies: *source that reads correctly and behaves differently at runtime.* Resolve every token to its runtime value before trusting it. The three QML gates (`qmllint`, `qmltestrunner`, `omarchy plugin validate`) are **blind to a dangling id or reference in plugin QML** — the runner never compiles a file that imports `qs.*`. Mitigation after every deletion: trace the references by hand **and** add a text guard banning the exact dead identifier.
- `tst_Tokens.qml` locks the scanned files to the closed `Util.alpha` set {0.55, 0.72} plus the single named `UsageWindow` 0.12 exception. This plan uses 0.72 (labels, unit) and 0.55 (compact reset column) and adds no third value and no second exception. `Item.opacity` (0.45/0.6/0.9/1.0) is not scanned and is unaffected.
- Closed decisions (do not reopen): one lead window rendered large and every other window compact; severity thresholds are the Rust ones, never a second set; `Color.urgent` is the only severity colour; tags, not pills; motion stays host-owned; the popup shows the countdown only.

## Measured facts (2026-07-31, this machine)

1. **The thresholds are inline literals today.** `src/notifications/state.rs:33-41` has `used >= 95.0` / `used >= 90.0` written into `from_used_percent`; there is no named const. `NotificationLevel` has exactly two variants, `Warning` and `Critical`, and derives `PartialOrd`/`Ord` from declaration order. The only Rust user-facing severity words are the notify-send urgency and title in `src/notifications/mod.rs:54-66`; nothing in Rust ever renders the word `Low`.
2. **The Rust-reads-JS parity template already exists**: `tests/servicecore_contract.rs` reads `assets/omarchy/CoreService.js` with a bare relative path (`std::fs::read_to_string("assets/omarchy/CoreService.js")`, resolved against the crate root by cargo) and parses with plain string finds, not a regex crate. `src/lib.rs` exports `pub mod notifications` and `src/notifications/mod.rs` exports `pub mod state`, so a test reaches new consts as `agent_bar::notifications::state::NAME`.
3. **Window fields reach QML as camelCase, verbatim.** `src/status/schema.rs:251-261` serializes `id`, `label`, `usedPercent`, `remainingPercent`, `resetsAt`; `CoreService.parseStatusEnvelope` is `JSON.parse` with validation and no renaming, so QML reads exactly those names. `resetsAt` is an RFC 3339 **string** (`"2026-07-26T22:00:00Z"`), never epoch. Windows arrive as a plain array in helper order and nothing sorts or filters them today.
4. **`windowDisplayLines` currently drops the raw percentages.** It emits only the display-metric `percent`/`percentText` (`CoreView.js:474-499`). Election rule 1 needs `remainingPercent` and severity needs `usedPercent`, both independent of the display metric (§7), so the line objects must start carrying the raw numbers.
5. **The approved mockup is the authority for the pixels** (`.superpowers/brainstorm/3552115-1785415426/content/final.html`, "Desenho fechado"). Measured from its CSS and markup: lead numeral **30px** (= `Style.font.body` 12 × 2.5), unit 10px (`caption`), lead track **6px** (`Style.spacing.md`), lead label line 11px (`bodySmall`) and **not uppercase**; compact row 11px with label ≈25% width, **4px track** (`Style.spacing.sm`), bold value, and a 10px muted countdown column; lead→rows gap 14 (`Style.spacing.xxxl`), row→row gap 10 (`Style.spacing.xl`); **no separator between the lead and the compact rows**; header severity tag reuses the plan-tag shape with `border-color` and `color` swapped to urgent (`.tag.crit`).
6. **The mockup's bar row settles the chip question**: critical Claude renders `7%` **and** `!` both in urgent, not dimmed; disconnected Grok renders `—` and `!` in the plain bar foreground, dimmed. So the urgent tint belongs to *severity*, never to the error cue, even though both draw `!`.
7. **The absolute clock disappears.** The mockup's lead reads `Session (5h) · reseta em 3h 1m` and every compact row reads `23h 1m` — no `14:59`, no weekday, anywhere. `formatResetText` (and with it `WEEKDAYS` and the only `Qt.formatDateTime` call in the plugin) has exactly one caller, `windowDisplayLines`, so it becomes dead code and the v10 contract forbids keeping it dormant. `countdownText` stays and is called directly.
8. **`UX-017` and `UX-028` were already amended by plan 03** and must not be re-amended; `UX-017` already says "severity when present". `UX-020A` (`docs/specs/v10/04-quickshell-ux-and-accessibility.md:66-67`) is the only spec amendment left, and `docs/specs/v10/04-*.md` carries **no** requirement today for severity thresholds, lead-window election, or window ordering — those must be added, not edited.
9. **Adding a QML file is invisible to two guards unless it is registered.** `tst_Accessibility.v10QmlFiles()` (`:28-45`) is the TEST-029 motion scan and `tst_Tokens.tokenScannedFiles()` (`:25-40`) is the token scan; both are hand-maintained lists. A new `components/HeaderTag.qml` that is not added to both ships unguarded. The plugin bundle copies `assets/omarchy` recursively — no manifest or inventory lists individual QML files.
10. **The screenshot mirror is hand-written and pinned in four places**: `tst_Screenshots.paintState` strings, `tst_Screenshots.test_required_names_match_spec` (`compare(names.length, 16)`), `tests/qml/TestPalette.js:requiredScreenshotNames()`, and `scripts/verify-v10-ui`'s `REQUIRED=( … )`. `tests/screenshot_inventory.rs` cross-checks only the last two; the count and the panel strings are not cross-checked by anything.
11. **`Color.urgent` resolves per theme.** `/usr/share/omarchy/shell/Commons/Color.qml:22` defaults to `#a55555` and `:159` remaps it from the theme's `red`/`color1`; on the owner's live theme it is `#ff9fbc`. It is already used by the stale banner (`ProviderView.qml:93,110`), so nothing new is introduced.

## File Structure

- Modify: `assets/omarchy/CoreView.js` — add severity constants and `severityLevel`/`severityTagText`/`providerSeverity`; add `chipSeverityUrgent`/`chipCueLabel` and fold severity into `chipStateCue`; add `resetCountdownText`/`resetPhrase` and delete `formatResetText`/`WEEKDAYS`; carry raw percentages, severity, and countdown on every line; add `electLeadIndex` and `windowLayout`; delete `PRIMARY_WINDOW_IDS` and `windowGroups`; fix the stale future-tense comment at `:144-146`.
- Modify: `src/notifications/state.rs` — name the two thresholds as `pub const` and use them in `from_used_percent`.
- Create: `tests/severity_parity.rs` — the Rust↔JS threshold seam.
- Modify: `assets/omarchy/components/ProviderChip.qml` — urgent numeral and cue, humanized cue `Accessible.name`.
- Modify: `assets/omarchy/BarWidget.qml` — bind the two new chip properties.
- Create: `assets/omarchy/components/HeaderTag.qml` — the one tag shape, with an urgent variant.
- Modify: `assets/omarchy/components/ProviderHeader.qml` — plan and severity tags via `HeaderTag`, exact spacer math.
- Modify: `assets/omarchy/components/UsageWindow.qml` — 2.5× lead numeral, promoted reset line, urgent numeral/fill when critical, compact row with its own track, value column, and measured countdown column.
- Modify: `assets/omarchy/ProviderView.qml` — lead/rest wiring, severity to the header, quiet separator removed.
- Modify: `docs/specs/v10/04-quickshell-ux-and-accessibility.md` — extend `UX-020A`; add `UX-020C` (severity) and `UX-020D` (lead election).
- Modify: `tests/qml/tst_ProviderStates.qml`, `tests/qml/tst_Format.qml`, `tests/qml/tst_BarWidget.qml`, `tests/qml/tst_Popup.qml`, `tests/qml/tst_Accessibility.qml`, `tests/qml/tst_Tokens.qml`, `tests/qml/tst_Screenshots.qml`, `tests/qml/TestPalette.js`, `scripts/verify-v10-ui`.
- Modify: `README.md`, `docs/architecture.md` — the rail is no longer bordered (inherited from plan 03) and the popup leads with one window.

## Seams with plan 05 (do not cross)

- **The Rust countdown humaniser is plan 05.** This plan does not add `countdownText` to Rust and does not touch `src/notifications/mod.rs` at all; `state.rs` is the only Rust file that changes, and only to name two numbers.
- **Notification copy and the notification display metric are plan 05.** The Rust notification title keeps saying `usage warning`/`usage critical`; the word `Low` is a GUI tag word only.
- **CLI and `install.sh` copy are plan 05.**
- The status schema stays frozen at v2: no new field, no severity in JSON.

---

### Task 1: CoreView severity model, lead election, countdown-only reset

**Files:**
- Modify: `assets/omarchy/CoreView.js` (`chipStateCue` :123-132, `stateQualifier` comment :144-146, `countdownText` :430-440, `formatResetText` :442-455, `windowDisplayLines` :474-499, `PRIMARY_WINDOW_IDS` :501, `windowGroups` :503-513)
- Test: `tests/qml/tst_ProviderStates.qml`, `tests/qml/tst_Format.qml`

**Interfaces:**
- Produces, consumed by Tasks 2–6:
  - `SEVERITY_CRITICAL_USED_PERCENT = 95`, `SEVERITY_WARNING_USED_PERCENT = 90` (module-level `var`, exact names — Task 2's Rust test parses them).
  - `severityLevel(usedPercent) -> "critical" | "warning" | ""`.
  - `severityTagText(level) -> "Critical" | "Low" | ""`.
  - `providerSeverity(provider) -> "critical" | "warning" | ""` (worst window).
  - `chipSeverityUrgent(provider) -> bool`.
  - `chipCueLabel(provider) -> string` (humanized word for the cue).
  - `resetCountdownText(iso, nowMs) -> "" | "now" | "3h 1m" | "2d 18h"`.
  - `resetPhrase(countdown) -> "" | "resets" | "resets in"`.
  - `windowLayout(provider, metric, nowMs) -> { lead: line | null, rest: [line] }`, where a line is
    `{ id, label, percentText, percent, usedPercent, remainingPercent, severity, resetsAt, resetCountdown, resetPhrase }`.
- Deleted, and no caller may survive: `PRIMARY_WINDOW_IDS`, `windowGroups`, `formatResetText`, `WEEKDAYS`.

- [ ] **Step 1: Write the failing tests — severity and election**

In `tests/qml/tst_ProviderStates.qml`, **delete** `test_window_groups_split_primary_and_models` (`:166-185`) and add, in its place:

```qml
  function readyWith(windows) {
    return { id: "claude", name: "Claude", state: "ready", windows: windows }
  }

  function layoutOf(windows, nowIso) {
    return Core.windowLayout(readyWith(windows), "remaining", Date.parse(nowIso))
  }

  function test_severity_level_thresholds() {
    compare(Core.severityLevel(89.9), "")
    compare(Core.severityLevel(90), "warning")
    compare(Core.severityLevel(94.9), "warning")
    compare(Core.severityLevel(95), "critical")
    compare(Core.severityLevel(100), "critical")
    compare(Core.severityLevel("nope"), "")
  }

  // The Warning level renders as "Low" (visual design §7); "warning" is the
  // internal name it shares with Rust.
  function test_severity_tag_words() {
    compare(Core.severityTagText("critical"), "Critical")
    compare(Core.severityTagText("warning"), "Low")
    compare(Core.severityTagText(""), "")
  }

  function test_provider_severity_is_the_worst_window() {
    compare(Core.providerSeverity(null), "")
    compare(Core.providerSeverity({ windows: [] }), "")
    compare(Core.providerSeverity({ windows: [{ usedPercent: 10 }, { usedPercent: 92 }] }),
            "warning")
    compare(Core.providerSeverity({ windows: [{ usedPercent: 92 }, { usedPercent: 97 }] }),
            "critical")
  }

  // §7: severity is computed from usedPercent, so switching the displayed
  // metric never changes what counts as critical.
  function test_severity_ignores_the_display_metric() {
    var provider = readyWith([{ id: "s", label: "S", usedPercent: 96, remainingPercent: 4,
                                resetsAt: "2026-07-28T18:00:00Z" }])
    var now = Date.parse("2026-07-28T15:00:00Z")
    compare(Core.windowLayout(provider, "remaining", now).lead.severity, "critical")
    compare(Core.windowLayout(provider, "used", now).lead.severity, "critical")
  }

  function test_lead_election_critical_beats_nearest_reset() {
    var layout = layoutOf([
      { id: "session", label: "Session (5h)", usedPercent: 10, remainingPercent: 90,
        resetsAt: "2026-07-28T15:30:00Z" },
      { id: "weekly", label: "Weekly (7d)", usedPercent: 96, remainingPercent: 4,
        resetsAt: "2026-07-31T11:59:59Z" }
    ], "2026-07-28T15:00:00Z")
    compare(layout.lead.id, "weekly")
    compare(layout.lead.severity, "critical")
    compare(layout.rest.length, 1)
    compare(layout.rest[0].id, "session")
  }

  function test_lead_election_lowest_remaining_among_criticals() {
    var layout = layoutOf([
      { id: "a", label: "A", usedPercent: 96, remainingPercent: 4,
        resetsAt: "2026-07-28T15:30:00Z" },
      { id: "b", label: "B", usedPercent: 99, remainingPercent: 1,
        resetsAt: "2026-07-31T11:59:59Z" }
    ], "2026-07-28T15:00:00Z")
    compare(layout.lead.id, "b")
  }

  function test_lead_election_nearest_future_reset_when_healthy() {
    var layout = layoutOf([
      { id: "weekly", label: "Weekly (7d)", usedPercent: 40, remainingPercent: 60,
        resetsAt: "2026-07-31T11:59:59Z" },
      { id: "session", label: "Session (5h)", usedPercent: 4, remainingPercent: 96,
        resetsAt: "2026-07-28T18:00:00Z" }
    ], "2026-07-28T15:00:00Z")
    compare(layout.lead.id, "session")
    // The rest keeps delivered order, not election order.
    compare(layout.rest.length, 1)
    compare(layout.rest[0].id, "weekly")
  }

  function test_lead_election_ignores_elapsed_and_missing_resets() {
    var layout = layoutOf([
      { id: "past", label: "Past", usedPercent: 10, remainingPercent: 90,
        resetsAt: "2026-07-28T14:00:00Z" },
      { id: "none", label: "None", usedPercent: 20, remainingPercent: 80, resetsAt: null },
      { id: "future", label: "Future", usedPercent: 30, remainingPercent: 70,
        resetsAt: "2026-07-29T09:00:00Z" }
    ], "2026-07-28T15:00:00Z")
    compare(layout.lead.id, "future")
    compare(layout.rest.length, 2)
    compare(layout.rest[0].id, "past")
  }

  function test_lead_election_without_any_reset_takes_first_delivered() {
    var layout = layoutOf([
      { id: "first", label: "First", usedPercent: 20, remainingPercent: 80, resetsAt: null },
      { id: "second", label: "Second", usedPercent: 10, remainingPercent: 90, resetsAt: null }
    ], "2026-07-28T15:00:00Z")
    compare(layout.lead.id, "first")
    compare(layout.rest.length, 1)
  }

  function test_lead_election_ties_keep_delivered_order() {
    var layout = layoutOf([
      { id: "first", label: "First", usedPercent: 50, remainingPercent: 50,
        resetsAt: "2026-07-28T18:00:00Z" },
      { id: "second", label: "Second", usedPercent: 50, remainingPercent: 50,
        resetsAt: "2026-07-28T18:00:00Z" }
    ], "2026-07-28T15:00:00Z")
    compare(layout.lead.id, "first")
  }

  function test_single_window_leads_with_no_rest() {
    var layout = layoutOf([
      { id: "session", label: "Session (5h)", usedPercent: 42, remainingPercent: 58,
        resetsAt: "2026-07-28T18:00:00Z" }
    ], "2026-07-28T15:00:00Z")
    compare(layout.lead.id, "session")
    compare(layout.rest.length, 0)
  }

  function test_no_windows_means_no_lead() {
    var layout = layoutOf([], "2026-07-28T15:00:00Z")
    compare(layout.lead, null)
    compare(layout.rest.length, 0)
  }

  // A window id outside the old allowlist is now electable — the exact bug
  // the allowlist caused (it silently demoted anything unknown).
  function test_unknown_window_id_can_lead() {
    var layout = layoutOf([
      { id: "weekly-model:opus", label: "Opus", usedPercent: 97, remainingPercent: 3,
        resetsAt: "2026-07-31T11:59:59Z" },
      { id: "session", label: "Session (5h)", usedPercent: 10, remainingPercent: 90,
        resetsAt: "2026-07-28T15:30:00Z" }
    ], "2026-07-28T15:00:00Z")
    compare(layout.lead.id, "weekly-model:opus")
  }

  function test_lines_carry_raw_percentages_and_countdown() {
    var layout = layoutOf([
      { id: "session", label: "Session (5h)", usedPercent: 31, remainingPercent: 69,
        resetsAt: "2026-07-28T17:59:59Z" }
    ], "2026-07-28T15:00:00Z")
    compare(layout.lead.usedPercent, 31)
    compare(layout.lead.remainingPercent, 69)
    compare(layout.lead.percentText, "69%")
    compare(layout.lead.resetCountdown, "2h 59m")
    compare(layout.lead.resetPhrase, "resets in")
  }
```

- [ ] **Step 2: Write the failing tests — countdown-only reset**

In `tests/qml/tst_Format.qml`, **replace** the five `formatResetText` tests (`:11-39`) with:

```qml
  function test_reset_countdown_under_1h() {
    compare(Core.resetCountdownText("2026-07-28T15:37:00Z", nowMs), "37m")
  }

  function test_reset_countdown_under_24h_keeps_hours() {
    compare(Core.resetCountdownText("2026-07-28T17:30:00Z", nowMs), "2h 30m")
  }

  function test_reset_countdown_over_24h_uses_days() {
    compare(Core.resetCountdownText("2026-07-31T09:00:00Z", nowMs), "2d 18h")
  }

  function test_reset_countdown_past_is_now() {
    compare(Core.resetCountdownText("2026-07-28T14:00:00Z", nowMs), "now")
  }

  function test_reset_countdown_invalid_is_empty() {
    compare(Core.resetCountdownText("", nowMs), "")
    compare(Core.resetCountdownText("garbage", nowMs), "")
    compare(Core.resetCountdownText(null, nowMs), "")
  }

  // The lead window's label line reads "Session (5h) · resets in 3h 1m", and
  // "resets in now" is not English.
  function test_reset_phrase_follows_the_countdown() {
    compare(Core.resetPhrase("3h 1m"), "resets in")
    compare(Core.resetPhrase("now"), "resets")
    compare(Core.resetPhrase(""), "")
  }
```

Keep `test_ago_variants` unchanged — the stale banner still uses `formatAgoText`.

- [ ] **Step 3: Write the failing deletion guards**

In `tests/qml/tst_Popup.qml`, add (the file's `read()` takes a **repo-root-relative** path — no `../../`):

```qml
  // The three QML gates never compile a file that imports qs.*, so a dangling
  // reference to a deleted function is invisible to them. These guards ban the
  // exact dead identifiers by name.
  function test_primary_window_allowlist_is_gone() {
    var core = read("assets/omarchy/CoreView.js")
    verify(core.indexOf("PRIMARY_WINDOW_IDS") < 0,
           "the id allowlist must be deleted, not left dormant")
    verify(core.indexOf("windowGroups") < 0,
           "windowGroups is replaced by windowLayout")
    verify(core.indexOf("function electLeadIndex") >= 0)
    var view = read("assets/omarchy/ProviderView.qml")
    verify(view.indexOf("groups.primary") < 0)
    verify(view.indexOf("groups.secondary") < 0)
  }

  // §6/§3.7: the popup shows the countdown only. The absolute-clock humaniser
  // is deleted with its weekday table and its only Qt.formatDateTime call.
  function test_absolute_clock_humaniser_is_gone() {
    var core = read("assets/omarchy/CoreView.js")
    verify(core.indexOf("formatResetText") < 0)
    verify(core.indexOf("WEEKDAYS") < 0)
    verify(core.indexOf("Qt.formatDateTime") < 0)
    verify(core.indexOf("function resetCountdownText") >= 0)
  }
```

- [ ] **Step 4: Run the QML suite — confirm the new tests fail**

```bash
QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
  /usr/lib/qt6/bin/qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
```

Expected: the new `test_severity_*`, `test_lead_election_*`, `test_reset_countdown_*`, and both deletion guards FAIL (undefined functions; `PRIMARY_WINDOW_IDS`/`formatResetText` still present). Everything else passes. If a *pre-existing* test fails, stop and report — the baseline is 203 passed / 0 failed.

- [ ] **Step 5: Implement the severity model**

In `assets/omarchy/CoreView.js`, insert immediately after `ERROR_STATES` (`:111-117`):

```js
// Severity thresholds on usedPercent (visual design §7). These duplicate
// NotificationLevel::from_used_percent (src/notifications/state.rs): the
// status schema is frozen at v2 and must not gain a field, so the numbers
// live on both sides and tests/severity_parity.rs fails the build if either
// side moves alone. Change one, change both.
var SEVERITY_CRITICAL_USED_PERCENT = 95
var SEVERITY_WARNING_USED_PERCENT = 90

// "critical" | "warning" | "". Always from usedPercent, never from the
// displayed metric — switching to `used` must not change what is critical.
function severityLevel(usedPercent) {
  var v = Number(usedPercent)
  if (!isFinite(v))
    return ""
  if (v >= SEVERITY_CRITICAL_USED_PERCENT)
    return "critical"
  if (v >= SEVERITY_WARNING_USED_PERCENT)
    return "warning"
  return ""
}

// The word the header tag renders. Warning shows as "Low" (§7 table);
// "warning" stays the internal level name shared with the Rust notifier.
function severityTagText(level) {
  if (level === "critical")
    return "Critical"
  if (level === "warning")
    return "Low"
  return ""
}

// The provider's worst window. A11Y-012: this drives a word, never a colour
// on its own.
function providerSeverity(provider) {
  if (!provider || !Kernel.isArrayLike(provider.windows))
    return ""
  var worst = ""
  for (var i = 0; i < provider.windows.length; i++) {
    var w = provider.windows[i]
    if (!w)
      continue
    var level = severityLevel(w.usedPercent)
    if (level === "critical")
      return "critical"
    if (level === "warning")
      worst = "warning"
  }
  return worst
}

// Severity tints the bar only for a ready provider: any other state already
// dims the whole chip and spends the cue on the state itself, so an urgent
// tint there would mean two things at once. Approved mockup: critical Claude
// is urgent, disconnected Grok is not.
function chipSeverityUrgent(provider) {
  if (!provider)
    return false
  return String(provider.state || "") === "ready"
      && providerSeverity(provider) === "critical"
}
```

- [ ] **Step 6: Implement the chip cue changes**

Replace `chipStateCue` (`:123-132`) with:

```js
function chipStateCue(provider) {
  if (!provider)
    return ""
  var state = String(provider.state || "")
  if (state === "stale")
    return "󰅐"
  if (ERROR_STATES[state])
    return "!"
  if (chipSeverityUrgent(provider))
    return "!"
  return ""
}

// Plan 02 deferred minor: the cue exposed its raw glyph to screen readers.
// It now carries a word — the severity when severity produced the cue,
// otherwise the same qualifier the tooltip already speaks.
function chipCueLabel(provider) {
  if (!provider)
    return ""
  if (chipSeverityUrgent(provider))
    return "critical"
  var state = String(provider.state || "")
  if (state === "stale" || ERROR_STATES[state])
    return stateQualifier(state)
  return ""
}
```

Note the ordering constraint: `chipCueLabel` calls `stateQualifier`, which is declared at `:147`. Both are function declarations in the same `.pragma library`, so hoisting makes the order irrelevant — place `chipCueLabel` directly after `chipStateCue` for readability.

While in this region, fix the stale future-tense comment above `stateQualifier` (`:144-146`) — plan 03 already did the deletion it announces:

```js
// Copy design §5.4: lowercase trailing qualifier for the chip tooltip and
// for the cue's accessible name. It replaced `connectionLabel`, which plan 03
// deleted together with the meta footer.
```

- [ ] **Step 7: Implement countdown-only resets**

Delete `WEEKDAYS` (`:428`) and `formatResetText` (`:442-455`) entirely. Keep `countdownText` unchanged and add after it:

```js
// §6: the popup renders the countdown alone — no absolute clock, no weekday.
// "" when there is no usable timestamp, "now" once the reset has passed.
function resetCountdownText(iso, nowMs) {
  var ms = parseIsoMs(iso)
  if (!isFinite(ms))
    return ""
  var diff = ms - nowMs
  if (diff <= 0)
    return "now"
  return countdownText(diff)
}

// The muted lead-in the lead window prints before the countdown:
// "Session (5h) · resets in 3h 1m" / "Session (5h) · resets now".
function resetPhrase(countdown) {
  var c = String(countdown || "")
  if (!c.length)
    return ""
  return c === "now" ? "resets" : "resets in"
}
```

- [ ] **Step 8: Implement the line model and the election**

Replace `windowDisplayLines` (`:474-499`) with:

```js
function windowDisplayLines(provider, metric, nowMs) {
  var lines = []
  if (!provider || !Kernel.isArrayLike(provider.windows))
    return lines
  var mode = metric === "used" ? "used" : "remaining"
  var effectiveNowMs = nowMs === undefined ? Date.now() : nowMs
  for (var i = 0; i < provider.windows.length; i++) {
    var w = provider.windows[i]
    if (!w)
      continue
    var used = Number(w.usedPercent)
    var remaining = Number(w.remainingPercent)
    var pct = mode === "used" ? used : remaining
    var finite = isFinite(pct)
    var rounded = finite ? Math.round(pct) : null
    // Keep the escaped form the rest of this file already uses.
    var pctText = finite ? (rounded + "%") : "\u2014"
    var countdown = w.resetsAt
        ? resetCountdownText(String(w.resetsAt), effectiveNowMs)
        : ""
    lines.push({
      id: String(w.id || ("w" + i)),
      label: plainText(w.label || w.id || "Window"),
      percentText: pctText,
      // 0–100 for progress track; -1 when unavailable.
      percent: finite ? Math.max(0, Math.min(100, rounded)) : -1,
      // Raw percentages drive severity (§7) and election (§8); they never
      // follow the displayed metric. null when the provider omitted them.
      usedPercent: isFinite(used) ? used : null,
      remainingPercent: isFinite(remaining) ? remaining : null,
      severity: severityLevel(used),
      resetsAt: w.resetsAt ? String(w.resetsAt) : null,
      resetCountdown: countdown,
      resetPhrase: resetPhrase(countdown)
    })
  }
  return lines
}
```

Replace `PRIMARY_WINDOW_IDS` (`:501`) and `windowGroups` (`:503-513`) with:

```js
// A critical window with no usable remainingPercent sorts last among its
// peers instead of winning the comparison by accident.
function remainingRank(line) {
  return line.remainingPercent === null ? Infinity : line.remainingPercent
}

// §8: deterministic lead election, replacing the PRIMARY_WINDOW_IDS
// allowlist that silently demoted any window id it did not know. Returns an
// index into `lines`, or -1 when there is nothing to lead.
//
// Delivered order is unique per line, so `<` on the index is already a total
// tiebreak; the spec's further "then by window id" step is unreachable and is
// deliberately not written as dead code.
function electLeadIndex(lines) {
  if (!lines || !lines.length)
    return -1

  var i
  var best = -1

  // 1. Any critical window leads; among criticals, the lowest remaining.
  for (i = 0; i < lines.length; i++) {
    if (lines[i].severity !== "critical")
      continue
    if (best < 0 || remainingRank(lines[i]) < remainingRank(lines[best]))
      best = i
  }
  if (best >= 0)
    return best

  // 2. Otherwise the nearest reset still in the future. A missing timestamp
  //    or one that already elapsed ("now") does not compete.
  var bestMs = NaN
  for (i = 0; i < lines.length; i++) {
    if (!lines[i].resetCountdown.length || lines[i].resetCountdown === "now")
      continue
    var ms = parseIsoMs(lines[i].resetsAt)
    if (!isFinite(ms))
      continue
    if (best < 0 || ms < bestMs) {
      best = i
      bestMs = ms
    }
  }
  if (best >= 0)
    return best

  // 3. Nothing has a future reset: the first delivered window leads.
  return 0
}

// §3.4: the popup leads with one window; every other window renders as a
// compact row, in delivered order.
function windowLayout(provider, metric, nowMs) {
  var lines = windowDisplayLines(provider, metric, nowMs)
  var leadIndex = electLeadIndex(lines)
  var layout = { lead: null, rest: [] }
  for (var i = 0; i < lines.length; i++) {
    if (i === leadIndex)
      layout.lead = lines[i]
    else
      layout.rest.push(lines[i])
  }
  return layout
}
```

- [ ] **Step 9: Bridge `ProviderView.qml` so the branch stays green**

Deleting `windowGroups` leaves `ProviderView.qml:43` calling a function that no longer exists — a dangling reference none of the three QML gates can see (Global Constraints). Rewire it here, in the same commit as the deletion, without doing Task 5's layout work.

In `assets/omarchy/ProviderView.qml`, replace `:43`:

```qml
  readonly property var windows: Core.windowLayout(provider, displayMetric, nowMs)
```

and feed the two existing `Repeater`s from the new shape, changing nothing else about them:

```qml
        model: root.windows.lead ? [root.windows.lead] : []
```
```qml
        model: root.windows.rest
```

Each `UsageWindow` delegate still binds `resetText: modelData.resetText ? modelData.resetText : ""`, which is now always `""` because the line objects no longer carry that field — the lead loses its reset line until Task 5 restores it as the promoted label line. That is the intended intermediate state; do not add a compatibility `resetText` back into `windowDisplayLines`.

- [ ] **Step 10: Run the QML suite — expect 0 failed**

Same command as Step 4. `tst_ProviderStates`, `tst_Format`, and both `tst_Popup` deletion guards must pass. If a *pre-existing* test fails, stop and report.

- [ ] **Step 11: Rust gates** — `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`. Expected: unchanged 290/15, all clean (no Rust file changed yet).

- [ ] **Step 12: Commit**

```bash
git add assets/omarchy/CoreView.js assets/omarchy/ProviderView.qml \
  tests/qml/tst_ProviderStates.qml tests/qml/tst_Format.qml tests/qml/tst_Popup.qml
git commit -m "feat: elect lead window and model severity"
```

---

### Task 2: Rust threshold constants and the parity gate

**Files:**
- Modify: `src/notifications/state.rs` (`from_used_percent` :33-41)
- Create: `tests/severity_parity.rs`

**Interfaces:**
- Consumes: Task 1's `SEVERITY_CRITICAL_USED_PERCENT` / `SEVERITY_WARNING_USED_PERCENT` in `assets/omarchy/CoreView.js`.
- Produces: `agent_bar::notifications::state::CRITICAL_USED_PERCENT: f64` and `WARNING_USED_PERCENT: f64`.

- [ ] **Step 1: Write the failing test**

Create `tests/severity_parity.rs`:

```rust
//! The severity thresholds exist twice: in Rust, where they fire
//! notifications, and in `CoreView.js`, where they colour the popup and the
//! bar. The status schema is frozen at v2 and must not carry them, so this
//! test is the seam — it reads the JS constants and fails the build if either
//! side moves alone.

use agent_bar::notifications::state::{
    NotificationLevel, CRITICAL_USED_PERCENT, WARNING_USED_PERCENT,
};

/// `var NAME = 95` → `95.0`. Deliberately dumb string parsing, like
/// `tests/servicecore_contract.rs`: no regex dependency, and a rename on
/// either side fails loudly instead of silently matching nothing.
fn js_constant(source: &str, name: &str) -> f64 {
    let needle = format!("var {name} = ");
    let start = source
        .find(&needle)
        .unwrap_or_else(|| panic!("{name} not found in CoreView.js"));
    let rest = &source[start + needle.len()..];
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(rest.len());
    rest[..end]
        .parse()
        .unwrap_or_else(|_| panic!("{name} is not a number in CoreView.js"))
}

fn core_view() -> String {
    std::fs::read_to_string("assets/omarchy/CoreView.js").expect("read CoreView.js")
}

#[test]
fn severity_thresholds_match_core_view() {
    let js = core_view();
    assert_eq!(
        js_constant(&js, "SEVERITY_CRITICAL_USED_PERCENT"),
        CRITICAL_USED_PERCENT,
        "CoreView.js critical threshold drifted from NotificationLevel"
    );
    assert_eq!(
        js_constant(&js, "SEVERITY_WARNING_USED_PERCENT"),
        WARNING_USED_PERCENT,
        "CoreView.js warning threshold drifted from NotificationLevel"
    );
}

#[test]
fn severity_boundaries_agree_across_the_seam() {
    // Behaviour, not only literals: every boundary the two sides could
    // disagree on, driven by the numbers the JS side actually ships.
    let js = core_view();
    let critical = js_constant(&js, "SEVERITY_CRITICAL_USED_PERCENT");
    let warning = js_constant(&js, "SEVERITY_WARNING_USED_PERCENT");

    let cases = [
        (warning - 0.1, None),
        (warning, Some(NotificationLevel::Warning)),
        (critical - 0.1, Some(NotificationLevel::Warning)),
        (critical, Some(NotificationLevel::Critical)),
        (100.0, Some(NotificationLevel::Critical)),
    ];
    for (used, expected) in cases {
        assert_eq!(
            NotificationLevel::from_used_percent(used),
            expected,
            "used = {used}"
        );
    }
}
```

- [ ] **Step 2: Run it and confirm the intended failure**

Run: `cargo test --test severity_parity`
Expected: FAIL to compile — `CRITICAL_USED_PERCENT` and `WARNING_USED_PERCENT` are not defined in `agent_bar::notifications::state`.

- [ ] **Step 3: Implement**

In `src/notifications/state.rs`, add above the `NotificationLevel` enum (after `NOTIFICATION_STATE_VERSION` at `:16`):

```rust
/// Severity thresholds on `usedPercent`. Duplicated in
/// `assets/omarchy/CoreView.js` because the status schema is frozen at v2 and
/// must not gain a field; `tests/severity_parity.rs` fails the build if the
/// two sides drift.
pub const CRITICAL_USED_PERCENT: f64 = 95.0;
pub const WARNING_USED_PERCENT: f64 = 90.0;
```

and rewrite the body of `from_used_percent` to read them:

```rust
    pub fn from_used_percent(used: f64) -> Option<Self> {
        if used >= CRITICAL_USED_PERCENT {
            Some(Self::Critical)
        } else if used >= WARNING_USED_PERCENT {
            Some(Self::Warning)
        } else {
            None
        }
    }
```

Leave the existing `thresholds` unit test (`:280-291`) untouched: it pins the literal numbers from the Rust side, which is exactly the assertion that would otherwise disappear once the code stops naming them inline.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --test severity_parity`
Expected: 2 passed.

- [ ] **Step 5: Full Rust gates**

```bash
cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings && git diff --check
```

Expected: **292 passed across 16 suites** (290 + the two new tests in the new `severity_parity` suite), clippy and fmt clean.

- [ ] **Step 6: Commit**

```bash
git add src/notifications/state.rs tests/severity_parity.rs
git commit -m "test: pin severity thresholds across rust and qml"
```

---

### Task 3: Bar chip severity treatment

**Files:**
- Modify: `assets/omarchy/components/ProviderChip.qml` (numeral `Text` :80-93, cue `Text` :95-106)
- Modify: `assets/omarchy/BarWidget.qml` (chip bindings :87-107)
- Test: `tests/qml/tst_BarWidget.qml` (`test_chip_state_cue` :253-262)

**Interfaces:**
- Consumes: `Core.chipStateCue`, `Core.chipSeverityUrgent`, `Core.chipCueLabel` from Task 1.
- Produces: `ProviderChip.severityUrgent: bool` and `ProviderChip.cueLabel: string`, both defaulting to the non-severity behaviour so an unbound chip renders exactly as today.

- [ ] **Step 1: Write the failing tests**

In `tests/qml/tst_BarWidget.qml`, extend `test_chip_state_cue` (`:253-262`) and add two tests after it:

```qml
  function test_chip_state_cue() {
    compare(Core.chipStateCue(null), "")
    compare(Core.chipStateCue({ state: "ready" }), "")
    compare(Core.chipStateCue({ state: "loading" }), "")
    compare(Core.chipStateCue({ state: "stale" }), "󰅐")
    compare(Core.chipStateCue({ state: "cli_missing" }), "!")
    compare(Core.chipStateCue({ state: "unauthenticated" }), "!")
    compare(Core.chipStateCue({ state: "rate_limited" }), "!")
    compare(Core.chipStateCue({ state: "network_error" }), "!")
    compare(Core.chipStateCue({ state: "provider_error" }), "!")
    // §7: a ready provider over the critical threshold earns the same cue.
    compare(Core.chipStateCue({ state: "ready", windows: [{ usedPercent: 96 }] }), "!")
    compare(Core.chipStateCue({ state: "ready", windows: [{ usedPercent: 92 }] }), "")
    // A state cue outranks severity: the clock keeps the stale meaning.
    compare(Core.chipStateCue({ state: "stale", windows: [{ usedPercent: 96 }] }), "󰅐")
  }

  // The urgent tint belongs to severity, never to the error cue — the
  // approved mockup shows critical Claude urgent and disconnected Grok plain.
  function test_chip_severity_urgent_only_when_ready_and_critical() {
    compare(Core.chipSeverityUrgent(null), false)
    compare(Core.chipSeverityUrgent({ state: "ready", windows: [{ usedPercent: 96 }] }), true)
    compare(Core.chipSeverityUrgent({ state: "ready", windows: [{ usedPercent: 92 }] }), false)
    compare(Core.chipSeverityUrgent({ state: "stale", windows: [{ usedPercent: 96 }] }), false)
    compare(Core.chipSeverityUrgent({ state: "network_error", windows: [] }), false)
  }

  // Plan 02 deferred minor: the cue used to expose its raw glyph.
  function test_chip_cue_label_is_a_word() {
    compare(Core.chipCueLabel({ state: "ready", windows: [{ usedPercent: 96 }] }), "critical")
    compare(Core.chipCueLabel({ state: "stale", windows: [] }), "stale")
    compare(Core.chipCueLabel({ state: "cli_missing", windows: [] }), "no CLI")
    compare(Core.chipCueLabel({ state: "unauthenticated", windows: [] }), "signed out")
    compare(Core.chipCueLabel({ state: "ready", windows: [{ usedPercent: 10 }] }), "")
  }

  function sourceAt(url) {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", url, false)
    xhr.send()
    return String(xhr.responseText)
  }

  function test_chip_source_binds_severity() {
    var chip = sourceAt(chipUrl)
    verify(chip.indexOf("property bool severityUrgent") >= 0)
    verify(chip.indexOf("property string cueLabel") >= 0)
    verify(chip.indexOf("Color.urgent") >= 0,
           "severity uses the host urgent token, never a literal")
    verify(chip.indexOf("Accessible.name: root.stateCue") < 0,
           "the cue must speak a word, not its glyph")
    var widget = sourceAt(widgetUrl)
    verify(widget.indexOf("chipSeverityUrgent") >= 0)
    verify(widget.indexOf("chipCueLabel") >= 0)
  }
```

`tst_BarWidget.qml` has **no** named `read()` helper: it holds full `file://` URLs in `widgetUrl`/`chipUrl`/`coreViewUrl` (`:22-24`) and inlines the `XMLHttpRequest` at each call site (`:164-171`). The `sourceAt` helper above factors that same pattern once; do not import the repo-root-relative `read(rel)` shape from `tst_Popup.qml`, which would resolve to the wrong path here.

- [ ] **Step 2: Run the QML suite — confirm the new asserts fail**

Same command as Task 1 Step 4. Expected failures: the three new cue/severity comparisons and `test_chip_source_binds_severity`.

- [ ] **Step 3: Implement `ProviderChip.qml`**

Add two properties after `iconScale` (`:21`):

```qml
  property bool severityUrgent: false
  property string cueLabel: ""

  // §7: Color.urgent is the single severity colour; no new colour exists.
  readonly property color numeralColor: root.severityUrgent ? Color.urgent : root.foreground
```

Bind the numeral `Text` (`:80-93`) with `color: root.numeralColor` (replacing `color: root.foreground`), and rewrite the cue `Text` (`:95-106`) as:

```qml
    Text {
      visible: !root.vertical && root.stateCue.length > 0
      text: root.stateCue
      color: root.numeralColor
      font.family: root.fontFamily
      font.pixelSize: Style.font.body
      anchors.verticalCenter: parent.verticalCenter
      renderType: Text.NativeRendering
      textFormat: Text.PlainText
      Accessible.name: root.cueLabel.length > 0 ? root.cueLabel : root.stateCue
      Accessible.role: Accessible.StaticText
    }
```

`Color` is already in scope through `import qs.Commons` (`:3`). Add no `Behavior` — the host owns motion (A11Y-013); `WidgetButton`'s own `Behavior on color` lives on its hidden label and does not reach this text, which is the accepted trade-off recorded by plan 02.

- [ ] **Step 4: Implement the `BarWidget.qml` bindings**

In the `ProviderChip` delegate (`:87-107`), add after `stateCue`:

```qml
        cueLabel: Core.chipCueLabel(modelData)
        severityUrgent: Core.chipSeverityUrgent(modelData)
```

- [ ] **Step 5: Run the full QML gate**

```bash
find assets/omarchy -type f -name '*.qml' -exec qmllint -I /usr/share/omarchy/shell {} +
omarchy plugin validate assets/omarchy
QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
  /usr/lib/qt6/bin/qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
```

Expected: clean, 0 failed.

- [ ] **Step 6: Commit**

```bash
git add assets/omarchy/components/ProviderChip.qml assets/omarchy/BarWidget.qml \
  tests/qml/tst_BarWidget.qml
git commit -m "feat: tint the chip when a quota is critical"
```

---

### Task 4: Header severity tag

**Files:**
- Create: `assets/omarchy/components/HeaderTag.qml`
- Modify: `assets/omarchy/components/ProviderHeader.qml` (plan pill :45-66, spacer :68-75)
- Test: `tests/qml/tst_Popup.qml` (`test_header_has_no_provider_icon` region), `tests/qml/tst_Accessibility.qml` (`v10QmlFiles` :28-45), `tests/qml/tst_Tokens.qml` (`tokenScannedFiles` :25-40)

**Interfaces:**
- Consumes: `Core.severityTagText`, `Core.providerSeverity` (Task 1) — wired by Task 5, which owns `ProviderView.qml`.
- Produces:
  - `HeaderTag { label: string; urgent: bool; accessibleName: string; foreground: color; fontFamily: string }`, self-hiding when `label` is empty.
  - `ProviderHeader.severityText: string` and `ProviderHeader.severityUrgent: bool`, both defaulting to empty/false.

- [ ] **Step 1: Write the failing tests**

In `tests/qml/tst_Popup.qml`, add:

```qml
  // §6: name · plan tag · [severity tag] · spacer · refresh. One tag shape,
  // one urgent variant — not two hand-copied Rectangles.
  function test_header_renders_plan_and_severity_tags() {
    var hdr = read("assets/omarchy/components/ProviderHeader.qml")
    verify(hdr.indexOf("HeaderTag {") >= 0)
    verify(hdr.indexOf("id: planTag") >= 0)
    verify(hdr.indexOf("id: severityTag") >= 0)
    verify(hdr.indexOf("property string severityText") >= 0)
    verify(hdr.indexOf("property bool severityUrgent") >= 0)
    // The pill's own Rectangle is gone; the tag lives in one file now.
    verify(hdr.indexOf("border.color: Style.normalBorderColor") < 0)

    var tag = read("assets/omarchy/components/HeaderTag.qml")
    verify(tag.indexOf("Color.urgent") >= 0)
    verify(tag.indexOf("radius: Style.cornerRadius") >= 0)
    verify(tag.indexOf("Font.AllUppercase") >= 0)
    verify(tag.indexOf("Qt.rgba(") < 0)
  }

  // The refresh glyph must stay inside the pane when both tags render; the
  // spacer subtracts the real tag widths instead of a lump constant.
  function test_header_spacer_accounts_for_both_tags() {
    var hdr = read("assets/omarchy/components/ProviderHeader.qml")
    verify(hdr.indexOf("planTag.visible ? planTag.width") >= 0)
    verify(hdr.indexOf("severityTag.visible ? severityTag.width") >= 0)
    verify(hdr.indexOf("Style.space(60)") < 0,
           "the old lump constant hid the second tag's width")
  }
```

In `tests/qml/tst_Accessibility.qml`, add `"assets/omarchy/components/HeaderTag.qml"` to the array returned by `v10QmlFiles()` (`:28-45`), after the `ProviderHeader.qml` entry. In `tests/qml/tst_Tokens.qml`, add the same path to `tokenScannedFiles()` (`:25-40`). Do **not** add it to `convertedFiles()` — that list means "converted away from `Qt.darker`", and this file never had one.

- [ ] **Step 2: Run the QML suite — confirm failure**

Expected: both new `tst_Popup` tests fail, and the two list additions fail with "file not found" style empty reads (the XHR helper returns `""` for a missing file, so `indexOf(...) < 0` assertions may pass vacuously — the `tst_Popup` positives are the falsifiable ones).

- [ ] **Step 3: Create `assets/omarchy/components/HeaderTag.qml`**

```qml
import QtQuick
import qs.Commons

// Header tag (visual design §6): a 1px border at the host corner radius,
// caption type, uppercase. Plan and severity share this one shape; `urgent`
// is the severity variant (§7), and Color.urgent is the only severity colour
// in the product. Uppercasing also normalises plan labels that arrive
// lowercase from the API, such as Codex's `plus`.
Rectangle {
  id: root

  property string label: ""
  property bool urgent: false
  property string accessibleName: ""
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family

  visible: root.label.length > 0
  implicitWidth: tagText.implicitWidth + Style.space(10)
  implicitHeight: tagText.implicitHeight + Style.space(4)
  width: implicitWidth
  height: implicitHeight
  radius: Style.cornerRadius
  color: "transparent"
  border.width: 1
  border.color: root.urgent ? Color.urgent : Style.normalBorderColor

  Text {
    id: tagText
    anchors.centerIn: parent
    text: root.label
    color: root.urgent ? Color.urgent : Util.alpha(root.foreground, 0.72)
    font.family: root.fontFamily
    font.pixelSize: Style.font.caption
    font.capitalization: Font.AllUppercase
    font.letterSpacing: 0.5
    textFormat: Text.PlainText
    Accessible.name: root.accessibleName.length > 0 ? root.accessibleName : root.label
    Accessible.role: Accessible.StaticText
  }
}
```

`Style.cornerRadius` defaults to **0** (square) and themes override it — never compensate for that here.

- [ ] **Step 4: Rewrite `ProviderHeader.qml`**

Add the two properties after `showStale` (`:15`):

```qml
  property string severityText: ""
  property bool severityUrgent: false
```

Replace the plan pill `Rectangle` (`:45-66`) with two `HeaderTag` instances, and replace the spacer (`:68-75`) with the exact math:

```qml
    HeaderTag {
      id: planTag
      anchors.verticalCenter: parent.verticalCenter
      label: root.plan
      accessibleName: "plan " + root.plan
      foreground: root.foreground
      fontFamily: root.fontFamily
    }

    HeaderTag {
      id: severityTag
      anchors.verticalCenter: parent.verticalCenter
      label: root.severityText
      urgent: root.severityUrgent
      accessibleName: "severity " + root.severityText
      foreground: root.foreground
      fontFamily: root.fontFamily
    }

    Item {
      // Flexible spacer; never negative. Pushes the refresh glyph right.
      // Subtracts what is actually rendered — an invisible tag takes no
      // room in the Row, so it must take none here either.
      width: Math.max(Style.space(4),
          parent.width
          - nameLabel.width
          - (planTag.visible ? planTag.width + row.spacing : 0)
          - (severityTag.visible ? severityTag.width + row.spacing : 0)
          - Style.space(22)
          - row.spacing)
      height: 1
    }
```

`Style.space(22)` is the `PanelActionButton` size already declared at `:79`. The header's own file comment (`:5-8`) should gain the severity tag; keep the `UX-016` note about deliberately having no provider icon.

- [ ] **Step 5: Run the full QML gate**

Same three commands as Task 3 Step 5. Expected clean, 0 failed. `qmllint` must resolve `HeaderTag` through the `assets/omarchy` import path — if it reports an unknown type, the file is in the wrong directory, not the import.

- [ ] **Step 6: Commit**

```bash
git add assets/omarchy/components/HeaderTag.qml \
  assets/omarchy/components/ProviderHeader.qml \
  tests/qml/tst_Popup.qml tests/qml/tst_Accessibility.qml tests/qml/tst_Tokens.qml
git commit -m "feat: add a severity tag to the provider header"
```

---

### Task 5: Lead window, compact rows with tracks, and the spec amendments

**Files:**
- Modify: `assets/omarchy/components/UsageWindow.qml` (whole body)
- Modify: `assets/omarchy/ProviderView.qml` (`groups` binding :43, window Columns :146-202, header instance :64-76)
- Modify: `docs/specs/v10/04-quickshell-ux-and-accessibility.md` (`UX-020A` :66-67)
- Test: `tests/qml/tst_Popup.qml`

**Interfaces:**
- Consumes: `Core.windowLayout`, `Core.providerSeverity`, `Core.severityTagText` (Task 1); `ProviderHeader.severityText`/`severityUrgent` (Task 4).
- Produces: `UsageWindow { label; percentText; percent; unitText; severity; resetCountdown; resetPhrase; emphasis; dimmed; foreground; accent; fontFamily }`. `resetText` is **removed** from the API — no caller may keep using it.

- [ ] **Step 1: Write the failing tests**

In `tests/qml/tst_Popup.qml`, add:

```qml
  // §6: the lead window is 2.5x the body size with the reset promoted into
  // its label line; the old bottom "resets" row is gone.
  function test_lead_window_geometry_and_label_line() {
    var win = read("assets/omarchy/components/UsageWindow.qml")
    verify(win.indexOf("Math.round(Style.font.body * 2.5)") >= 0)
    verify(win.indexOf("Style.font.body * 1.8") < 0)
    verify(win.indexOf("root.resetPhrase") >= 0)
    verify(win.indexOf("root.resetCountdown") >= 0)
    verify(win.indexOf("property string resetText") < 0,
           "resetText is replaced by the countdown pair")
    verify(win.indexOf('text: "resets"') < 0,
           "the reset row moved into the label line")
  }

  // UX-020A extended: every window row carries a track, not just the lead.
  function test_compact_rows_carry_their_own_track() {
    var win = read("assets/omarchy/components/UsageWindow.qml")
    var compact = win.slice(win.indexOf("id: compactRow"))
    verify(compact.length > 0, "compactRow must still exist")
    verify(compact.indexOf("color: root.trackColor") >= 0,
           "the compact row needs a track of its own (UX-020A)")
    verify(compact.indexOf("root.resetCountdown") >= 0,
           "the compact row needs its reset column")
    verify(win.indexOf('text: "23h 1m"') >= 0,
           "the reset column is measured with TextMetrics, never hardcoded px")
  }

  // §7: critical paints the numeral and the fill in Color.urgent; nothing
  // else in this file may introduce a colour.
  function test_critical_window_uses_the_urgent_token() {
    var win = read("assets/omarchy/components/UsageWindow.qml")
    verify(win.indexOf("Color.urgent") >= 0)
    verify(win.indexOf('root.severity === "critical"') >= 0)
    verify(win.indexOf("Qt.rgba(") < 0)
  }

  function test_provider_view_leads_with_one_window() {
    var view = read("assets/omarchy/ProviderView.qml")
    verify(view.indexOf("Core.windowLayout(") >= 0)
    verify(view.indexOf("emphasis: true") >= 0)
    verify(view.indexOf("emphasis: false") >= 0)
    verify(view.indexOf("severityText: ") >= 0)
    // The quiet rule between lead and rows is gone (approved mockup).
    verify(view.indexOf("strength: 0.08") < 0)
  }
```

- [ ] **Step 2: Run the QML suite — confirm the five new asserts fail**

- [ ] **Step 3: Rewrite `assets/omarchy/components/UsageWindow.qml`**

Full file:

```qml
import QtQuick
import qs.Commons

// One normalized percentage window. The elected lead renders large (label
// line with the promoted reset -> 2.5x numeral + unit -> track); every other
// window renders as a compact row that carries its own track (UX-020A
// extended by the visual design §6). Severity paints numeral and fill in
// Color.urgent (§7) and always travels with a word — see Accessible.name.
Item {
  id: root

  property string label: ""
  property string percentText: "—"
  // 0–100 when known; negative when unavailable (hide fill).
  property real percent: -1
  // Countdown only: the popup shows no absolute clock (§6).
  property string resetCountdown: ""
  property string resetPhrase: ""
  property string unitText: "left"
  // "critical" | "warning" | "" — computed from usedPercent by CoreView.
  property string severity: ""
  // The elected lead renders large; every other window renders compact.
  property bool emphasis: true
  property bool dimmed: false
  property color foreground: Color.foreground
  property color accent: Color.accent
  property string fontFamily: Style.font.family

  readonly property bool hasPercent: root.percent >= 0 && root.percent <= 100
  readonly property real fillRatio: hasPercent
      ? Math.max(0, Math.min(1, root.percent / 100))
      : 0
  readonly property bool isCritical: root.severity === "critical"
  readonly property color valueColor: root.isCritical ? Color.urgent : root.foreground
  readonly property color fillColor: root.dimmed
      ? root.foreground
      : (root.isCritical ? Color.urgent : root.accent)
  readonly property real fillOpacity: root.dimmed ? 0.45 : (root.isCritical ? 1.0 : 0.9)
  // Data surface, not control chrome — no host token covers it. Declared
  // once here; both layouts tint from the same place.
  readonly property color trackColor: Util.alpha(root.foreground, 0.12)

  width: parent ? parent.width : implicitWidth
  implicitHeight: root.emphasis ? bigCol.implicitHeight : compactRow.implicitHeight
  height: implicitHeight
  opacity: root.dimmed ? 0.6 : 1.0

  // §6: the compact reset column is sized for the widest countdown the
  // humaniser produces below 24 hours, and the value column for a full 100%.
  // Never a hardcoded pixel: both scale with [font] base-size.
  TextMetrics {
    id: countdownMetrics
    font.family: root.fontFamily
    font.pixelSize: Style.font.caption
    text: "23h 1m"
  }

  TextMetrics {
    id: valueMetrics
    font.family: root.fontFamily
    font.pixelSize: Style.font.bodySmall
    text: "100%"
  }

  Column {
    id: bigCol
    visible: root.emphasis
    width: parent.width
    spacing: Style.spacing.sm

    // Label line with the reset promoted into it: the label and the lead-in
    // recede, the countdown itself keeps full ink. Not uppercased — this line
    // is now a sentence, not a kicker.
    Row {
      width: parent.width
      spacing: Style.spacing.sm

      Text {
        id: leadLabel
        width: Math.min(implicitWidth,
                        Math.max(0, parent.width - leadReset.implicitWidth - Style.spacing.sm))
        text: root.resetPhrase.length > 0
            ? root.label + " · " + root.resetPhrase
            : root.label
        color: Util.alpha(root.foreground, 0.72)
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        elide: Text.ElideRight
        textFormat: Text.PlainText
        Accessible.ignored: true
      }

      Text {
        id: leadReset
        text: root.resetCountdown
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.bodySmall
        textFormat: Text.PlainText
        Accessible.ignored: true
      }
    }

    Row {
      spacing: Style.spacing.md

      Text {
        id: bigNumeral
        text: root.percentText
        color: root.valueColor
        font.family: root.fontFamily
        font.pixelSize: Math.round(Style.font.body * 2.5)
        font.bold: true
        textFormat: Text.PlainText
        Accessible.ignored: true
      }

      Text {
        anchors.baseline: bigNumeral.baseline
        text: root.unitText
        color: Util.alpha(root.foreground, 0.72)
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        textFormat: Text.PlainText
        Accessible.ignored: true
      }
    }

    Rectangle {
      width: parent.width
      height: Style.spacing.md
      radius: height / 2
      color: root.trackColor
      Accessible.ignored: true

      Rectangle {
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: Math.max(root.hasPercent && root.fillRatio > 0 ? Style.spacing.md : 0,
                        parent.width * root.fillRatio)
        height: parent.height
        radius: parent.radius
        color: root.fillColor
        opacity: root.fillOpacity
        visible: root.hasPercent && root.fillRatio > 0
      }
    }
  }

  Row {
    id: compactRow
    visible: !root.emphasis
    width: parent.width
    spacing: Style.spacing.lg

    Text {
      id: compactLabel
      anchors.verticalCenter: parent.verticalCenter
      width: Math.max(0, Math.round(parent.width * 0.25))
      text: root.label
      color: Util.alpha(root.foreground, 0.72)
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      elide: Text.ElideRight
      textFormat: Text.PlainText
      Accessible.ignored: true
    }

    Rectangle {
      id: compactTrack
      anchors.verticalCenter: parent.verticalCenter
      width: Math.max(0, parent.width
                         - compactLabel.width
                         - compactValue.width
                         - compactReset.width
                         - Style.spacing.lg * 3)
      height: Style.spacing.sm
      radius: height / 2
      color: root.trackColor
      Accessible.ignored: true

      Rectangle {
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: Math.max(root.hasPercent && root.fillRatio > 0 ? Style.spacing.sm : 0,
                        parent.width * root.fillRatio)
        height: parent.height
        radius: parent.radius
        color: root.fillColor
        opacity: root.fillOpacity
        visible: root.hasPercent && root.fillRatio > 0
      }
    }

    Text {
      id: compactValue
      anchors.verticalCenter: parent.verticalCenter
      width: valueMetrics.advanceWidth
      horizontalAlignment: Text.AlignRight
      text: root.percentText
      color: root.valueColor
      font.family: root.fontFamily
      font.pixelSize: Style.font.bodySmall
      font.bold: true
      textFormat: Text.PlainText
      Accessible.ignored: true
    }

    Text {
      id: compactReset
      anchors.verticalCenter: parent.verticalCenter
      width: countdownMetrics.advanceWidth
      horizontalAlignment: Text.AlignRight
      text: root.resetCountdown
      color: Util.alpha(root.foreground, 0.55)
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      textFormat: Text.PlainText
      Accessible.ignored: true
    }
  }

  // A11Y-012: severity reaches assistive tech as a word, in both layouts.
  Accessible.name: {
    var parts = [root.label, root.percentText + " " + root.unitText]
    if (root.severity === "critical")
      parts.push("critical")
    else if (root.severity === "warning")
      parts.push("low")
    if (root.resetCountdown.length > 0)
      parts.push(root.resetPhrase + " " + root.resetCountdown)
    return parts.join(", ")
  }
  Accessible.role: Accessible.StaticText
}
```

Two runtime traps this file walks past deliberately: `compactTrack.width` depends on its siblings' widths, which depend only on `TextMetrics` and on `parent.width`, so there is no binding cycle; and `anchors.verticalCenter` inside a `Row` is legal because `Row` owns `x` only.

- [ ] **Step 4: Rewrite the window section of `ProviderView.qml`**

Replace the `groups`/`windows` binding (`:43`) with:

```qml
  readonly property var windows: Core.windowLayout(provider, displayMetric, nowMs)
  readonly property string severity: Core.providerSeverity(provider)
```

Pass severity into the header instance (`:64-76`), keeping every existing binding:

```qml
      severityText: Core.severityTagText(root.severity)
      severityUrgent: root.severity === "critical"
```

Replace both window `Column`s (`:146-202`) with one:

```qml
    // §3.4/§8: one elected lead window rendered large, every other window as
    // a compact row in delivered order. No rule between them — the size
    // difference is the hierarchy (approved mockup).
    Column {
      width: parent.width
      spacing: Style.spacing.xxxl
      visible: root.mode === "windows" || root.mode === "stale_windows"

      Repeater {
        model: root.windows.lead ? [root.windows.lead] : []
        UsageWindow {
          required property var modelData
          width: parent.width
          label: modelData.label
          percentText: modelData.percentText
          percent: modelData.percent !== undefined && modelData.percent !== null
              ? Number(modelData.percent) : -1
          resetCountdown: modelData.resetCountdown ? modelData.resetCountdown : ""
          resetPhrase: modelData.resetPhrase ? modelData.resetPhrase : ""
          severity: modelData.severity ? modelData.severity : ""
          unitText: root.unitText
          emphasis: true
          dimmed: root.isStale
          foreground: root.foreground
          accent: root.accentColor
          fontFamily: root.fontFamily
        }
      }

      Column {
        width: parent.width
        spacing: Style.spacing.xl
        visible: root.windows.rest.length > 0

        Repeater {
          model: root.windows.rest
          UsageWindow {
            required property var modelData
            width: parent.width
            label: modelData.label
            percentText: modelData.percentText
            percent: modelData.percent !== undefined && modelData.percent !== null
                ? Number(modelData.percent) : -1
            resetCountdown: modelData.resetCountdown ? modelData.resetCountdown : ""
            resetPhrase: modelData.resetPhrase ? modelData.resetPhrase : ""
            severity: modelData.severity ? modelData.severity : ""
            unitText: root.unitText
            emphasis: false
            dimmed: root.isStale
            foreground: root.foreground
            accent: root.accentColor
            fontFamily: root.fontFamily
          }
        }
      }
    }
```

The `PanelSeparator { strength: 0.08 }` that used to separate primary from secondary windows is deleted with the old structure. The full-width separator under the header (`:78-81`) stays — `tst_Popup.test_full_width_separator_present` covers it. Update the file's header comment (`:7-11`) to describe lead/compact instead of primary/secondary.

- [ ] **Step 5: Run the full QML gate**

Same three commands as Task 3 Step 5. Expected clean, 0 failed. Pay attention to `qmllint` warnings about unused ids — a leftover `groups` reference is exactly the class of defect the three gates cannot see.

- [ ] **Step 6: Amend the specification**

In `docs/specs/v10/04-quickshell-ux-and-accessibility.md`, replace `UX-020A` (`:66-67`) and add two new requirements immediately after it:

```markdown
- `UX-020A`: Every percentage window row shows a horizontal usage track
  filled by the displayed metric (used or remaining), in both the lead window
  and the compact rows, so secondary windows stay comparable.
- `UX-020C`: Severity is computed from `usedPercent`, independent of the
  displayed metric, using the notification thresholds: at or above 95 the
  window is Critical, at or above 90 it is Warning. The popup header shows a
  severity tag reading `Critical` or `Low` when the provider has one, the
  critical lead window renders its numeral and track in the urgent theme
  colour, and a ready provider with a critical window shows the `!` cue on its
  bar chip. Every level carries a word; no level is colour-only.
- `UX-020D`: The popup renders exactly one lead window, elected
  deterministically: a critical window wins, and among criticals the one with
  the lowest remaining percentage; otherwise the window whose reset comes
  soonest; ties keep the delivered order; when no window has a future reset
  the first delivered window leads. Every other window renders as a compact
  row in delivered order. Reset times render as a countdown, in hours below
  24 hours.
```

Then re-run the docs gate: `cargo test --test active_docs` (internal Markdown links and doc contracts) plus `cargo test --test active_language`.

- [ ] **Step 7: Full checkpoint**

```bash
cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings && git diff --check
```

Expected 292/16, clean.

- [ ] **Step 8: Commit**

```bash
git add assets/omarchy/components/UsageWindow.qml assets/omarchy/ProviderView.qml \
  docs/specs/v10/04-quickshell-ux-and-accessibility.md tests/qml/tst_Popup.qml
git commit -m "feat: lead one window and track every row"
```

---

### Task 6: Screenshot mirror, docs sweep, full checkpoint, execution record

**Files:**
- Modify: `tests/qml/tst_Screenshots.qml` (`paintState` :99-179, `test_required_names_match_spec` :208-214)
- Modify: `tests/qml/TestPalette.js` (`requiredScreenshotNames` :5-24)
- Modify: `scripts/verify-v10-ui` (`REQUIRED` :9-26)
- Modify: `README.md` (:34), `docs/architecture.md` (:49-52)
- Modify: `docs/superpowers/plans/2026-07-31-v11-04-severity-and-lead-window.md` (this file)

**Interfaces:**
- Consumes: everything from Tasks 1–5. Produces no shipped-code interface.

- [ ] **Step 1: Add the critical capture to both inventory lists**

In `tests/qml/TestPalette.js`, add `"critical-dark.png"` immediately after `"stale-dark.png"`. In `scripts/verify-v10-ui`, add the bare name `critical-dark.png` at the same position in `REQUIRED=( … )`. Run `cargo test --test screenshot_inventory` — it passes only when both lists agree, and fails loudly if only one was edited.

- [ ] **Step 2: Sync the mirror panel and its count**

In `tests/qml/tst_Screenshots.qml`, change `compare(names.length, 16)` to `compare(names.length, 17)` in `test_required_names_match_spec` and add the capture branch inside `paintState`, immediately after the `stale-dark` branch:

```qml
    } else if (name.indexOf("critical-dark") === 0) {
      stage.titleText = "Claude"
      stage.badgeText = "CRITICAL"
      stage.bodyText = "Session (5h) · resets in 41m · 3% left · Weekly (7d) 60% · 23h 1m"
```

Update the three `ready-*` bodies (light, white, dark) from `"Session (5h) 58% left · Max plan"` to the shipped shape:

```qml
      stage.bodyText = "Session (5h) · resets in 3h 1m · 58% left · Weekly (7d) 60% · 23h 1m"
```

and the `refreshing-with-data-dark` body from `"Weekly (7d) 74% left (prior data kept)"` to
`"Weekly (7d) · resets in 23h 1m · 74% left (prior data kept)"`.

Leave `stale-dark`'s `badgeText = "STALE"` as it is: the shipped header tag uppercases its text (`Font.AllUppercase`), so the mirror's all-caps badge is now consistent with the product rather than the taste outlier plan 03 flagged. Record that in the execution record instead of changing it.

- [ ] **Step 3: Run the screenshot gate**

```bash
scripts/verify-v10-ui
```

Expected: `verify-v10-ui: 17 PNGs ok → …/SHA256SUMS`, exit 0.

- [ ] **Step 4: Docs sweep**

`README.md:34` and `docs/architecture.md:49` still call the rail "bordered" — plan 03 deleted the rail's own frame and its sweep missed both lines. Fix them together with the lead-window sentence:

- `README.md:34-36` becomes:

```markdown
The popup has a vertical icon rail, one provider view at a time, one lead
percentage window with every other window as a compact row, a usage track on
every row, content-fit height, overflow-only scrolling, complete keyboard
navigation, and active Omarchy theme tokens.
```

- `docs/architecture.md:49-52` becomes:

```markdown
The consolidated popup uses an icon rail (providers + Settings), a
content-fit card height, overflow-gated vertical scrolling, one lead
percentage window, and compact rows with a progress track. Widgets do not own
polling, provider state, settings persistence, or cache.
```

Then sweep every canonical doc for the same strings, the way plan 03's review caught two files outside its own list:

```bash
rg -n -i 'bordered (vertical )?icon rail|primary window' README.md docs/ --glob '!docs/releases/**' --glob '!docs/superpowers/**'
```

`docs/releases/*.md` is a historical record and is never rewritten.

- [ ] **Step 5: Hygiene greps (all must return nothing)**

```bash
rg -n 'PRIMARY_WINDOW_IDS|windowGroups|formatResetText|WEEKDAYS' assets src tests scripts
rg -n 'Qt\.darker|Qt\.rgba' assets/omarchy
rg -n 'resetText' assets/omarchy
rg -n $'⌛' assets/omarchy
rg -n 'groups\.(primary|secondary)' assets tests
```

- [ ] **Step 6: Full checkpoint**

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
git diff --check
find assets/omarchy -type f -name '*.qml' -exec qmllint -I /usr/share/omarchy/shell {} +
omarchy plugin validate assets/omarchy
QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
  /usr/lib/qt6/bin/qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
scripts/verify-v10-ui
```

Expected: Rust **292 / 16 suites**, QML 0 failed, everything else clean. No GPU probe is needed — this plan touches no shader or tint path; `MultiEffect` is untouched. The perceptual gate is the owner's live QA, exactly as in plans 01–03.

- [ ] **Step 7: Execution record**

Append an `## Execution record` section to this file in the shape plans 01–03 use: commit list with task mapping, final test counts, what the plan got wrong, deferred minors carried forward. Then:

```bash
git add docs/superpowers/plans/2026-07-31-v11-04-severity-and-lead-window.md \
  tests/qml/tst_Screenshots.qml tests/qml/TestPalette.js scripts/verify-v10-ui \
  README.md docs/architecture.md
git commit -m "docs: record execution outcome in plan"
```

---

## Done when

- `qmltestrunner` 0 failed, with these named tests printing `PASS`: `test_severity_level_thresholds`, `test_severity_tag_words`, `test_provider_severity_is_the_worst_window`, `test_severity_ignores_the_display_metric`, the six `test_lead_election_*`, `test_single_window_leads_with_no_rest`, `test_no_windows_means_no_lead`, `test_unknown_window_id_can_lead`, `test_lines_carry_raw_percentages_and_countdown`, the six `test_reset_countdown_*`/`test_reset_phrase_follows_the_countdown`, `test_primary_window_allowlist_is_gone`, `test_absolute_clock_humaniser_is_gone`, `test_chip_severity_urgent_only_when_ready_and_critical`, `test_chip_cue_label_is_a_word`, `test_chip_source_binds_severity`, `test_header_renders_plan_and_severity_tags`, `test_header_spacer_accounts_for_both_tags`, `test_lead_window_geometry_and_label_line`, `test_compact_rows_carry_their_own_track`, `test_critical_window_uses_the_urgent_token`, `test_provider_view_leads_with_one_window`.
- `cargo test` green at **292 across 16 suites**, including the new `severity_parity` suite; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `git diff --check` clean.
- `qmllint`, `omarchy plugin validate assets/omarchy`, and `scripts/verify-v10-ui` (17 PNGs) all pass.
- Every hygiene grep in Task 6 Step 5 returns nothing: no `PRIMARY_WINDOW_IDS`, no `windowGroups`, no `formatResetText`, no `WEEKDAYS`, no `resetText`, no `Qt.darker`/`Qt.rgba` under `assets/omarchy`.
- Changing either threshold on one side alone fails `cargo test --test severity_parity` (verify by hand once, then revert).
- `UX-020A` extended and `UX-020C`/`UX-020D` added; `UX-017` and `UX-028` untouched (plan 03 already amended them).
- Nothing installed into `~/.config/omarchy/plugins/` — live QA remains the owner's gate.

## Not in this plan

| Plan | Covers |
| --- | --- |
| 05 — notifications and CLI | Rust `countdownText` equivalent + QML equivalence test (copy design §6.1), notification display-metric threading and copy (§6.2, §5.5), CLI and `install.sh` copy, `agent-bar` → `Agent Bar` in maintenance copy |

Known deferrals, deliberately not touched here:

- `ProviderHeader.showStale` is set by `ProviderView` and never read by the header — dead since plan 03 moved stale into the banner. Removing it also touches `headerModel` and its test, which is churn this plan does not need; it belongs with whoever next edits `headerModel`.
- The popup rebuilds every window delegate on each 30-second `nowMs` tick, because `windowLayout` returns fresh objects. Pre-existing behaviour, unchanged here.

## Execution record

Executed 2026-07-31 on branch `feat/v11-foundation`, task order 1 → 2 → 3 → 4
→ 5 → 6, 7 implementation/fix commits across Tasks 1–5 plus this record:
`7d4a004` (Task 1), `74314ee` (Task 2), `4700250` (Task 3), `e339ebc` (Task 4
implement), `d003377` (Task 4 fix round 1), `5d67382` (Task 5 implement),
`1f89936` (Task 5 fix round 1). Tasks 4 and 5 each needed exactly one fix
round; Tasks 1, 2, and 3 went clean on first review. Task 6 produced no
product-behavior commit — screenshot mirror sync, docs sweep, full checkpoint,
and this record only.

Final state: `cargo test` 292 passed across 16 suites, including the
`severity_parity` suite added in Task 2 (unchanged by Task 6 — the new
`critical-dark` capture extends an existing test's body, not a new test
function). `qmltestrunner` 228 passed / 0 failed (also unchanged in count for
the same reason). `cargo fmt --check`, `cargo clippy --all-targets -- -D
warnings`, and `git diff --check` all clean. `qmllint -I
/usr/share/omarchy/shell` over every `assets/omarchy/**/*.qml` file: 0 errors
(367 unqualified-access/unresolved-import warnings, 126 info notes —
pre-existing `qs.Commons`/`qs.Ui` noise across the whole plugin tree, see
"What the plan got wrong" §4, not a regression from this plan). `omarchy
plugin validate assets/omarchy` clean. `scripts/verify-v10-ui`:
`verify-v10-ui: 17 PNGs ok → …/SHA256SUMS`, exit 0 (up from 16 — this task's
new `critical-dark.png`). No GPU probe was run — this plan touches no shader
or tint path; `MultiEffect` is untouched.

Every Step 5 hygiene grep returned nothing beyond the self-referential guard
assertions in `tests/qml/tst_Popup.qml`: `test_primary_window_allowlist_is_gone`
and `test_absolute_clock_humaniser_is_gone` must name `PRIMARY_WINDOW_IDS`,
`windowGroups`, `formatResetText`, and `WEEKDAYS` as string literals to assert
their absence from `CoreView.js`, and the same test names `groups.primary`/
`groups.secondary` to assert their absence from `ProviderView.qml` — those are
the only matches, all inside the guard tests themselves, zero in `assets`,
`src`, or `scripts` outside `tests/`. `Qt.darker`/`Qt.rgba`, `resetText`, and
`⌛` under `assets/omarchy` returned nothing at all.

Per Step 2's ruling: `stale-dark`'s all-caps `badgeText = "STALE"` in the
screenshot mirror is left as it was. The shipped header tag uppercases its
text (`Font.AllUppercase`), so the mirror's all-caps badge is now consistent
with the product rather than the taste outlier plan 03 flagged — the product
caught up to the mirror, not the other way around.

### What the plan got wrong

Four defects surfaced during execution, all confirmed and resolved by the
controller; a fifth surfaced later, in the final whole-branch review:

1. The plan's own sample comment for `electLeadIndex` (Task 1) named the
   literal string `PRIMARY_WINDOW_IDS` — the exact identifier Task 1's own
   Step 3 deletion guard (`test_primary_window_allowlist_is_gone`) bans from
   the file outright. The plan contradicted itself before a single line was
   implemented. Caught during Task 1's implementation and worded around with
   zero functional change, which is why Task 1 needed no separate fix round.
   Same shape as plan 02's stray-glyph-in-a-comment defect: a literal
   surviving inside a comment, not a rendering claim, propagating forward
   into a later guard test.
2. The header spacer formula (Task 4, `ProviderHeader.qml`) charged three
   inter-child gaps for a `Row` of five children (name label, plan tag,
   severity tag, flexible spacer, refresh button) that renders four gaps
   between consecutive visible items once both tags are visible. The
   corrected trailing term is `row.spacing * 2`, covering the two gaps that
   exist unconditionally — before the spacer and after it — on top of the
   two gaps already charged conditionally per visible tag. This lands right
   for zero, one, and two visible tags. Fixed in `d003377`.
3. `countdownMetrics` (Task 5) was specified against the mockup's sample data
   `23h 1m`, not the widest countdown string the shipped format function
   actually renders. `countdownText` pads no digits, so the widest sub-24h
   string is `23h 59m` — one character wider — and the compact row's reset
   column was sized one monospace character too narrow. The plan's own test
   encoded the same `23h 1m` literal, so it could not have caught the gap by
   construction; it surfaced only by checking the format function's actual
   output range against the column width, not the plan's sample value.
   Fixed in `1f89936`.
4. A measured verification gap, not a code defect. `/usr/bin/qmllint` is a
   stub that reports version "1.0" and stays completely silent even on a
   file that contains an undefined type. The real binary,
   `/usr/lib/qt6/bin/qmllint` 6.11.1, does not share that silent-stub defect,
   but it also cannot resolve the `qs.*` module namespace (`qs.Commons`,
   `qs.Ui`) that every plugin QML file imports — so this task's run, like
   every prior plan's run, emits only unresolved-import and "Unqualified
   access" warnings across every file under `assets/omarchy`, including
   files this plan never touched. In effect, the `qmllint` line in the
   repository's documented verification gate type-checks nothing for plugin
   QML; `qmltestrunner`, `omarchy plugin validate`, and the owner's live QA
   carry that weight instead. This is a finding recorded for the owner to
   decide on — no change was made to `CLAUDE.md` or the v10 spec here.
5. The plan authored `valueColor` and `fillColor` in the same Task 5 step
   (`UsageWindow.qml`) with opposite treatments of `dimmed`: `valueColor`
   ignored it and stayed on `isCritical` alone, while `fillColor` gave
   `dimmed` precedence over `isCritical`. A stale window whose last reading
   was critical therefore rendered an urgent numeral beside a plain,
   non-urgent track — the two halves of the same window disagreeing about
   its own severity. No gate in this plan could catch it: the QML tests read
   the file as text and never instantiate the real components, and the
   screenshot mirror has no capture that is both stale and critical at once.
   It surfaced only in the final whole-branch review, reading the two
   `readonly property color` lines side by side. The rule the fix settles
   on: in the popup, severity describes the numbers currently on screen, so
   severity outranks dimming; `dimmed` only chooses between the accent and
   the plain foreground for a window that is not critical. Fixed in the
   whole-branch review's single fix commit for this plan.

### Deferred minors carried forward

None blocking; triaged here for whoever next touches these files.

1. `test_lead_election_ties_keep_delivered_order` uses ids that also sort
   alphabetically, so it cannot distinguish an index tiebreak from an id
   tiebreak.
2. `HeaderTag.qml` is deliberately outside `tst_Tokens.convertedFiles()`, so
   no test requires it to call `Util.alpha(`.
3. `leadReset` stays a visible `Row` child with empty text when a window has
   no reset, reserving one `Style.spacing.sm` of trailing dead space.
4. Below roughly 120–130px of content width the compact row's fixed columns
   can overlap once the track floors to zero, which the popup's
   `Style.space(540)` content width makes unreachable in practice.
5. `ProviderHeader.showStale` is set by `ProviderView` but never read, dead
   since an earlier plan moved staleness into the banner.

## Open question for the owner

- The popup header shows the severity tag whenever the provider has one,
  including while the provider is stale, so a stale account whose last
  reading was critical shows an urgent `CRITICAL` tag above the stale
  banner. The bar chip deliberately does the opposite: `chipSeverityUrgent`
  requires `state === "ready"`, because the chip has a single cue slot that
  the state already owns. The two surfaces differ on purpose under the rule
  above — the popup has a dedicated tag slot with no collision to arbitrate,
  and it renders the actual last-known numbers next to the tag, while the
  chip's one slot must choose between showing state and showing severity. If
  the owner prefers the header to go quiet while stale instead, the change
  is to gate `severityText`/`severityUrgent` in `ProviderView.qml` on
  `provider.state === "ready"`, the same way the chip is gated. Not
  implemented here.
