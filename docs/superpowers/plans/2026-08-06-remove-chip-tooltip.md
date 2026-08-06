# Remove Bar-Chip Hover Tooltip Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop rendering the hover tooltip on bar chips while preserving the chip's accessible name, with zero visual change to the bar.

**Architecture:** The Quattro host renders a tooltip only when a `WidgetButton` sets `tooltipText`, so removal means the chip stops setting it. `Core.chipTooltip` (two lines, live clock) collapses into `Core.chipAccessibleLabel` (former first line, no clock), consumed by a new `accessibleLabel` property on `ProviderChip` that feeds `Accessible.name`. Popup tooltips are untouched.

**Tech Stack:** QML (Quickshell/Quattro plugin), plain-JS view model (`CoreView.js`), qmltestrunner (Qt6). No Rust changes.

**Spec:** `docs/superpowers/specs/2026-08-06-remove-chip-tooltip-design.md`

## Global Constraints

- Rust/Cargo and QML only; no Node or npm tooling.
- Commit subjects: English Conventional Commits, at most 50 characters, no AI attribution anywhere.
- QML never parses raw provider output; render external strings as plain text.
- Bar visuals must not change: icon, numeral, state cues (`󰅐` / `!`), dimming, click routing.
- Popup tooltips (Refresh, Settings gear, reorder arrows) must keep working.
- Verification must use the Qt6 binaries by absolute path: `/usr/lib/qt6/bin/qmllint` and `/usr/lib/qt6/bin/qmltestrunner` (the PATH versions fail silently).
- Shared time helpers `resetCountdownText`, `resetClockText`, `resetPhrase` in `CoreView.js` stay — the popup uses them.
- The `fakeBar` test double in `tst_BarWidget.qml` replicates the host bar API (`showTooltip`, `lastTooltip`, click targets). Leave it intact: the host still calls `bar.showTooltip` on hover; only our text becomes empty.

---

### Task 1: Remove the tooltip wiring, keep the accessible label

**Files:**
- Modify: `assets/omarchy/CoreView.js:248-302` (delete `chipWindowLine` + `chipTooltip`, add `chipAccessibleLabel`)
- Modify: `assets/omarchy/BarWidget.qml:36-45,109-115` (drop tooltip clock and binding)
- Modify: `assets/omarchy/components/ProviderChip.qml:7-9,14-22,94` (new property, new `Accessible.name`)
- Test: `tests/qml/tst_BarWidget.qml`

**Interfaces:**
- Consumes: `chipPercentText(provider, metric)`, `stateQualifier(state)`, `providerDisplayName(id)` — already in `CoreView.js`, unchanged.
- Produces: `Core.chipAccessibleLabel(provider, metric) -> string` (single line, `"<name> · <pct> · <qualifier>"`, empty for null provider) and `ProviderChip.accessibleLabel: string`. Task 2 has no code dependency on these.

- [x] **Step 1: Rewrite the tooltip tests as accessible-label tests**

In `tests/qml/tst_BarWidget.qml`:

**Delete** these five functions entirely (they prove behaviour that is being removed):
- `test_chip_tooltip_carries_window_and_reset` (lines ~335-403, including the comment block above it)
- `test_chip_tooltip_window_line_is_earned_not_filled` (lines ~405-434)
- `test_tooltip_snapshot_is_fresh_at_hover` (lines ~561-594)
- `test_bar_widget_wires_the_tooltip_clock` (lines ~596-606)
- `test_host_still_snapshots_the_tooltip_text` (lines ~608-617, including its comment)

**Replace** `test_chip_tooltip_humanized` (lines ~310-333) with:

