# Amp Subscription Adaptive Lead Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the chip and popup lead with the Amp subscription's most constrained bucket for subscriber accounts, while free-only accounts stay byte-identical.

**Architecture:** One new rule in `electLeadIndex` (plan windows outrank non-plan when nothing is critical), the chip switches from `windows[0]` to the same elected lead, and the Rust mapper renames two labels. Approved spec: `docs/superpowers/specs/2026-08-07-amp-subscription-lead-window-design.md`.

**Tech Stack:** Rust (parser labels + tests), QML/JS (`CoreView.js` pure functions, qmltestrunner Qt 6 binary path only).

## Global Constraints

- Contract: `CLAUDE.md`; product contract: `docs/specs/v10/` plus the approved design above. Status schema stays frozen at v2 — no new field.
- No monetary data: the `Individual credits: $` line stays unparsed (JSON-022B).
- All shipped UI copy is English. Window ids `plan-other`/`plan-orb` do not change; only labels do.
- Commits require owner authorization. The owner authorizes execution of this plan; each task ends in exactly one commit with an English Conventional Commit subject ≤ 50 chars, no AI-attribution text anywhere.
- Rust gates (Tasks 1): `cargo fmt --check` · `cargo test` · `cargo clippy --all-targets -- -D warnings` · `git diff --check`.
- QML gates (Tasks 2–3): the Rust gates plus
  ```bash
  find assets/omarchy -type f -name '*.qml' -exec \
    /usr/lib/qt6/bin/qmllint -I /usr/share/omarchy/shell {} +
  omarchy plugin validate assets/omarchy
  QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
    /usr/lib/qt6/bin/qmltestrunner -input tests/qml \
    -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
  ```
  `qmllint` from PATH is a silent stub and `qmltestrunner` from PATH is Qt 5 — the `/usr/lib/qt6/bin/` paths and both env vars are mandatory. The qmltestrunner bar is **0 failed**; never chase an exact total.
- The three QML gates are blind to a dangling reference in plugin QML (nothing compiles files importing `qs.*`). Every deletion gets a hand-trace of references **and** a text guard banning the exact dead identifier.
- Known flake: `binary_interactive_update_rejects_non_tty` (`ExecutableFileBusy` on a machine with a live plugin) — retry once; pre-existing isolation bug, not a regression.
- Preserve unrelated worktree changes. Never bypass hooks, force-push, merge, tag, or publish.

## File Structure

- Modify: `src/providers/v2_map.rs` — two label strings (`:93-94`) and their test assertions (`:879`, `:882`).
- Modify: `assets/omarchy/CoreView.js` — plan rule in `electLeadIndex` (after the critical loop, `:625-660` region); `chipPercentText` (`:98-106`) reads the elected lead; `primaryWindow` (`:91-95`) deleted.
- Modify: `tests/qml/tst_ProviderStates.qml` — three new election tests next to the existing `test_lead_election_*` family (`:211+`).
- Modify: `tests/qml/tst_BarWidget.qml` — subscriber chip test next to `test_used_versus_remaining_metric` (`:218`).
- Modify: `tests/qml/tst_Popup.qml` — extend the dead-identifier guard `test_primary_window_allowlist_is_gone` (`:299-310`).
- Modify: `docs/specs/v10/04-quickshell-ux-and-accessibility.md` — amend `UX-002` (`:13-14`) and `UX-020D` (`:78-84`).

---

### Task 1: Rust label rename (`Plan · agent` / `Plan · orbs`)

**Files:**
- Modify: `src/providers/v2_map.rs:93-94` (labels), `:879`, `:882` (test assertions)

**Interfaces:**
- Consumes: nothing.
- Produces: `amp_from_usage_text` emits windows labeled `Plan · agent` (id `plan-other`) and `Plan · orbs` (id `plan-orb`). Tasks 2–3 use these labels only as opaque test data; ids are unchanged.

- [ ] **Step 1: Make the test expect the new labels**

In `src/providers/v2_map.rs`, test `amp_subscription_fixture_emits_plan_windows_and_plan` (`:871-893`), change the two assertions:

```rust
                assert_eq!(windows[1].label(), "Plan · agent");
```
and
```rust
                assert_eq!(windows[2].label(), "Plan · orbs");
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test amp_subscription_fixture_emits_plan_windows_and_plan`
Expected: FAIL — left `"Plan · other"`, right `"Plan · agent"`.

