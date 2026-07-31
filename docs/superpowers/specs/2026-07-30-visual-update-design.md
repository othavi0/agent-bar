# Agent Bar v11 — Visual Update Design

Status: approved design, not yet planned or implemented.
Date: 2026-07-30.
Surface: bar chips and consolidated popup. Mode: Operate.

This design refines the incumbent Quattro-native visual world. It is not a
redesign: product truth, copy language, information architecture, keyboard
model, and the v10 removal contract are preserved.

## 1. Problem

Agent Bar renders correct data through an accidental parallel design system.
Sixteen `Qt.darker(foreground, n)` calls and nineteen hardcoded
`Qt.rgba(foreground…, alpha)` calls reimplement, badly, a token vocabulary the
host already exports. That single root cause produces the visible defects:
icons off the bar grid, a rail that does not align with its own content,
severity that never appears, and a text hierarchy that inverts on light
themes.

`PRODUCT.md` design principle 6 ("No custom theme system over Quattro") and
`UX-056` already forbid this. The code drifted from its own contract.

## 2. Measured evidence

All figures measured against the live installation and the checked-in assets
on 2026-07-30.

### 2.1 Provider icon optical weight

Ink coverage inside an identical 64x64 box (ImageMagick, alpha channel mean):

| Asset | Format | Coverage | Reads as |
| --- | --- | --- | --- |
| `codex.png` | PNG 64x64, gray+alpha | 78.5% | solid white puck |
| `amp.svg` | SVG, `#F34E3F` | 36.3% | correct |
| `claude.png` | PNG 48x48, indexed | 35.9% | correct |
| `grok.svg` | SVG, `fill="white"` | 22.6% | thin ring |

Codex carries 3.5x the ink of Grok in the same box. Cause: `codex.png` is an
app icon — a filled white disc with the artwork knocked out in black — while
the other three are marks on transparency. It is the wrong grade of asset,
not the wrong size.

`grok.svg` hardcodes `fill="white"`. Omarchy ships five light themes
(`catppuccin-latte`, `flexoki-light`, `lupine`, `rose-pine`, `white`), on
which the Grok mark is invisible. This violates `UX-057`.

### 2.2 Bar grid

Quattro exports `Style.bar.iconSlot` (27), `Style.bar.iconCanvas` (16),
`Style.bar.iconFont` (13) and `Style.bar.statusSlot` (21), plus `OpticalGlyph`
for optical centring. `ProviderChip.qml` uses none of them and hardcodes
`width: 13; height: 13`. Agent Bar chips do not sit on the same icon grid as
every neighbouring bar module.

### 2.3 Horizontal jitter

Chip text length varies between one and four characters (`—`, `97%`, `100%`,
`—…`). The bar is right-aligned, so any provider crossing 100 to 99, or
entering loading, shifts every module to its left by roughly one character
cell.

### 2.4 Light-theme hierarchy inversion

`Qt.darker` divides HSV value. On a dark theme this recedes; on a light theme
it advances. Contrast ratio of primary versus `Qt.darker(foreground, 1.35)`
secondary, against each theme background:

| Theme | Primary CR | Secondary CR | Result |
| --- | --- | --- | --- |
| tokyo-night | 8.10 | 4.58 | correct |
| catppuccin-latte | 7.06 | 9.80 | inverted — secondary 39% louder |
| lupine | 15.43 | 16.94 | inverted |
| flexoki-light | 18.62 | 19.12 | inverted |
| white | 21.00 | 21.00 | collapsed — identical pixels |

On the `white` theme `Qt.darker(#000000, n)` returns `#000000`: secondary and
primary text become indistinguishable. Sixteen call sites are affected.

Note: the host's own `PanelSectionHeader.qml` has the same flaw. That is an
upstream issue and is out of scope here; Agent Bar's sixteen call sites are
in scope.

### 2.5 Popup geometry