```qml
  function test_chip_accessible_label_humanized() {
    var ready = { name: "Claude", state: "ready",
                  windows: [{ usedPercent: 4, remainingPercent: 96 }] }
    compare(Core.chipAccessibleLabel(ready, "remaining"), "Claude · 96%")

    var signedOut = { name: "Claude", state: "unauthenticated", windows: [] }
    compare(Core.chipAccessibleLabel(signedOut, "remaining"),
            "Claude · signed out")

    var rateLimited = { name: "Codex", state: "rate_limited",
                        windows: [{ usedPercent: 98, remainingPercent: 2 }] }
    compare(Core.chipAccessibleLabel(rateLimited, "used"),
            "Codex · 98% · rate limited")

    var noCli = { name: "Grok", state: "cli_missing", windows: [] }
    compare(Core.chipAccessibleLabel(noCli, "remaining"), "Grok · no CLI")

    var failed = { name: "Amp", state: "provider_error", windows: [] }
    compare(Core.chipAccessibleLabel(failed, "remaining"), "Amp · failed")

    var emptyReady = { name: "Claude", state: "ready", windows: [] }
    compare(Core.chipAccessibleLabel(emptyReady, "remaining"), "Claude · —")

    var loading = { name: "Claude", state: "loading", windows: [] }
    compare(Core.chipAccessibleLabel(loading, "remaining"), "Claude · loading")

    var stale = { name: "Claude", state: "stale",
                  windows: [{ usedPercent: 95, remainingPercent: 5 }] }
    compare(Core.chipAccessibleLabel(stale, "remaining"), "Claude · 5% · stale")

    // Single-line by construction; no provider stays empty.
    compare(Core.chipAccessibleLabel(null, "remaining"), "")
  }
```

**Add** this source guard (in place of the two deleted wiring tests, near the other `sourceAt` guards):

```qml
  function test_bar_widget_renders_no_tooltip() {
    var widget = sourceAt(widgetUrl)
    verify(widget.indexOf("tooltipText") < 0,
           "the bar chip must never feed the host tooltip")
    verify(widget.indexOf("tooltipNowMs") < 0,
           "the tooltip clock died with the tooltip")
    verify(widget.indexOf("chipAccessibleLabel") >= 0,
           "the accessible label must still reach the chip")
    var chip = sourceAt(chipUrl)
    verify(chip.indexOf("property string accessibleLabel") >= 0)
    verify(chip.indexOf("Accessible.name: root.accessibleLabel") >= 0)
    verify(chip.indexOf("tooltipText") < 0,
           "the chip must not reference the host tooltip property")
  }
```

**Edit** `test_provider_chip_registers_and_unregisters` (line ~535): change the
`createObject` property `tooltipText: "Claude · 90% · ready"` to
`accessibleLabel: "Claude · 90% · ready"`.

- [x] **Step 2: Run the suite to verify the intended failures**

Run:
```bash
QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
  /usr/lib/qt6/bin/qmltestrunner \
  -input tests/qml/tst_BarWidget.qml \
  -import /usr/share/omarchy/shell \
  -import assets/omarchy \
  -o -,txt
```
Expected: FAIL — `test_chip_accessible_label_humanized` (chipAccessibleLabel is not a function) and `test_bar_widget_renders_no_tooltip` (BarWidget still contains `tooltipText`/`tooltipNowMs`).

- [x] **Step 3: Replace `chipTooltip`/`chipWindowLine` in CoreView.js**

In `assets/omarchy/CoreView.js`, delete lines 248-302 — the `chipWindowLine`
comment block and function, and the `chipTooltip` comment block and function —
and put this in their place:

```js
// UX-011 (superseded 2026-08-06): the chip renders no hover tooltip. The
// former tooltip's first line survives as the chip's accessible name — the
// provider, the displayed percentage when one exists, and a plain-language
// qualifier when not ready. The raw enum value never renders (copy design
// §5.4). Single-line by construction: no window detail, no live clock.
function chipAccessibleLabel(provider, metric) {
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
  return parts.join(" · ")
}
```

(The head-building logic is copied verbatim from the old `chipTooltip`; only
the window line and its `nowMs`/`localeTimeFormat` parameters are gone.)

- [x] **Step 4: Give ProviderChip an accessibleLabel and stop reading tooltipText**

In `assets/omarchy/components/ProviderChip.qml`:

1. In the header comment (lines 7-10), delete the words `tooltip delivery,` —
   the host no longer delivers a tooltip for this widget.
2. Add to the property block (after `property string cueLabel: ""`):

