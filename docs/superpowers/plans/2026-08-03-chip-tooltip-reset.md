# Chip Tooltip Window and Reset Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hovering a bar chip answers when its window resets, on a second line
that names the window, without touching the popup.

**Architecture:** `CoreView.chipTooltip` gains two optional parameters (`nowMs`,
`localeTimeFormat`) and composes a second line from `primaryWindow(provider)`
using the reset humanisers the popup already uses. `BarWidget` supplies the
clock and the locale format, refreshing `nowMs` the moment a chip is hovered —
the host copies `tooltipText` by value inside `WidgetButton.onEntered`, so the
string only has to be correct at that instant.

**Tech Stack:** QML (Qt 6) and plain ES5-style JavaScript in `.js` files loaded
by QML. No Rust change, no status-schema change, no new dependency.

**Spec:** `docs/superpowers/specs/2026-08-03-chip-tooltip-reset-design.md`

## Global Constraints

- Rust/Cargo and QML only. No Node, npm, Bun, pnpm, Yarn, ts-node, or Deno.
- QML never parses raw provider output; external strings render as plain text.
- `CoreView.js` must never contain the literal `"HH:mm"` or `"hh:mm"` — a guard
  in `tests/qml/tst_Popup.qml:327-328` enforces that time formats come from the
  caller's locale.
- `assets/omarchy/ProviderView.qml` must keep exactly one `resetClock:` binding
  site — guard in `tests/qml/tst_Popup.qml:338`. This plan does not touch it.
- Commit subjects are English Conventional Commits of at most 50 characters.
- No AI attribution in any commit message or body.
- Every checkpoint runs, from the repository root:

  ```bash
  cargo fmt --check
  cargo test
  cargo clippy --all-targets -- -D warnings
  git diff --check
  ```

- QML changes additionally run, with these exact binary paths — the `PATH`
  `qmllint` is a stub that stays silent, and the `PATH` `qmltestrunner` is Qt5
  and fails silently:

  ```bash
  find assets/omarchy -type f -name '*.qml' -exec \
    /usr/lib/qt6/bin/qmllint -I /usr/share/omarchy/shell {} +
  omarchy plugin validate assets/omarchy
  QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
    /usr/lib/qt6/bin/qmltestrunner \
    -input tests/qml \
    -import /usr/share/omarchy/shell \
    -import assets/omarchy \
    -o -,txt
  ```

  `qmllint` exits 0 while printing warnings and cannot resolve `qs.*`, so every
  plugin QML file emits unresolved-import noise. Read it for what your change
  introduced; never treat its exit code as a verdict.

## File Structure

| File | Responsibility after this plan |
| --- | --- |
| `assets/omarchy/CoreView.js` | Adds `chipWindowLine` (pure composition of the second line) next to `chipTooltip`, which gains the two optional parameters. No new humaniser: `resetCountdownText`, `resetPhrase`, `resetClockText` and `plainText` are reused verbatim. |
| `assets/omarchy/BarWidget.qml` | Owns `tooltipNowMs` and `shortTimeFormat`, refreshes the clock on chip hover, passes both into `chipTooltip`. |
| `tests/qml/tst_BarWidget.qml` | Copy-contract cases, the no-filler and newline-injection cases, the hover-freshness case, and two source guards. |
| `docs/specs/v10/04-quickshell-ux-and-accessibility.md` | `UX-011` stops claiming reset detail lives only in the popup. |

`ProviderChip.qml` is deliberately absent: the chip already forwards
`tooltipText` to the host untouched and needs no change.

## Constraint discovered while planning — read before Task 2

The design says the hover-ordering assumption "gets an explicit test". The
obvious form of that test — instantiate the real `ProviderChip` and hover it —
**is impossible in this runner**. `ProviderChip.qml` is built on `qs.Ui`'s
`WidgetButton` and `tests/qml/tst_BarWidget.qml:598-604` already records why:
`qs.Commons`/`qs.Ui` do not resolve in the pure Qt 6 test runner, which is why
the file carries a hand-written `ProviderChipHost` stand-in.