The rail frame starts 18px from the card top (`popupPadding` 14 +
`ProviderRail.outerMargin` 4). Content starts at 28px (`popupPadding` 14 +
`Popup.contentMargins` 14). Ten pixels of drift on a baseline that should be
shared. The rail additionally draws its own fill and border inside a card
that already has a 2px border.

### 2.6 Emoji in a monospace surface

`CoreView.chipStateCue` returns `⌛` (U+231B), which fontconfig resolves
through the emoji font — a colour glyph inside a JetBrainsMono bar. The cue
strings are also inconsistently padded: `" ⌛"` and `" !"` carry a leading
space, `"…"` does not.

## 3. Decisions

Taken with the user across four visual-companion reviews.

1. **Brand colour is preserved.** Claude and Amp keep their official colours.
   Codex and Grok are monochrome brands and adopt `Color.foreground`, which
   is the correct use of a monochrome mark and fixes light themes. Rule:
   polychrome marks keep their colour, monochrome marks inherit the theme ink.
2. **Bar chips keep number and unit** (`97%`), with a fixed-width numeral box.
3. **Motion stays host-owned.** Agent Bar declares no `Behavior`, `Transition`
   or animation. `A11Y-013` and `TEST-029` are unchanged.
4. **The popup leads with one window.** The most urgent window renders large;
   every other window renders as a compact row with its own track.
5. **The plan badge becomes a tag**, not a pill.
6. **The meta footer is removed** in all states. The user confirmed this after
   the `UX-017`/`UX-028` cost was raised. See section 9.
7. **Reset countdowns show hours below 24h.** Already correct in
   `CoreView.countdownText`; the compact row must not abbreviate it.

## 4. Token binding

This is the backbone of the change. Most of the work is deletion.

| Current | Replacement |
| --- | --- |
| `Qt.darker(fg, 1.2…1.4)` — 16 sites | `Util.alpha(Color.foreground, a)`. The five existing factors collapse to two roles: supporting text and labels at `0.72`, meta and caption text at `0.55`. The mapping is by role, resolved per call site; no call site keeps a third value. |
| `Qt.rgba(fg…, 0.05)` + `0.22` — rail frame | deleted with the frame |
| `Qt.rgba(fg…, 0.12)` — selected rail plate | `Style.selectedFill` |
| `Qt.rgba(fg…, 0.3)` — selected rail border | `Style.selectedBorderColor` |
| `Qt.rgba(fg…, 0.08)` — hover plate | `Style.hoverFill` |
| `Qt.rgba(fg…, 0.12)` and `0.08` — separators (8 sites) | `PanelSeparator` (its default `strength` is already `0.12`) |
| hardcoded `width: 13` / `16` on icons | `Style.bar.iconCanvas` and `Style.bar.iconSlot` |
| chip `opacity: 0.55` / `0.45` for dimmed | `WidgetButton.dimmed` semantics (`0.45`) |
| ad-hoc spacings | `Style.spacing.{sm,md,lg,xl,xxl}` and `Style.spacing.popupPadding` |

One value has no host token: the usage track background. It is declared once,
as a named readonly property on `UsageWindow`, and never repeated.

`Color.urgent` is the single severity colour. No new colour is introduced.

## 5. Bar specification

- `ProviderChip` is rebuilt on Quattro's `WidgetButton`, inheriting slot
  sizing, `dimmed`/`concealed` semantics, tooltip delivery, pointer cursor,
  and the host `Behavior on opacity` (140ms OutCubic) and `Behavior on color`
  (160ms). Implementation must verify that `WidgetButton`'s own bar
  registration does not double-register the click target required by
  `UX-010`; if it does, the chip keeps its explicit registration and adopts
  only the sizing and state vocabulary.
- Icon canvas is `Style.bar.iconCanvas`. Per-provider optical scale lives in
  `CoreView.iconOpticalScale(providerId)`, beside the existing
  `iconFileName`: Claude, Codex and Amp at `1.0`; Grok at `0.875`, because its
  mark is a thin ring that fills its own box edge to edge.