- [ ] **Step 3: Rename the labels in the mapper**

In `amp_from_usage_text` (`src/providers/v2_map.rs:92-95`), the tuple array becomes:

```rust
        for (idx, id, label) in [
            (2usize, "plan-other", "Plan · agent"),
            (3usize, "plan-orb", "Plan · orbs"),
        ] {
```

The comment above the `Subscription` regex (`:81-84`) already explains that "other usage" is included agent usage; extend its last sentence so the label choice is documented:

```rust
    // line is monetary and intentionally never parsed into a window (JSON-022B).
    // Labels render the meaning, not the CLI word: "other" is included agent
    // usage, "orb" is included orb-hours (design 2026-08-07).
```

- [ ] **Step 4: Run the Rust gates**

Run: `cargo test amp` then `cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings && git diff --check`
Expected: all pass, 0 failures (retry the known flake once if it trips).

- [ ] **Step 5: Commit**

```bash
git add src/providers/v2_map.rs
git commit -m "feat: rename amp plan labels to agent/orbs"
```

---

### Task 2: Plan-window rule in lead election

**Files:**
- Modify: `assets/omarchy/CoreView.js` (`electLeadIndex`, insert between the critical loop and the nearest-reset loop)
- Test: `tests/qml/tst_ProviderStates.qml` (after `test_lead_election_lowest_remaining_among_criticals`, `:233`)

**Interfaces:**
- Consumes: `layoutOf(windows, nowIso)` helper (`tst_ProviderStates.qml:170-172`), `remainingRank(line)` (`CoreView.js:612-614`), line objects from `windowDisplayLines` whose `id` is always a string (`String(w.id || ("w" + i))`).
- Produces: `electLeadIndex(lines)` rule order: critical → plan (`id` prefix `plan-`, lowest `remainingPercent`) → nearest future reset → first delivered. Task 3's chip relies on this exact order.

- [ ] **Step 1: Write the failing tests**

Add to `tests/qml/tst_ProviderStates.qml` after `test_lead_election_lowest_remaining_among_criticals`:

```qml
  // UX-020D step 2 (amended 2026-08-07): plan windows outrank non-plan
  // windows when nothing is critical, so a subscriber leads with the
  // subscription instead of the daily free window's nearer reset.
  function test_lead_election_plan_beats_nearest_reset() {
    var layout = layoutOf([
      { id: "daily", label: "Daily (1d)", usedPercent: 31, remainingPercent: 69,
        resetsAt: "2026-08-08T00:00:00Z" },
      { id: "plan-other", label: "Plan · agent", usedPercent: 8, remainingPercent: 92,
        resetsAt: null },
      { id: "plan-orb", label: "Plan · orbs", usedPercent: 0, remainingPercent: 100,
        resetsAt: null }
    ], "2026-08-07T15:00:00Z")
    compare(layout.lead.id, "plan-other")
    // The rest keeps delivered order: free first, then the healthier bucket.
    compare(layout.rest.length, 2)
    compare(layout.rest[0].id, "daily")
    compare(layout.rest[1].id, "plan-orb")
  }

  function test_lead_election_lowest_remaining_among_plan_windows() {
    var layout = layoutOf([
      { id: "plan-other", label: "Plan · agent", usedPercent: 20, remainingPercent: 80,
        resetsAt: null },
      { id: "plan-orb", label: "Plan · orbs", usedPercent: 65, remainingPercent: 35,
        resetsAt: null }
    ], "2026-08-07T15:00:00Z")
    compare(layout.lead.id, "plan-orb")
  }

  // A critical plan window leads through rule 1 (severity), not rule 2 —
  // same lead either way, but the severity tag must say Critical.
  function test_lead_election_critical_plan_leads_by_severity() {
    var layout = layoutOf([
      { id: "daily", label: "Daily (1d)", usedPercent: 31, remainingPercent: 69,
        resetsAt: "2026-08-08T00:00:00Z" },
      { id: "plan-other", label: "Plan · agent", usedPercent: 97, remainingPercent: 3,
        resetsAt: null },
      { id: "plan-orb", label: "Plan · orbs", usedPercent: 0, remainingPercent: 100,
        resetsAt: null }
    ], "2026-08-07T15:00:00Z")
    compare(layout.lead.id, "plan-other")
    compare(layout.lead.severity, "critical")
  }

  // Severity is the one signal plan preference never displaces: an exhausted
  // free window still takes the lead over a healthy subscription.
  function test_lead_election_critical_free_beats_healthy_plan() {
    var layout = layoutOf([
      { id: "daily", label: "Daily (1d)", usedPercent: 96, remainingPercent: 4,
        resetsAt: "2026-08-08T00:00:00Z" },
      { id: "plan-other", label: "Plan · agent", usedPercent: 8, remainingPercent: 92,
        resetsAt: null }
    ], "2026-08-07T15:00:00Z")
    compare(layout.lead.id, "daily")
    compare(layout.lead.severity, "critical")
  }
```