Task 2 therefore splits the assumption into two things that *are* testable, and
neither is optional:

1. **The Qt ordering itself** — `ProviderChipHost` grows a real `MouseArea`
   shaped exactly like `WidgetButton`'s (hover-enabled, `tooltipHovered` derived
   from `containsMouse`, `onEntered` pushing `tooltipText` by value). Hovering it
   with `mouseMove` exercises the genuine Qt signal order, because that order is
   Qt behaviour, not Omarchy behaviour.
2. **That the replica still matches the host, and that our widget uses the
   pattern** — two source guards, one over
   `/usr/share/omarchy/shell/Ui/WidgetButton.qml` and one over
   `BarWidget.qml`. The first is what fails loudly if an Omarchy upgrade stops
   snapshotting the text; without it the replica would be self-confirming.

If Step 4 of Task 2 fails — the hover pushes a stale countdown — do not iterate
on the handler. Fall back to the design's documented alternative: a `Timer` in
`BarWidget` with `interval: 15000` and `running` bound to any chip being
hovered, keeping the hover-start refresh as well. Report the failure before
switching.

---

### Task 1: Second line in `chipTooltip`

**Files:**
- Modify: `assets/omarchy/CoreView.js:248-264` (the `chipTooltip` block)
- Modify: `docs/specs/v10/04-quickshell-ux-and-accessibility.md:24-27` (`UX-011`)
- Test: `tests/qml/tst_BarWidget.qml` (add after `test_chip_tooltip_humanized`,
  which ends at line 332)

**Interfaces:**
- Consumes: existing `primaryWindow(provider)`, `chipPercentText(provider,
  metric)`, `stateQualifier(state)`, `plainText(value)`,
  `resetCountdownText(iso, nowMs) -> "" | "now" | "3h 1m"`,
  `resetPhrase(countdown) -> "" | "resets" | "resets in"`,
  `resetClockText(iso, localeTimeFormat) -> "" | "(13:31)"`.
- Produces: `chipTooltip(provider, metric, nowMs, localeTimeFormat) -> string`
  — `nowMs` defaults to `Date.now()` when `undefined`, `localeTimeFormat`
  defaults to `""` (no clock). Also `chipWindowLine(provider, nowMs,
  localeTimeFormat) -> string`, the second line alone, `""` when the chip's
  window has nothing to say. Task 2 calls only `chipTooltip`.

- [ ] **Step 1: Write the failing tests**

Add both functions to `tests/qml/tst_BarWidget.qml`, immediately after
`test_chip_tooltip_humanized`. Do not modify that existing function: its
fixtures carry unlabelled, unresettable windows, so it doubles as the
regression that one-line tooltips stay one line.

Every expected clock is composed through `Core.resetClockText`, never written
out. `Qt.formatTime` renders in the machine's local zone, so a hardcoded
`(13:31)` would pass only in one timezone. The formatting itself is already
covered by `tests/qml/tst_Popup.qml:323-336`; these cases prove composition.

