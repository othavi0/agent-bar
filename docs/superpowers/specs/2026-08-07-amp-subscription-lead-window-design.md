# Amp subscription: adaptive lead window

Status: approved 2026-08-07. Scope: `electLeadIndex`/chip source in
`CoreView.js`, two label strings in `v2_map.rs`, two spec amendments. No
status-schema change, no new setting.

## Problem

Amp accounts now carry up to three quota surfaces at once (measured live on
this machine, 2026-08-07, `amp usage` on a Megawatt account):

```
Amp Free: 69% remaining today (resets daily) - https://ampcode.com/settings#amp-free
Subscription Megawatt: 100% other usage and 100% orb usage remaining
Individual credits: $4.19 remaining (replenishes automatically) - https://ampcode.com/settings
```

- **Amp Free** — a daily allowance, closed to new signups but alive on
  existing accounts, consumed *before* subscription usage. Resets daily.
- **Subscription** (beta, 2026-07-18) — Megawatt ($20/mo) or Gigawatt
  ($200/mo). Two buckets: `other usage` is the included agent usage and `orb
  usage` is the included orb-hours. Monthly reset; the CLI exposes no
  timestamp. `amp usage` text is the only interface — there is no JSON flag.
- **Individual credits** — monetary; excluded by product rule (v10 removes
  monetary data) and by JSON-022B. Stays excluded.

The parser already emits the plan badge and the `plan-other`/`plan-orb`
windows. The display does not follow: the chip renders `windows[0]`
(`primaryWindow`), which is always the Amp Free daily window, and the popup's
election (`UX-020D`) prefers the nearest future reset — plan windows carry no
`resetsAt`, so Amp Free leads even at 5% subscription remaining. A subscriber
never sees the thing they pay for without opening the popup and reading the
compact rows.

## Decisions

| # | Decision | Rejected alternative |
| --- | --- | --- |
| 1 | Election gains one rule between critical and nearest-reset: plan windows (id prefix `plan-`) outrank non-plan windows; among plan windows the lowest remaining percentage leads. | Always leading with the agent bucket — burning orb-hours would stay invisible. |
| 2 | The chip renders the elected lead window, not `windows[0]`. Chip and popup can never disagree on which window a number belongs to. | Reordering windows in Rust — it cannot fix the popup (nearest-reset still elects Amp Free) and braids display policy into a pure parser. |
| 3 | Free-only accounts are byte-identical to today: with no plan window, rules 1 and 3–4 reproduce the current election exactly. | A settings toggle — adaptive needs no configuration surface. |
| 4 | Labels become `Plan · agent` and `Plan · orbs`. Window ids stay `plan-other`/`plan-orb`. | Keeping the CLI's word `other`, which explains nothing; renaming ids, which churns tests for no user-visible gain. |
| 5 | Plan windows keep `resetsAt: null`. No countdown renders for them. | Synthesizing a monthly reset timestamp the CLI does not expose — falso-preciso. |

The critical rule stays first on purpose: an exhausted Amp Free window (≥95%
used) still takes the lead over a healthy subscription. Severity is the one
signal that must never be displaced by plan preference.

## Election contract (amended UX-020D)

1. A critical window wins; among criticals, the lowest remaining percentage.
2. Otherwise a plan window (window id starting `plan-`) wins; among plan
   windows, the lowest remaining percentage.
3. Otherwise the window whose reset comes soonest; a missing or elapsed
   timestamp does not compete.
4. Otherwise the first delivered window.

Ties keep the delivered order at every step. Every other window renders as a
compact row in delivered order — for a subscriber that is where Amp Free
lives, countdown intact.

The `plan-` prefix is typed schema data — window ids reach QML through the
frozen v2 schema, already sanitized by the Rust mappers — so QML matching on
it does not violate the "QML never parses raw provider output" rule. Today
only the Amp mapper emits `plan-` ids (`plan-other`, `plan-orb`). Grok and
Codex derive some ids from payload enum values (`grok_window_identity`,
`codex_window_identity`), so a novel upstream value could in principle start
with `plan-`; the consequence is only lead preference among that provider's
own windows, which is the intended meaning of the prefix, not a safety
concern.

## Chip contract (amended UX-002)

`chipPercentText` reads the elected lead window via the same election the
popup uses, instead of `primaryWindow(provider)`. `chipAccessibleLabel` and
`chipNumeralText` follow for free since they compose `chipPercentText`.
`primaryWindow` loses its last caller and is deleted, not left dormant.

Chip severity (`chipSeverityUrgent`) keeps reading the *worst* window,
independent of the displayed one — unchanged.

## Rust change

`amp_from_usage_text` label constants only: `Plan · other` → `Plan · agent`,
`Plan · orb` → `Plan · orbs`. Emission order, ids, percentages, plan badge,
and the credits exclusion are untouched.

## Contract amendments

`docs/specs/v10/04-quickshell-ux-and-accessibility.md`:

- `UX-002` gains: the percentage is the elected lead window's (per
  `UX-020D`), so chip and popup always name the same number.
- `UX-020D` gains the plan-window rule as step 2 (wording above).

## Tests

- QML (`tst_Format`/`tst_ProviderStates`, direct JS calls): healthy
  subscriber → plan window leads; free critical + plan healthy → free leads;
  plan critical → lowest-remaining critical leads; free-only → today's
  election, byte-identical; two plan windows → lowest remaining leads.
- QML (`tst_BarWidget`): subscriber chip shows the plan percentage; free-only
  chip unchanged.
- Rust (`v2_map` tests): the two new labels; existing subscription fixture
  already matches the live CLI format (verified against a real Megawatt
  account).
- Text guard: `primaryWindow` banned as a dead identifier after deletion.

## Out of scope

- Credits/monetary lines: still never parsed into windows (JSON-022B, v10
  product boundary).
- Workspace credit pools: no fixture exists; the parser's behavior on unknown
  lines (ignore) is already correct.
- Any new settings, schema fields, or notification changes.