- [ ] **Step 2: Run the QML tests to verify the new ones fail**

Run the qmltestrunner line from Global Constraints.
Expected: `test_lead_election_plan_beats_nearest_reset` and
`test_lead_election_lowest_remaining_among_plan_windows` FAIL (the old
election picks `daily` / `plan-other` by delivered order).
`test_lead_election_critical_plan_leads_by_severity` and
`test_lead_election_critical_free_beats_healthy_plan` already PASS — they
pin rule 1 as a regression guard. Every pre-existing test still passes.

- [ ] **Step 3: Implement the plan rule**

In `assets/omarchy/CoreView.js`, `electLeadIndex`: after the critical loop's `if (best >= 0) return best` and before the `// 2. Otherwise the nearest reset…` comment, insert (and renumber the two comments below it to 3. and 4.):

```js
  // 2. UX-020D (amended 2026-08-07): a plan window (id prefix "plan-")
  //    outranks every non-plan window; among plan windows the lowest
  //    remaining leads. Ids are typed schema data authored by the Rust
  //    mappers, so the prefix is a contract, not raw-output parsing.
  for (i = 0; i < lines.length; i++) {
    if (lines[i].id.indexOf("plan-") !== 0)
      continue
    if (best < 0 || remainingRank(lines[i]) < remainingRank(lines[best]))
      best = i
  }
  if (best >= 0)
    return best
```

Also update the function's header comment (`// §8: deterministic lead election…`) to mention the four-step order.

- [ ] **Step 4: Run the QML gates**

Run: qmllint line, `omarchy plugin validate assets/omarchy`, qmltestrunner line (all from Global Constraints), then `git diff --check`.
Expected: 0 failed; the three new tests print PASS.

- [ ] **Step 5: Commit**

```bash
git add assets/omarchy/CoreView.js tests/qml/tst_ProviderStates.qml
git commit -m "feat: plan windows outrank free in election"
```

---

### Task 3: Chip renders the elected lead window

**Files:**
- Modify: `assets/omarchy/CoreView.js` (`primaryWindow` `:91-95` deleted, `chipPercentText` `:97-106` rewritten)
- Test: `tests/qml/tst_BarWidget.qml` (after `test_used_versus_remaining_metric`), `tests/qml/tst_Popup.qml` (`test_primary_window_allowlist_is_gone`)

**Interfaces:**
- Consumes: `windowDisplayLines(provider, metric, nowMs)` and `electLeadIndex(lines)` from Task 2. `windowDisplayLines` already handles array-like QVariantList windows and non-finite percentages (`percentText` falls back to `\u2014`).
- Produces: `chipPercentText(provider, metric)` returns the elected lead's `percentText`. `chipNumeralText` and `chipAccessibleLabel` compose it unchanged. `primaryWindow` no longer exists anywhere in the plugin.

- [ ] **Step 1: Write the failing chip test**

Add to `tests/qml/tst_BarWidget.qml` after `test_used_versus_remaining_metric`:

```qml
  // UX-002 (amended 2026-08-07): the chip shows the elected lead window —
  // for a subscriber that is the subscription bucket, not windows[0]
  // (Amp Free), even though the free window has the nearer reset.
  function test_chip_shows_elected_lead_for_subscriber() {
    var p = {
      id: "amp",
      name: "Amp",
      state: "ready",
      windows: [
        { id: "daily", label: "Daily (1d)", usedPercent: 31, remainingPercent: 69,
          resetsAt: "2099-01-01T00:00:00Z" },
        { id: "plan-other", label: "Plan · agent", usedPercent: 8, remainingPercent: 92 },
        { id: "plan-orb", label: "Plan · orbs", usedPercent: 0, remainingPercent: 100 }
      ]
    }
    compare(Core.chipPercentText(p, "remaining"), "92%")
    compare(Core.chipPercentText(p, "used"), "8%")
  }
```

