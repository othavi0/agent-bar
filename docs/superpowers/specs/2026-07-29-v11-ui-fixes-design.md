# v11 UI polish fixes — design

Date: 2026-07-29. Approved by the owner after live use of the Fase 2 popup.
Amends the locked visual decisions of
`2026-07-28-v11-recovery-quattro-parity-design.md` (label format A) at the
owner's request.

## Problems observed in live use

1. Chip hover tooltip reads `Claude · 14% · ready · resets 5h Reset 1m ·
   12:50`. The raw `ready` state is noise, and the window label (`5h Reset`)
   concatenated with the humanized reset text repeats "Reset"/"resets".
2. The provider popup opens with a scrollbar and a few pixels of scroll even
   though content fits. Root cause: `Popup.qml` sizes the card as
   `content + padding * 2`, but `KeyboardPanel` subtracts
   `verticalContentInset` (padding **plus** top/bottom border) from the inner
   area. The ~4px border becomes overflow, `flickableInteractive` turns on.
3. Amp window labels `1d Reset` / `7d Reset` are cryptic; the owner wants
   `Daily` / `Weekly`. The same scheme generalizes to every provider and
   removes the duplicated "RESET" kicker in the popup.

## Decisions

### A. Window labels (Rust-owned, all providers)

Labels keep being born in `src/providers/v2_map.rs`; QML still never
re-derives them. New naming, duration kept in parentheses (owner's request):

| window id            | old label   | new label      |
| -------------------- | ----------- | -------------- |
| `session`            | `5h Reset`  | `Session (5h)` |
| `daily`              | `1d Reset`  | `Daily (1d)`   |
| `weekly`             | `7d Reset`  | `Weekly (7d)`  |
| Codex `other:{n}:…`  | `{n}m Reset`| `{n}m`         |

Popup kicker renders uppercase: `SESSION (5H)`, `WEEKLY (7D)`.

### B. Chip tooltip (CoreView.js)

`chipTooltip` returns `Name · pct`. The typed state is appended only when it
is not `ready` (stale/error states are actionable; `ready` is noise). The
reset segment is removed entirely — reset detail lives in the popup.

Examples: `Claude · 99%` · `Claude · 99% · stale` · `Grok · — · network_error`.

### C. Popup height (Popup.qml)

`contentHeight` uses the panel's own `verticalContentInset`
(`padding * 2 + Border.top + Border.bottom`) instead of `padding * 2`, so the
border no longer eats viewport and short content never scrolls.

## Testing

- Rust: existing tests pinning old labels updated; label table asserted per
  provider adapter.
- QML: `chipTooltip` unit cases (ready / stale / error); source-inspection
  test asserting the `contentHeight` binding uses `verticalContentInset`.
- Full gate: cargo fmt/test/clippy, qmllint, `omarchy plugin validate`,
  Qt6 `qmltestrunner`.

Out of scope: any other visual change; popup structure stays as shipped in
Fase 2.