```qml
  function test_chip_tooltip_carries_window_and_reset() {
    var nowMs = Date.parse("2026-08-03T10:30:00Z")

    // Reset today: label, countdown and clock all land on line 2.
    var todayIso = "2026-08-03T13:31:00Z"
    var todayClock = Core.resetClockText(todayIso, "HH:mm")
    var today = { name: "Claude", state: "ready",
                  windows: [{ id: "session", label: "Session (5h)",
                              usedPercent: 95, remainingPercent: 5,
                              resetsAt: todayIso }] }
    compare(Core.chipTooltip(today, "remaining", nowMs, "HH:mm"),
            "Claude · 5%\nSession (5h) · resets in 3h 1m " + todayClock)

    // Distance never suppresses the clock (design decision 3).
    var farIso = "2026-08-09T14:34:00Z"
    var farClock = Core.resetClockText(farIso, "HH:mm")
    var far = { name: "Codex", state: "ready",
                windows: [{ id: "weekly", label: "Weekly (7d)",
                            usedPercent: 98, remainingPercent: 2,
                            resetsAt: farIso }] }
    compare(Core.chipTooltip(far, "remaining", nowMs, "HH:mm"),
            "Codex · 2%\nWeekly (7d) · resets in 6d 4h " + farClock)

    // An elapsed reset speaks the popup's phrase and drops the clock.
    var elapsed = { name: "Claude", state: "ready",
                    windows: [{ id: "session", label: "Session (5h)",
                                usedPercent: 4, remainingPercent: 96,
                                resetsAt: "2026-08-03T10:00:00Z" }] }
    compare(Core.chipTooltip(elapsed, "remaining", nowMs, "HH:mm"),
            "Claude · 96%\nSession (5h) · resets now")

    // The qualifier stays on line 1, and the window keeps its reset — this is
    // the most valuable hover in the product: when does work resume.
    var limitedIso = "2026-08-03T11:11:00Z"
    var limitedClock = Core.resetClockText(limitedIso, "HH:mm")
    var limited = { name: "Grok", state: "rate_limited",
                    windows: [{ id: "daily", label: "Daily (1d)",
                                usedPercent: 100, remainingPercent: 0,
                                resetsAt: limitedIso }] }
    compare(Core.chipTooltip(limited, "remaining", nowMs, "HH:mm"),
            "Grok · 0% · rate limited\nDaily (1d) · resets in 41m "
            + limitedClock)

    // Stale keeps the cached window: resetsAt is an absolute instant, and
    // staleness devalues the percentage, never the timestamp.
    var stale = { name: "Claude", state: "stale",
                  windows: [{ id: "session", label: "Session (5h)",
                              usedPercent: 95, remainingPercent: 5,
                              resetsAt: todayIso }] }
    compare(Core.chipTooltip(stale, "remaining", nowMs, "HH:mm"),
            "Claude · 5% · stale\nSession (5h) · resets in 3h 1m " + todayClock)

    // A window with no resetsAt says only what it is.
    var noReset = { name: "Amp", state: "ready",
                    windows: [{ id: "context", label: "Context",
                                usedPercent: 95, remainingPercent: 5 }] }
    compare(Core.chipTooltip(noReset, "remaining", nowMs, "HH:mm"),
            "Amp · 5%\nContext")

    // No locale format: the countdown survives, the clock does not.
    compare(Core.chipTooltip(today, "remaining", nowMs, ""),
            "Claude · 5%\nSession (5h) · resets in 3h 1m")

    // Omitted nowMs falls back to the wall clock without throwing.
    verify(Core.chipTooltip(today, "remaining").indexOf("Claude · 5%") === 0)
  }

  function test_chip_tooltip_window_line_is_earned_not_filled() {
    var nowMs = Date.parse("2026-08-03T10:30:00Z")

    // Neither label nor reset: nothing to say. No second line, and no
    // "Window" filler — this is what keeps every one-line state one line.
    var bare = { name: "Claude", state: "ready",
                 windows: [{ usedPercent: 4, remainingPercent: 96 }] }
    compare(Core.chipTooltip(bare, "remaining", nowMs, "HH:mm"), "Claude · 96%")

    // Unlabelled but resettable: the reset stands alone, no leading separator.
    var unlabelledIso = "2026-08-03T13:31:00Z"
    var unlabelledClock = Core.resetClockText(unlabelledIso, "HH:mm")
    var unlabelled = { name: "Claude", state: "ready",
                       windows: [{ usedPercent: 4, remainingPercent: 96,
                                   resetsAt: unlabelledIso }] }
    compare(Core.chipTooltip(unlabelled, "remaining", nowMs, "HH:mm"),
            "Claude · 96%\nresets in 3h 1m " + unlabelledClock)

    // plainText spares U+000A on purpose, so a label carried in provider
    // payload could forge a whole tooltip line. Exactly one newline may exist.
    var forged = { name: "Claude", state: "ready",
                   windows: [{ id: "session", label: "Session\nresets in 0m",
                               usedPercent: 4, remainingPercent: 96 }] }
    var tip = Core.chipTooltip(forged, "remaining", nowMs, "HH:mm")
    compare(tip.split("\n").length, 2)
    compare(tip, "Claude · 96%\nSession resets in 0m")

    // No provider at all stays empty, as before.
    compare(Core.chipTooltip(null, "remaining", nowMs, "HH:mm"), "")
  }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:

```bash
QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
  /usr/lib/qt6/bin/qmltestrunner \
  -input tests/qml \
  -import /usr/share/omarchy/shell \
  -import assets/omarchy \
  -o -,txt 2>&1 | grep -A3 tooltip
