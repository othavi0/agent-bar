# Quickshell UX and Accessibility

## Visual direction

Agent Bar follows the active Omarchy Quattro theme. It does not introduce a
separate brand shell, hardcoded Saga palette, or custom control system.
Provider identity comes from the official provider icons; generic actions use
verified Quattro-native glyphs or explicit English text labels.

## Bar chips

- `UX-001`: Render one compact chip per enabled provider in settings order.
- `UX-002`: A chip contains the provider icon and the configured used or
  remaining percentage.
- `UX-003`: Do not render `AB`, `Agent Bar`, or a generic leading product icon.
- `UX-004`: Left click opens the clicked provider.
- `UX-005`: Left-clicking the same provider while open closes the popup.
- `UX-006`: Left-clicking another provider switches content without closing.
- `UX-007`: Middle click requests one all-provider cache-bypass refresh.
- `UX-008`: Right click opens Settings.
- `UX-009`: Mouse wheel over a bar chip has no effect.
- `UX-010`: Each chip registers and unregisters a Quattro bar click target and
  implements the host trigger protocol.
- `UX-011` (superseded 2026-08-06): The chip renders no hover tooltip. Its
  former first line — the provider name, the displayed percentage when one
  exists, and a plain-language state qualifier when the provider is not
  ready — survives as the chip's accessible name. Raw state identifiers
  never render. Reset detail lives only in the popup. See
  `docs/superpowers/specs/2026-08-06-remove-chip-tooltip-design.md`.
- `UX-012`: Stale and error states use an icon/text cue in addition to color.

## Popup layout

```text
+------+-------------------------------------------+
|  C   | Claude                  Max | Connected   |
|  O   | Updated 2 minutes ago              [R]    |
|  A   +-------------------------------------------+
|  G   |                                           |
|      | Selected provider content                 |
|      |                                           |
|      | Full-width sections and separators        |
|      |                                           |
|  S   |                                           |
+------+-------------------------------------------+
```

`C`, `O`, `A`, and `G` represent provider icons. `S` represents the native
settings glyph. `[R]` represents the native refresh glyph; these letters are
not literal UI.

- `UX-013`: The left rail is visually separate and uses provider icons only.
- `UX-014`: Provider names are available through tooltips and accessibility.
- `UX-015`: Settings is the last control in the rail stack (the stack shares
  the popup content inset top and bottom; not overlaid with
  `anchors.bottom` on short cards).
- `UX-016`: The provider header does not repeat the provider icon.
- `UX-017`: The header shows the provider name, the plan tag, severity when
  present, and the provider refresh control. Connection state is implied
  structurally (windows render only when ready) and update age lives in the
  stale banner.
- `UX-018`: Only one provider's content is visible at a time.
- `UX-019`: Section backgrounds and separators extend through the full content
  width.
- `UX-020`: Popup dimensions use Quattro fitting helpers and the current screen
  geometry; fixed sizes are maximum intentions, not unconditional dimensions.
  Content-fit height has a small compact floor only; no large empty minimum.
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
- `UX-020B`: The selected rail icon uses a neutral soft plate only for the
  provider that owns the open content; no accent edge tick; Settings has no
  idle selected-looking border.
- `UX-021`: The popup opens on the monitor that received the interaction.
- `UX-022`: Only one agent-bar popup is visible across all monitors.
- `UX-023`: Moving the popup to another monitor preserves selected provider
  and service state.
- `UX-024`: Selection and scroll position persist while the popup remains open.
- `UX-025`: A new open resets the selected provider content to the top.

## Provider states

- `UX-026`: Initial collection with no prior data uses skeleton placeholders.
- `UX-027`: Refresh with prior data keeps content and shows a subtle progress
  indicator.
- `UX-028`: Stale content stays visible. The stale banner carries the
  `󰅐` cue, the last success age, the safe error summary, and Retry; it
  renders whenever the provider state is stale.
- `UX-029`: Missing CLI keeps the enabled provider icon dimmed and shows
  `Install guide` and `Check again`.
- `UX-030`: Unauthenticated shows `Sign in` when login discovery succeeds;
  otherwise it shows `Install guide`.
- `UX-031`: Network and rate-limit screens distinguish retryable state and do
  not imply authentication failure.
- `UX-032`: Provider errors never render raw stderr or rich text.
- `UX-032A`: A ready provider with no normalized percentage window shows `—`
  in its chip and `This account is billed another way.` in the popup.

## Settings

Settings contains:

- provider enable controls;
- provider order controls;
- a used/remaining selector;
- a native numeric refresh interval control;
- the notification toggle;
- `Restore defaults`;
- `Cancel`;
- `Save changes`;
- the Maintenance section.

Requirements:

- `UX-033`: Provider rows use official icons and English names.
- `UX-034`: Ordering uses verified native up/down chevrons.
- `UX-035`: The interval uses a native numeric control, not custom plus/minus
  glyph buttons.
- `UX-036`: `Restore defaults` changes only the draft.
- `UX-037`: `Save changes` is unavailable while invalid or saving.
- `UX-038`: `Cancel` restores the persisted snapshot.
- `UX-039`: Settings has no credentials, theme editor, cache editor, local
  history, currency, or hidden advanced panel.

## Maintenance