- Numeral box is fixed width, right-aligned, measured with `TextMetrics` on
  the string `100%` at `Style.font.body`. Never a hardcoded pixel value: the
  font scales with `[font] base-size`.
- Gap inside a chip is `Style.spacing.sm` (4). Gap between chips is
  `Style.spacing.xxl` (12). The 3:1 ratio is what makes an icon and its number
  read as one unit; today's 4:10 does not.
- State cues carry no leading space; separation is layout spacing. `⌛` is
  replaced by a Nerd Font glyph from the active bar family. The loading cue is
  visually distinct from the `—` no-data glyph.

## 6. Popup specification

- **Rail.** Own fill and border deleted. Slot stack top edge equals content
  top edge: one shared inset of `Style.spacing.popupPadding`. Gutter stays
  `Style.spacing.lg` (8). Selected plate uses `Style.selectedFill` and
  `Style.selectedBorderColor`, preserving `UX-020B` (neutral plate, no accent
  tick).
- **Header.** `name · plan tag · [severity tag] · spacer · refresh`. The tag
  is a 1px `Style.normalBorderColor` border at `Style.cornerRadius`,
  `Style.font.caption`, uppercase. Uppercasing also normalises provider plan
  labels that arrive lowercase from the API, such as Codex's `plus`.
- **Lead window.** `Style.font.body * 2.5` numeral, unit at
  `Style.font.caption`, a `Style.spacing.md` track, and the reset promoted
  into the label line: `Session (5h) · resets in 3h 1m`.
- **Other windows.** One compact row each: `label · track · value · time to
  reset`. Reset time uses `CoreView.countdownText` unmodified, so anything
  under 24h reads in hours. The compact column is sized for `23h 1m`.
- **Footer.** Removed.
- **Stale banner.** Absorbs the last-success age that the footer carried:
  glyph, `Last data 14m ago · <safe error summary>`, `Retry`. Only rendered
  when stale, which is the existing behaviour.
- **Connection state.** No longer rendered as a label. It is already implied
  structurally: `CoreView.contentMode` only returns `windows` when the
  provider state is `ready`, so a pane showing windows with no stale banner is
  by definition connected. The label was redundant. See section 9.

## 7. Severity model

Severity reuses the existing Rust thresholds in
`NotificationLevel::from_used_percent`. No second threshold is invented.

| `usedPercent` | Level | Treatment |
| --- | --- | --- |
| `>= 95` | Critical | severity tag reading `Critical`; lead numeral and track in `Color.urgent`; `!` cue on the bar chip |
| `>= 90` | Warning | severity tag reading `Low`; numeral and track unchanged |
| `< 90` | none | no tag |

Severity is always computed from `usedPercent`, independent of the user's
`remaining`/`used` display metric, so switching the metric never changes what
counts as critical.

`A11Y-012` is satisfied: every level carries a word, never colour alone.

Thresholds are duplicated in `CoreView.js` because the status schema is
frozen at v2 and must not gain a field. A Rust test reads both
`src/notifications/state.rs` and `assets/omarchy/CoreView.js` and asserts the
constants match, so the duplication cannot drift.

## 8. Lead window election

Deterministic, in order:

1. If any window is Critical, the lead is the Critical window with the lowest
   `remainingPercent`.
2. Otherwise the lead is the window with the nearest future `resetsAt`.
3. Ties break by the provider's window order as delivered, then by window id.
4. If no window has `resetsAt`, the lead is the first window.
5. Every other window renders as a compact row, in delivered order.

This replaces the `PRIMARY_WINDOW_IDS` allowlist (`session`, `weekly`,
`daily`), which is deleted. The allowlist silently demoted any window whose id
was not in the set; election handles new window ids without a code change.

## 9. Specification amendments required

- **`UX-017`** currently reads: "The header shows name, plan badge, connection
  state, update age, and provider refresh." Amend to: "The header shows name,
  plan badge, severity when present, and provider refresh." Rationale in
  section 6: connection state is structurally implied and update age moves to
  the stale banner.