```

Expected: `test_chip_tooltip_carries_window_and_reset` FAILS with `Actual
(): Claude · 5%` against `Expected (): Claude · 5%\nSession (5h) · ...` —
the second line is simply absent. `test_chip_tooltip_window_line_is_earned_not_filled`
fails only on the `unlabelled` and `forged` cases; its `bare` and `null` cases
already pass, which is expected and correct.

- [ ] **Step 3: Write the implementation**

In `assets/omarchy/CoreView.js`, replace the whole `chipTooltip` block (lines
248-264, comment included) with the following. Leave every other function
alone, and keep the literal `"—"` on the percentage comparison exactly as it is
today — the file uses the character, not an escape, on that line.

```js
// The chip's own window, humanised for the tooltip's second line: the label,
// the reset, or both. Empty when the window has neither, because an
// unlabelled window with no reset has nothing to say and "Window" filler is
// not an answer — that emptiness is what keeps signed-out, no-CLI, loading
// and windowless-ready tooltips at exactly one line.
//
// plainText strips control characters but spares U+000A by design, so a label
// carried in provider payload (Codex derives window identity from its own
// response) could forge an entire tooltip line. The newline collapse below is
// the only thing preventing that, and tst_BarWidget proves it.
function chipWindowLine(provider, nowMs, localeTimeFormat) {
  var w = primaryWindow(provider)
  if (!w)
    return ""
  var parts = []
  var label = plainText(w.label || w.id || "").replace(/[\r\n]+/g, " ").trim()
  if (label.length)
    parts.push(label)
  var iso = w.resetsAt ? String(w.resetsAt) : ""
  var countdown = iso.length ? resetCountdownText(iso, nowMs) : ""
  if (countdown.length) {
    // resetPhrase already owns the "resets" / "resets in" distinction, and an
    // elapsed reset takes no clock — the same rule the popup lead follows.
    var clock = countdown === "now" ? "" : resetClockText(iso, localeTimeFormat)
    parts.push(clock.length
        ? resetPhrase(countdown) + " " + countdown + " " + clock
        : resetPhrase(countdown) + " " + countdown)
  }
  return parts.join(" · ")
}