- `UX-040`: Show the installed version.
- `UX-041`: `Check for updates` performs an explicit network request.
- `UX-042`: When available, show `Update to <version>` and a release-notes
  link.
- `UX-043`: Update confirmation names current version, target version,
  settings preservation, and rollback behavior.
- `UX-044`: `Uninstall Agent Bar` is visually separated as a danger action.
- `UX-045`: Uninstall confirmation defaults to preserving settings.
- `UX-046`: `Also delete saved settings and backups` is unchecked by default.
- `UX-047`: A second explicit destructive click is required.
- `UX-048`: QML passes typed argv and structured stdin; it never constructs a
  shell command.

## Icons and theme tokens

- `UX-049`: Provider icons are the official approved assets. The Codex icon
  is the approved monochrome mark derived from the official Codex app icon
  (knockout extraction, adopted 2026-07-30, sha256
  `880a6d7e2fdb3ed4cb7c9f2f9c8c295050294756dd30eb951462a3b2d08c5397`);
  OpenAI publishes no standalone monochrome mark. Monochrome marks (Codex,
  Grok) are stored as white-on-transparency masks: the runtime always
  renders them through `MultiEffect` colorization, which multiplies mask
  luminance by the theme foreground, so the stored white never reaches the
  screen raw. Claude and Amp keep their official brand colors and are never
  tinted.
- `UX-050`: Generic controls use Quattro controls and its active font stack.
- `UX-051`: Refresh uses `󰑐`.
- `UX-052`: Settings uses `󰒓`.
- `UX-053`: Navigation uses Quattro's verified native chevrons.
- `UX-054`: `Sign in`, `Save changes`, and `Restore defaults` use text labels.
- `UX-055`: Delete the v9 custom `IconButton` and its mixed Unicode/Font
  Awesome codepoint table.
- `UX-056`: Color, typography, spacing, radius, and focus use the values and
  native controls actually exported by Quattro `Color` and `Style`. Agent Bar
  creates no custom elevation or motion system.
- `UX-057`: No theme-specific color is hardcoded.
- `UX-058`: No spend, currency, credit balance, or other monetary value is
  rendered.

## Keyboard and focus

- `A11Y-001`: Use `KeyboardPanel` as the popup keyboard surface.
- `A11Y-002`: Up/Down and `j`/`k` switch provider rail entries.
- `A11Y-003`: Tab and Shift+Tab traverse visible actions in visual order.
- `A11Y-004`: Enter and Space activate the focused control.
- `A11Y-005`: `r` refreshes the selected provider.
- `A11Y-006`: `s` opens Settings.
- `A11Y-007`: Escape closes the popup.
- `A11Y-008`: Panel shortcuts are suspended while a field editor owns input.
- `A11Y-009`: Every interactive control has native visible focus,
  `Accessible.name`, role, and action.
- `A11Y-010`: Focus order excludes hidden and disabled controls.
- `A11Y-011`: Focus movement scrolls an off-screen target into view.
- `A11Y-012`: No provider state is color-only.
- `A11Y-013`: Agent Bar QML declares no `Behavior`, `Transition`, or animation
  of its own. Internal motion of imported Quattro controls is host-owned and is
  not represented as a plugin-controlled reduced-motion setting.

## Scrolling

- `A11Y-014`: Provider and Settings content uses a native vertical `Flickable`.
- `A11Y-015`: `contentWidth` equals the viewport width.
- `A11Y-016`: `flickableDirection` is vertical.
- `A11Y-017`: `boundsBehavior` is `Flickable.StopAtBounds`.
- `A11Y-018`: A vertical scrollbar appears when content overflows.
- `A11Y-018A`: When content does not overflow the viewport, the Flickable is
  not interactive and `contentY` stays at 0.
- `A11Y-019`: Mouse wheel and touchpad scrolling follow system direction.
- `A11Y-020`: No custom wheel inversion, debounce, or network request is tied
  to scrolling.
- `A11Y-021`: Keyboard scrolling clamps to valid content bounds.
- `A11Y-022`: Switching to shorter content clamps stale `contentY`.
- `A11Y-023`: PageUp/PageDown move by one viewport minus one content line;
  Home/End move to the first/last valid vertical position. These keys scroll
  only when no editor is active.

## Focus routing contract

`PanelKeyCatcher` uses `Keys.BeforeItem`, so native focus alone is insufficient.
`Popup.qml` owns one `FocusController` with an ordered list of every visible,
enabled action in visual order.

- `onTabRequested(direction)` moves cyclically through that list, calls
  `forceActiveFocus()` on the selected item, and scrolls it fully into view.
- `onActivateRequested()` invokes the selected item's typed activation
  callback; it never fabricates a mouse event or shell command.
- Every interactive Quattro button sets `focusable: true` and complete
  `Accessible` properties.
- `PanelKeyCatcher.blocked` is true while a text field, spin box, combo popup,
  confirmation editor, or other native editor owns focus.
- Up/Down and `j`/`k` change provider only when the catcher is not blocked.
- PageUp/PageDown/Home/End are handled after the focused item declines them,
  then clamp `contentY` to `0..max(0, contentHeight - height)`.

The scroll surface is exactly:

```qml
Flickable {
    id: flick
    contentWidth: width
    contentHeight: contentColumn.implicitHeight
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    flickableDirection: Flickable.VerticalFlick
    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
}
```
