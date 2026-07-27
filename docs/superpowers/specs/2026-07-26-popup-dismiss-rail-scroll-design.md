# Popup dismiss, rail layout, usage bars, and short-content scroll

**Date:** 2026-07-26  
**Branch:** feat/quickshell-native-v10  
**Status:** Implemented  
**Approach:** Content-fit card + overflow-gated Flickable + option-A rail stack;
foreign-monitor dismiss overlay; usage progress tracks on percentage windows.

## Problem (live QA)

1. **Popup did not open on chip click** under Quattro: `Loader`+`Component`
   failed KeyboardPanel required `anchorItem`/`bar`; fixed by direct child
   Popup without redeclaring required props.
2. **Rail icons overlapped** when the panel was content-fit short: Settings
   used `anchors.bottom` over a top-stacked column.
3. **Selected rail chrome stuck on Claude** while viewing Grok: focus ring and
   id source of truth.
4. **No usage bar** for Amp/Grok windows: `UsageWindow` was text-only.
5. **Scroll without overflow** and **280px empty floor**.
6. **Cross-monitor outside-click** only works on the owner monitor with plain
   KeyboardPanel (same as first-party Omarchy panels). Agent Bar adds an
   optional foreign-dismiss layer for always-close on the other monitor.

## Final rail (option A)

- Single `ColumnLayout`: providers (fixed gap) → flexible spacer → Settings.
- **Never** pin Settings with `anchors.bottom` over icons.
- Full border around the rail strip; equal `framePad` top and bottom.
- Horizontal: `railWidth = outerMargin×2 + framePad×2 + slotSize` so L/R
  padding matches (slot fills the inner width).
- Selected provider: soft neutral fill + thin neutral border only (no blue
  accent tick). Selection follows `popupOwner.providerId` when open.
- Settings: same slot size as providers; no idle border (does not look
  selected).

## Usage windows

- `windowDisplayLines` exposes numeric `percent` (0–100 or −1).
- `UsageWindow` renders label, percent text, optional reset, and a horizontal
  track filled by `percent` (remaining or used per settings metric).

## Scroll and height

- Card height: `fittedPopupContentHeight(body, minCompact≈160, max)`.
- No `Style.space(280)` empty floor.
- `Flickable.interactive` only when content overflows viewport.
- Content width: exact `parent − rail − gutter` (avoids left glyph clip).

## Cross-monitor dismiss

- `Service.dismissPopup()` clears ownership unconditionally.
- Non-owner `BarWidget` hosts `agent-bar-foreign-dismiss` layer-shell overlay;
  bar-strip clicks forward to click targets for transfer.
- Same-monitor outside-click remains KeyboardPanel dismiss.

## Files

| File | Role |
| --- | --- |
| `BarWidget.qml` | Direct Popup; foreign dismiss |
| `Popup.qml` | Height fit, gutter, selectedId, Flickable gate |
| `ProviderRail.qml` | Option-A stack + border + selection |
| `UsageWindow.qml` | Progress track |
| `Service.qml` / `ServiceCore.js` | dismissPopup, fit/scroll helpers |
| `ProviderHeader` / `StateMessage` | Width/elide fixes |
| `tests/qml/tst_*` | Contracts for rail, scroll, dismiss |

## Acceptance (live)

- Chips show real percents; left/right click open usage/settings.
- Rail: spaced icons, bordered strip, correct selected plate, settings inset.
- Amp/Grok windows show filled usage track.
- Short content does not scroll; long settings can.
- Same-monitor outside-click closes; other-monitor closes via foreign dismiss
  when the compositor delivers the click.