- **`UX-028`** currently requires stale content to show a `Stale` label, last
  success age, and safe error summary. The requirement is preserved; only its
  location changes, from the meta footer to the stale banner. Amend the
  wording to name the banner as the carrier.
- **`UX-020A`** already requires a track per percentage window. Extend it to
  the compact rows so secondary windows stay comparable.
- **`UX-016`** (header does not repeat the provider icon) is unchanged.
- **`A11Y-013`**, **`TEST-029`**, **`UX-049`**, **`UX-056`** and
  **`UX-057`** are unchanged and this design moves the code toward compliance
  with the last three rather than away.

The user was told the footer removal costs two amendments and confirmed the
removal. This section records that decision, not a proposal.

## 10. Asset actions

- **Codex.** The design requires a mark-grade monochrome asset, not the
  filled app icon. A knockout derived from the current official asset proves
  the target and measures at 14.5% coverage. Implementation must obtain the
  official monochrome mark; if none is published, the derivation is adopted
  and recorded as the approved asset under `UX-049`.
- **Grok.** `fill="white"` is removed. The mark is tinted to
  `Color.foreground` at render time with `MultiEffect { colorization: 1.0 }`
  from `QtQuick.Effects`, which is present in the runtime and already used by
  first-party Omarchy plugins (`plugins/background/Background.qml:250`). The
  same treatment applies to Codex. Claude and Amp are never tinted.

> Correction (2026-07-30, plan 02 execution): `fill="white"` is NOT removed
> from `grok.svg`. Measured on the GPU backend, `MultiEffect` colorization
> multiplies source luminance by `colorizationColor` — a black source stays
> black and cannot take the theme ink. White is therefore the required mask
> convention for tinted monochrome marks, and the UX-057 concern is resolved
> by the tint itself: the hardcoded white never reaches the screen raw. The
> asset actions reduce to adopting the Codex mark; `grok.svg` is unchanged.

## 11. Copy

All shipped UI copy remains English, per the engineering contract. The
Portuguese strings in the companion mockups existed for review only and must
not reach the code.

## 12. Out of scope

- The Settings and Maintenance views are not redesigned. They inherit the
  section 4 token binding, and nothing else. Eight of the sixteen `Qt.darker`
  sites live outside the bar and provider pane — four in `SettingsView.qml`,
  three in `MaintenanceView.qml`, one in `ConfirmDialog.qml` — and are fixed
  as part of that binding.
- No new provider, metric, window type, or setting.
- No spend, currency, history, chart, or dashboard affordance.
- No change to polling, caching, IPC, keyboard model, or focus order.

## 13. Suggested phasing

The change is coherent but too large for one commit. Natural order, each
phase independently verifiable and independently shippable:

1. **Token binding** (section 4). Mechanical, repo-wide, highest value per
   risk: it alone fixes light themes and removes the parallel system.
2. **Bar chip** (section 5) plus the asset actions (section 10), because chip
   sizing and asset grade are the same problem.
3. **Popup structure** (section 6): rail, header, tags, footer removal, stale
   banner.
4. **Severity and lead-window election** (sections 7 and 8), the only phase
   introducing new product rules.
5. **Specification amendments** (section 9), landed with phase 3 and 4 so the
   spec never describes shipped behaviour incorrectly.

## 14. Verification

In addition to the standard checkpoint and the QML gate:

- A test asserting no `Qt.darker` remains in `assets/omarchy/**`.
- A test asserting the severity constants in `CoreView.js` match
  `NotificationLevel::from_used_percent`.
- A test asserting `CoreView.iconOpticalScale` returns a value for every
  catalog provider id.
- Lead-window election unit tests covering: critical wins over nearest reset,
  nearest reset among healthy windows, no `resetsAt`, single window, and tie
  breaking.
- Screenshot review on one dark theme and one light theme, with `white` as
  the light case, since it is where the current hierarchy collapses entirely.
- `TEST-029` must still pass unchanged.
