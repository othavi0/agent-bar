# Bar chip tooltip: window and reset

Status: approved 2026-08-03. Scope: QML/JS only. No Rust change, no status
schema change, no popup change.

## Problem

Hovering a bar chip renders `Claude · 5%` — the provider name and the number
the chip already prints. The hover restates what the eye just read and answers
nothing. The question a percentage provokes is *when does it come back*, and
today that answer exists only behind a click.

Clicking stays the primary flow. The popup keeps every window, the track, the
severity colour, the staleness banner and the actions. The hover gets one thing:
which window that number belongs to, and when it resets.

## Decisions

| # | Decision | Rejected alternative |
| --- | --- | --- |
| 1 | Two lines. Line 1 is byte-identical to today's tooltip. Line 2 names the window and carries the reset. | One long line — the countdown lands last in reading order, exactly where the eye is not looking. |
| 2 | Line 2 names the window (`Session (5h)`), not just the reset. | Reset alone — Claude runs a 5h and a weekly window at once and the chip shows one of them unnamed. |
| 3 | The wall-clock time renders whenever a countdown exists, at any distance. | A 24h threshold. It would have been a *new* rule, not the popup's — see "Why no 24h threshold". |
| 4 | Line 2 always describes `primaryWindow(provider)` — the same window that produced the chip numeral. | Reusing the popup's `electLeadIndex`. Out of scope; see "Known divergence". |

### Why no 24h threshold

A threshold was designed and rejected on evidence. The premise was that the
popup already suppresses the clock for distant resets, so the tooltip would only
be following suit. It does not. `ProviderView.qml:167` gates the clock on
*lead versus compact row*, not on distance, and `electLeadIndex` elects any
**critical** window first — so an exhausted weekly window leads from six days
out and the popup prints its bare clock today.

Adopting the threshold in the tooltip alone would have put two clock rules in
one product. Adopting it in both would have reopened a popup approved the same
day. Neither is worth the falso-preciso it removes, so the clock is
unconditional and both surfaces stay on one rule.

## Copy contract

`·` is U+00B7. `\n` is a literal newline inside one `tooltipText` string.

| Situation | Tooltip |
| --- | --- |
| ready, reset today | `Claude · 5%`<br>`Session (5h) · resets in 3h 1m (13:31)` |
| ready, reset days out | `Codex · 2%`<br>`Weekly (7d) · resets in 6d 4h (13:31)` |
| reset already elapsed | `Claude · 96%`<br>`Session (5h) · resets now` |
| rate limited, window intact | `Grok · 0% · rate limited`<br>`Daily (1d) · resets in 41m (11:11)` |
| stale, window from cache | `Claude · 5% · stale`<br>`Session (5h) · resets in 3h 1m (13:31)` |
| window without `resetsAt` | `Amp · 5%`<br>`Context` |
| ready, no windows | `Claude · —` |
| signed out / no CLI / failed / loading | `Claude · signed out` |

Line 1 is unchanged in every row: the existing name / percentage / qualifier
composition ships as-is. Every state that renders one line today still renders
exactly one line, because none of them carries a window.

A stale reading keeps its reset line on purpose. `resetsAt` is an absolute
instant; staleness devalues the percentage, never the timestamp. A rate-limited
provider keeps it for the same reason, and that hover is the most valuable one
in the product: it answers when work resumes.

## Composition

`chipTooltip(provider, metric, nowMs, localeTimeFormat)`.

- `nowMs` defaults to `Date.now()` when `undefined`.
- `localeTimeFormat` defaults to `""`. Empty means no clock — `resetClockText`
  already contracts this, and it keeps every pure test independent of the host
  locale.

Line 1: unchanged.

Line 2 is built from `primaryWindow(provider)` and emitted only when non-empty:

1. `labelText` — `plainText(w.label || w.id)`, then collapse `[\r\n]+` to a
   single space. No `"Window"` filler: an unlabelled, unresettable window has
   nothing to say and produces no second line.
2. `resetText` — `""` when there is no `resetsAt` or the countdown is empty;
   `"resets now"` when `resetCountdownText` returns `"now"`; otherwise
   `"resets in " + countdown`, with `" " + resetClockText(...)` appended when
   `localeTimeFormat` is non-empty.
3. `line2 = [labelText, resetText].filter(non-empty).join(" · ")`.
4. Join to line 1 with `"\n"` only when `line2` is non-empty.

`resetCountdownText`, `resetClockText` and `plainText` are reused verbatim. The
tooltip introduces no second humaniser, so hover and popup can never disagree on
how long "3h 1m" is.