// UX-011 (amended 2026-08-03): line 1 is the provider, the displayed
// percentage when one exists, and a plain-language qualifier when not ready —
// the raw enum value never renders (copy design §5.4). Line 2 describes the
// window that produced the numeral beside it, so the two can never disagree.
// The host copies this string by value in WidgetButton.onEntered, so nowMs
// only has to be fresh at the moment of hover; BarWidget owns that.
function chipTooltip(provider, metric, nowMs, localeTimeFormat) {
  if (!provider)
    return ""
  var name = provider.name ? String(provider.name) : providerDisplayName(provider.id)
  var parts = [name]
  var state = provider.state ? String(provider.state) : "unknown"
  var pct = chipPercentText(provider, metric)
  if (pct !== "—" || state === "ready")
    parts.push(pct)
  var qualifier = stateQualifier(state)
  if (qualifier.length)
    parts.push(qualifier)
  var head = parts.join(" · ")
  var windowLine = chipWindowLine(provider,
                                  nowMs === undefined ? Date.now() : nowMs,
                                  localeTimeFormat)
  return windowLine.length ? head + "\n" + windowLine : head
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run the full runner command from Step 2 without the `grep`. Expected: the whole
`tests/qml` suite passes, including the untouched `test_chip_tooltip_humanized`
and the `tst_Popup.qml` guards on `"HH:mm"` and `resetClock:`.

- [ ] **Step 5: Amend the UX-011 contract sentence**

`docs/specs/v10/04-quickshell-ux-and-accessibility.md` lines 24-27 currently
read:

```markdown
- `UX-011`: Tooltip copy includes the provider name, the displayed
  percentage when one exists, and a plain-language state qualifier when the
  provider is not ready. Raw state identifiers never render. Reset detail
  lives in the popup.
```

Replace the final sentence so the contract matches shipped behaviour. The v10
specification is the product contract, so leaving it asserting the opposite is
not acceptable:

```markdown
- `UX-011`: Tooltip copy includes the provider name, the displayed
  percentage when one exists, and a plain-language state qualifier when the
  provider is not ready. Raw state identifiers never render. Reset detail is
  limited to the chip's own window; every other window lives in the popup.
```

Change nothing else in that file.

- [ ] **Step 6: Run the checkpoint**

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
git diff --check
find assets/omarchy -type f -name '*.qml' -exec \
  /usr/lib/qt6/bin/qmllint -I /usr/share/omarchy/shell {} +
omarchy plugin validate assets/omarchy
```

`cargo test` includes `tests/active_language.rs`; the amended `UX-011` sentence
is English, so it stays green.

- [ ] **Step 7: Commit**

```bash
git add assets/omarchy/CoreView.js tests/qml/tst_BarWidget.qml \
        docs/specs/v10/04-quickshell-ux-and-accessibility.md
git commit -m "feat: chip tooltip names window and reset"
```

---

### Task 2: Fresh clock at hover time

**Files:**
- Modify: `assets/omarchy/BarWidget.qml:33-34` (add properties after
  `chipFontFamily`) and `assets/omarchy/BarWidget.qml:98` (the `tooltipText`
  binding, inside the `ProviderChip` block at lines 87-108)
- Test: `tests/qml/tst_BarWidget.qml` — extend the `ProviderChipHost` component
  at lines 597-637, add one property near `chipUrl` (line 23), add three test
  functions

**Interfaces:**
- Consumes: Task 1's `chipTooltip(provider, metric, nowMs, localeTimeFormat)`.
- Produces: nothing other tasks depend on. This is the last task.

- [ ] **Step 1: Extend the chip stand-in with the host's hover shape**

In `tests/qml/tst_BarWidget.qml`, inside `component ProviderChipHost`, add the
following after the `property var registeredBar: null` line. Keep every
existing property, signal and function in that component untouched.

```qml
    // Hover shape copied from qs.Ui's WidgetButton: tooltipHovered derives
    // from containsMouse, and onEntered hands the host a *copy* of the text.
    // The real ProviderChip cannot be instantiated here (qs.Ui does not
    // resolve in this runner), so this replica is what exercises the Qt
    // signal order; test_host_still_snapshots_the_tooltip_text is what keeps
    // the replica honest against the installed host.
    width: 40
    height: 20
    property double tooltipNowMs: 0
    readonly property bool tooltipHovered: hoverArea.containsMouse

    onTooltipHoveredChanged: {
      if (chipRoot.tooltipHovered)
        chipRoot.tooltipNowMs = Date.now()
    }

    MouseArea {
      id: hoverArea
      anchors.fill: parent
      hoverEnabled: true
      onEntered: {
        if (chipRoot.bar && typeof chipRoot.bar.showTooltip === "function")
          chipRoot.bar.showTooltip(chipRoot, chipRoot.tooltipText)
      }
      onExited: {
        if (chipRoot.bar && typeof chipRoot.bar.hideTooltip === "function")
          chipRoot.bar.hideTooltip(chipRoot)
      }
    }
```

Then add this property next to `chipUrl` at line 23:

```qml
  property string widgetButtonUrl: "file:///usr/share/omarchy/shell/Ui/WidgetButton.qml"
```

- [ ] **Step 2: Write the failing tests**

Add these three functions after `test_provider_chip_trigger_press_emits_pressed`
(which ends around line 455).

```qml
  function test_tooltip_snapshot_is_fresh_at_hover() {
    // A reset a decade out gives a four-digit countdown; measured from the
    // epoch the same reset gives five digits. That gap is the probe: it
    // proves the refresh ran before the host copied the string, without
    // restating the binding back to itself.
    var provider = { name: "Claude", state: "ready",
                     windows: [{ id: "session", label: "Session (5h)",
                                 usedPercent: 4, remainingPercent: 96,
                                 resetsAt: "2036-01-01T00:00:00Z" }] }
    var chip = providerChipComp.createObject(testCase, { bar: fakeBar })
    verify(chip !== null)
    chip.tooltipText = Qt.binding(function () {
      return Core.chipTooltip(provider, "remaining", chip.tooltipNowMs, "HH:mm")
    })

    chip.tooltipNowMs = 0
    verify(/\d{5}d/.test(chip.tooltipText),
           "epoch baseline must be five-digit days, got: " + chip.tooltipText)

    fakeBar.lastTooltip = ""
    mouseMove(chip, 5, 5)
    verify(fakeBar.lastTooltip.length > 0, "hover must push a tooltip")
    verify(!/\d{5}d/.test(fakeBar.lastTooltip),
           "host read a stale clock: " + fakeBar.lastTooltip)
    verify(fakeBar.lastTooltip.indexOf("Session (5h) · resets in") >= 0,
           "got: " + fakeBar.lastTooltip)

    chip.destroy()
    wait(0)
  }

  function test_bar_widget_wires_the_tooltip_clock() {
    var widget = sourceAt(widgetUrl)
    verify(widget.indexOf("property double tooltipNowMs") >= 0)
    verify(widget.indexOf("Qt.locale().timeFormat(Locale.ShortFormat)") >= 0,
           "the clock format is the host locale's, never a literal")
    verify(widget.indexOf("onTooltipHoveredChanged") >= 0,
           "nowMs must refresh before the host snapshots the text")
    verify(widget.indexOf("root.tooltipNowMs = Date.now()") >= 0)
    verify(widget.indexOf("root.shortTimeFormat") >= 0,
           "the locale format must reach chipTooltip")
  }

  function test_host_still_snapshots_the_tooltip_text() {
    // If an Omarchy upgrade changes either of these, the replica above stops
    // representing the host and the freshness strategy needs rethinking.
    var host = sourceAt(widgetButtonUrl)
    verify(host.length > 0, "host WidgetButton.qml must be readable")
    verify(host.indexOf("root.bar.showTooltip(root, root.tooltipText)") >= 0,
           "host still copies the text by value inside onEntered")
    verify(host.indexOf("mouseArea.containsMouse") >= 0,
           "tooltipHovered still derives from containsMouse")
  }
```

- [ ] **Step 3: Run the tests to verify they fail**

Run:

```bash
QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
  /usr/lib/qt6/bin/qmltestrunner \
  -input tests/qml \
  -import /usr/share/omarchy/shell \
  -import assets/omarchy \
  -o -,txt 2>&1 | grep -E 'tooltip|FAIL|PASS'
```

Expected: `test_bar_widget_wires_the_tooltip_clock` FAILS on the first
`verify` — `BarWidget.qml` has no `tooltipNowMs` yet.
`test_tooltip_snapshot_is_fresh_at_hover` and
`test_host_still_snapshots_the_tooltip_text` should already PASS, since Step 1
added the replica and the host file is installed. If
`test_tooltip_snapshot_is_fresh_at_hover` fails here, stop: the Qt ordering
assumption is wrong and the fallback in "Constraint discovered while planning"
applies.

- [ ] **Step 4: Write the implementation**

In `assets/omarchy/BarWidget.qml`, add after line 34 (`chipFontFamily`):

```qml
  // The host copies tooltipText by value inside WidgetButton.onEntered and
  // renders that copy 400ms later, so there is no live countdown to keep
  // ticking — the string only has to be right at the instant of hover.
  // MouseArea emits containsMouseChanged (which drives tooltipHovered) before
  // it emits entered(), so refreshing from the chip's handler lands before
  // the host reads the property. tst_BarWidget proves that order rather than
  // trusting it. The initial value is the wall clock so a chip that is
  // somehow read before any hover still shows a sane countdown.
  property double tooltipNowMs: Date.now()
  readonly property string shortTimeFormat: Qt.locale().timeFormat(Locale.ShortFormat)
```

Then replace line 98 inside the `ProviderChip` block:

```qml
        tooltipText: Core.chipTooltip(modelData, root.displayMetric)
```

with:

```qml
        tooltipText: Core.chipTooltip(modelData, root.displayMetric,
                                      root.tooltipNowMs, root.shortTimeFormat)

        onTooltipHoveredChanged: {
          if (chip.tooltipHovered)
            root.tooltipNowMs = Date.now()
        }
```

`chip` is the existing `id` on that `ProviderChip`, and `tooltipHovered` is the
host's readonly property — declaring a handler for its change signal at the use
site is allowed and does not shadow it.

- [ ] **Step 5: Run the tests to verify they pass**

Run the full runner command from Step 3 without the `grep`. Expected: every
`tests/qml` test passes.

- [ ] **Step 6: Run the checkpoint**

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
git diff --check
find assets/omarchy -type f -name '*.qml' -exec \
  /usr/lib/qt6/bin/qmllint -I /usr/share/omarchy/shell {} +
omarchy plugin validate assets/omarchy
```

Read the `qmllint` output for anything naming `BarWidget.qml` and
`tooltipNowMs`, `shortTimeFormat` or `Locale` specifically; ignore the standing
`qs.*` unresolved-import and unqualified-access noise, which every plugin file
emits regardless of this change.

- [ ] **Step 7: Commit**

```bash
git add assets/omarchy/BarWidget.qml tests/qml/tst_BarWidget.qml
git commit -m "feat: refresh tooltip clock at hover time"
```

- [ ] **Step 8: Report what is and is not verified**

Both tasks are green on automated tests. Automated tests cannot show the
tooltip rendering two lines on a real bar — `PanelToolTip`'s `contentItem` is a
single `Text` and nothing in this repository renders it. State plainly that the
work is implemented and unit-verified but not perceptually verified, and offer
the live check rather than claiming it is done:

```bash
omarchy plugin rescan
```

then hover a chip. Never run `omarchy-refresh-shell` — it resets `shell.json`.
A restart of the shell is the safe refresh.

## Self-Review

**Spec coverage.** Copy contract → Task 1 Step 1, one case per row.
Composition rules → Task 1 Step 3, all four numbered rules. Newline forging →
Task 1 Steps 1 and 3. Freshness → Task 2 Steps 1-4. Contract amendment →
Task 1 Step 5. Out-of-scope items are absent from every task, as intended: no
file under the popup appears in any `git add`.

**One spec statement is corrected here.** The spec says existing cases in
`test_chip_tooltip_humanized` "gain real labels". They must not: their windows
carry neither label nor `resetsAt`, so under the no-filler rule they stay
one-line and become the regression proving it. The plan leaves them untouched.

**A second spec statement is narrowed.** The spec's "explicit test" for hover
ordering cannot instantiate the real chip. The substitute and its limits are
documented in "Constraint discovered while planning".

**Placeholders.** None: every step carries the literal code, the exact command,
and the expected output.

**Type consistency.** `chipWindowLine` and `chipTooltip` are spelled
identically in Task 1's interface block, implementation and tests.
`tooltipNowMs` and `shortTimeFormat` are spelled identically in Task 2's
implementation, source guard and replica.