- [ ] **Step 2: Extend the dead-identifier guard**

In `tests/qml/tst_Popup.qml`, `test_primary_window_allowlist_is_gone`, after the `windowGroups` verify:

```qml
    verify(core.indexOf("primaryWindow") < 0,
           "primaryWindow is replaced by the elected lead window")
```

- [ ] **Step 3: Run the QML tests to verify both fail**

Run the qmltestrunner line.
Expected: `test_chip_shows_elected_lead_for_subscriber` FAILS (`69%` vs `92%`) and `test_primary_window_allowlist_is_gone` FAILS (identifier still present). Everything else passes.

- [ ] **Step 4: Rewrite the chip source and delete `primaryWindow`**

In `assets/omarchy/CoreView.js`, delete the `primaryWindow` function (`:91-95`) and replace `chipPercentText`:

```js
// UX-002 / UX-032A (amended 2026-08-07): the chip renders the elected lead
// window's used|remaining percent — the same election the popup runs — or an
// em-dash when there is no window. Chip and popup can never disagree on
// which window a number belongs to.
function chipPercentText(provider, metric) {
  var lines = windowDisplayLines(provider, metric, undefined)
  var lead = electLeadIndex(lines)
  if (lead < 0)
    return "\u2014"
  return lines[lead].percentText
}
```

Hand-trace: `primaryWindow` had exactly one caller (`chipPercentText`, verified by `rg -n "primaryWindow" assets tests` before deleting). `chipAccessibleLabel` (`:253`) and `chipNumeralText` (`:235`) call `chipPercentText` and need no edit.

- [ ] **Step 5: Run the QML gates**

Run: qmllint line, `omarchy plugin validate assets/omarchy`, qmltestrunner line, `git diff --check`.
Expected: 0 failed. Pre-existing chip tests (`test_empty_windows_render_em_dash`, `test_used_versus_remaining_metric`, `test_array_like_windows_render_percent`) still pass: a single window elects itself and empty windows yield `lead < 0`.

- [ ] **Step 6: Commit**

```bash
git add assets/omarchy/CoreView.js tests/qml/tst_BarWidget.qml tests/qml/tst_Popup.qml
git commit -m "feat: chip renders elected lead window"
```

---

### Task 4: Amend UX-002 and UX-020D

**Files:**
- Modify: `docs/specs/v10/04-quickshell-ux-and-accessibility.md:13-14` (`UX-002`), `:78-84` (`UX-020D`)

**Interfaces:**
- Consumes: the behavior shipped by Tasks 2–3.
- Produces: the product contract Tasks 2–3 already conform to; no code reads this file.

- [ ] **Step 1: Amend `UX-002`**

Replace lines 13–14:

```markdown
- `UX-002`: A chip contains the provider icon and the configured used or
  remaining percentage of the elected lead window (per `UX-020D`), so the
  chip and the popup always name the same number.
```

- [ ] **Step 2: Amend `UX-020D`**

Replace the requirement so the election reads:

```markdown
- `UX-020D`: The popup renders exactly one lead window, elected
  deterministically: a critical window wins, and among criticals the one with
  the lowest remaining percentage; otherwise a plan window (window id
  starting `plan-`) wins, and among plan windows the one with the lowest
  remaining percentage; otherwise the window whose reset comes soonest; ties
  keep the delivered order; when no window has a future reset the first
  delivered window leads. Every other window renders as a compact row in
  delivered order. Reset times render as a countdown, in hours below 24
  hours.
```

- [ ] **Step 3: Verify**

Run: `git diff --check`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add docs/specs/v10/04-quickshell-ux-and-accessibility.md
git commit -m "docs: amend UX-002/UX-020D for plan lead"
```

---

## Final checkpoint (after Task 4)

Run every gate from Global Constraints once more, end to end (Rust + QML). Also commit the approved design doc if the owner authorizes:

```bash
git add docs/superpowers/specs/2026-08-07-amp-subscription-lead-window-design.md \
        docs/superpowers/plans/2026-08-07-amp-subscription-lead-window.md
git commit -m "docs: amp subscription lead window design"
```

Live QA (only if the owner asks): with the plugin installed, a Megawatt account's Amp chip shows the subscription bucket percentage and the popup leads with `Plan · agent`, with `Daily (1d)` as a compact row keeping its countdown.