```qml
  // The host tooltip is intentionally never set; this label exists for
  // assistive tech only and must stay single-line.
  property string accessibleLabel: ""
```

3. Change line 94 from `Accessible.name: root.tooltipText` to:

```qml
      Accessible.name: root.accessibleLabel
```

- [x] **Step 5: Strip the tooltip machinery from BarWidget.qml**

In `assets/omarchy/BarWidget.qml`:

1. Delete lines 36-45: the entire tooltip-freshness comment block plus both
   properties `property double tooltipNowMs: Date.now()` and
   `readonly property string shortTimeFormat: Qt.locale().timeFormat(Locale.ShortFormat)`.
2. Replace the chip bindings at lines 109-115 —

```qml
        tooltipText: Core.chipTooltip(modelData, root.displayMetric,
                                      root.tooltipNowMs, root.shortTimeFormat)

        onTooltipHoveredChanged: {
          if (chip.tooltipHovered)
            root.tooltipNowMs = Date.now()
        }
```

— with:

```qml
        accessibleLabel: Core.chipAccessibleLabel(modelData, root.displayMetric)
```

- [x] **Step 6: Run the BarWidget suite to verify it passes**

Run the same qmltestrunner command as Step 2.
Expected: PASS, zero failures.

- [x] **Step 7: Run the full checkpoint**

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
git diff --check
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
Expected: everything green. Read qmllint output only for warnings this change
introduced (the `qs.*` unresolved-import noise is pre-existing); its exit code
is not a verdict.

- [x] **Step 8: Commit**

```bash
git add assets/omarchy/CoreView.js assets/omarchy/BarWidget.qml \
  assets/omarchy/components/ProviderChip.qml tests/qml/tst_BarWidget.qml
git commit -m "feat: remove bar chip hover tooltip"
```

---

### Task 2: Update user-facing docs and amend UX-011

**Files:**
- Modify: `README.md:11-15,28-33`
- Modify: `docs/specs/v10/04-quickshell-ux-and-accessibility.md:24-27`

**Interfaces:**
- Consumes: nothing from Task 1 (prose only).
- Produces: nothing consumed by code.

- [x] **Step 1: Remove hover copy from README.md**

Replace lines 11-15:

```markdown
Each enabled provider gets a compact chip with its icon and percentage,
shown as used or remaining, your pick. Hover a chip for the active
window and its reset time. Click it and the popup opens: plan tag (like
`MAX 20X`), a lead window with both the countdown and the wall-clock
reset, and every other window as a row with its own usage track.
```

with:

```markdown
Each enabled provider gets a compact chip with its icon and percentage,
shown as used or remaining, your pick. Click it and the popup opens:
plan tag (like `MAX 20X`), a lead window with both the countdown and
the wall-clock reset, and every other window as a row with its own
usage track.
```

Then delete the table row `| Hover a chip | Active window and its reset |`
(line 30), leaving the Left/Middle/Right click rows.

- [x] **Step 2: Amend UX-011 in the v10 spec**

In `docs/specs/v10/04-quickshell-ux-and-accessibility.md`, replace the
`UX-011` bullet (lines 24-27):

```markdown
- `UX-011`: Tooltip copy includes the provider name, the displayed
  percentage when one exists, and a plain-language state qualifier when the
  provider is not ready. Raw state identifiers never render. Reset detail is
  limited to the chip's own window; every other window lives in the popup.
```

with:

```markdown
- `UX-011` (superseded 2026-08-06): The chip renders no hover tooltip. Its
  former first line — the provider name, the displayed percentage when one
  exists, and a plain-language state qualifier when the provider is not
  ready — survives as the chip's accessible name. Raw state identifiers
  never render. Reset detail lives only in the popup. See
  `docs/superpowers/specs/2026-08-06-remove-chip-tooltip-design.md`.
```

- [x] **Step 3: Verify the language gate and diff hygiene**

```bash
cargo test --test active_language
git diff --check
```
Expected: PASS.

- [x] **Step 4: Commit**

```bash
git add README.md docs/specs/v10/04-quickshell-ux-and-accessibility.md
git commit -m "docs: record chip tooltip removal"
```