### Newline is now a forgeable character

`plainText` strips control characters but deliberately preserves U+000A.
Harmless while the tooltip was one line; with two lines a window label carrying
a newline forges a whole line of tooltip. Codex derives window identity from its
own payload, so the label is not always ours. Step 1 collapses newlines — this
is a requirement, not a nicety, and it gets its own test.

## Freshness

`WidgetButton.onEntered` calls `bar.showTooltip(root, root.tooltipText)`,
passing the string **by value**; `Bar.qml` stores it in `pendingTooltipText` and
renders that snapshot 400ms later. The tooltip is not bound to anything. A live
countdown is impossible without re-calling `showTooltip`, which begins with
`clearTooltip()` and restarts the 400ms delay — a flicker on every tick. There
is no live countdown, by host design.

What matters is that `tooltipText` is fresh at the instant `onEntered` runs:

- `BarWidget` gains `property double tooltipNowMs: 0` and
  `readonly property string shortTimeFormat: Qt.locale().timeFormat(Locale.ShortFormat)`.
- Each `ProviderChip` declares
  `onTooltipHoveredChanged: if (tooltipHovered) root.tooltipNowMs = Date.now()`.
- `tooltipText` binds to `Core.chipTooltip(modelData, root.displayMetric, root.tooltipNowMs, root.shortTimeFormat)`.

This relies on `QQuickMouseArea` emitting `containsMouseChanged` before
`entered()`, so the handler runs and the binding re-evaluates before the host
reads the property. That ordering is an assumption about the host and therefore
gets an explicit test, not a comment. **If the test fails**, the fallback is a
`Timer` in `BarWidget` at `interval: 15000`, `running: <any chip hovered>`, plus
the same hover-start refresh — bounded staleness instead of assumed ordering.
No timer ships unless the test forces it.

Consequence to accept: a hover held past a minute boundary shows a countdown one
minute old. The snapshot is the host's contract and the cost is invisible.

## Contract amendment

`docs/specs/v10/04-quickshell-ux-and-accessibility.md` — `UX-011` currently
ends "Reset detail lives in the popup." That sentence is what this design
reverses, so it is edited to read:

> Reset detail is limited to the chip's own window; every other window lives in
> the popup.

The rest of `UX-011` is untouched and still describes line 1.

## Out of scope

- **The popup.** Not one file under it changes.
- **Known divergence, accepted.** The chip numeral comes from
  `primaryWindow` (`windows[0]`); the popup leads with `electLeadIndex`. Naming
  the window in line 2 makes this visible for the first time — a provider can
  show `Session (5h)` on hover and lead with `Weekly (7d)` on click. Neither
  surface lies: line 2 always describes the number printed beside it. Aligning
  the two would change the number on the bar, which is a larger piece of work
  than this one.

## Testing

`tests/qml/tst_BarWidget.qml`, pure `Core.chipTooltip` — one case per row of the
copy contract, each passing a fixed `nowMs` and the fixed format `"HH:mm"` so no
case depends on the wall clock or the machine locale. Existing cases in
`test_chip_tooltip_humanized` gain real labels and keep their line 1 assertions.

Cases that are not rows of the table:

- empty `localeTimeFormat` → countdown renders, clock does not.
- label containing `\n` → exactly one `\n` in the whole tooltip.
- unlabelled window with no `resetsAt` → single line, no trailing separator.

Hover freshness — create a `ProviderChip` against the existing `fakeBar` stub
(it already captures `showTooltip`), simulate hover, and assert the **captured**
string carries the countdown computed from the current clock. A pushed text
computed at `tooltipNowMs === 0` yields a countdown of roughly 20 000 days, so
the assertion distinguishes fresh from stale unambiguously rather than
restating the binding.

Checkpoint commands are the ones in `CLAUDE.md`, including the Qt6 binary paths
for `qmllint` and `qmltestrunner`.

## Files

- `assets/omarchy/CoreView.js` — rewrite `chipTooltip`; add the line-2 builder
  beside it.
- `assets/omarchy/BarWidget.qml` — `tooltipNowMs`, `shortTimeFormat`, the hover
  handler, the updated `tooltipText` binding.
- `tests/qml/tst_BarWidget.qml` — extend `test_chip_tooltip_humanized`, add the
  freshness and injection tests.
- `docs/specs/v10/04-quickshell-ux-and-accessibility.md` — the `UX-011`
  sentence.
