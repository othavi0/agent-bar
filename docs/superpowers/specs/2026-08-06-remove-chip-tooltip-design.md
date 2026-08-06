# Remove bar-chip hover tooltip

Date: 2026-08-06
Status: approved

## Context

The bar chip currently sets `tooltipText`, so the Quattro host renders a
hover tooltip ~400ms after the pointer enters a chip (`Claude · 96%` plus a
second line describing the window and its reset countdown). The owner judged
the tooltip unnecessary: the chip already shows the percentage and state cue,
and the popup carries the full detail one click away.

A stale-status review preceded this decision and found no defect: stale
retention (temporary failure + prior good data → cached windows with
`lastSuccessAt`, typed error, and Retry) works as specified, `cargo test
stale` passes, and stale rendering (dimmed chip, 󰅐 cue, popup banner) is
untouched by this change.

## Decision

Remove the hover tooltip from the bar chips only. Popup tooltips (provider
Refresh, Settings gear, reorder arrows) stay: those buttons are icon-only and
the tooltip is their only visible name.

## Design

- `BarWidget.qml` stops setting `tooltipText` on `ProviderChip`. The host
  only renders a tooltip when that text is non-empty, so hover shows nothing.
  The machinery that existed solely for the tooltip is deleted with it:
  `tooltipNowMs`, `shortTimeFormat`, and the `onTooltipHoveredChanged`
  refresh handler.
- Accessibility is preserved, not dropped. The chip numeral's
  `Accessible.name` currently reuses the tooltip string. `Core.chipTooltip`
  becomes `Core.chipAccessibleLabel`, returning only the former first line
  (`<name> · <pct> · <qualifier>`), which needs no live clock or locale time
  format. `ProviderChip` exposes a dedicated property for that label,
  decoupled from the host's `tooltipText`.
- `Core.chipWindowLine` is deleted; its only consumer was the tooltip's
  second line. Its newline-injection guard (a provider-supplied window label
  forging an extra tooltip line) becomes moot because the accessible label is
  single-line by construction; `plainText` sanitisation still applies.
- Shared time helpers (`resetCountdownText`, `resetClockText`,
  `resetPhrase`) remain: the popup uses them.

## Not changing

- Bar visuals: icon, numeral, state cues, dimming, click routing, popup.
- Popup tooltips and every popup surface.
- Stale semantics in Rust and in the UI.

## Tests

In `tests/qml/tst_BarWidget.qml`: first-line tooltip assertions are
repointed at `chipAccessibleLabel`; countdown, hover-ordering
(`tooltipNowMs`), and forged-window-label tests are removed together with
the behaviour they proved.

## Spec amendment

UX-011 (chip tooltip, amended 2026-08-03) is superseded: the chip renders no
hover tooltip; its former first line survives as the chip's accessible name.
