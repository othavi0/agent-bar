# Agent Bar v11 — Copy and Language Design

Status: approved design, not yet planned or implemented.
Date: 2026-07-30.
Companion to `2026-07-30-visual-update-design.md`. Where the two overlap, the
visual design owns layout and this document owns wording.

## 1. Scope

Two requests, one document:

1. Every active surface is English, and a test enforces it. Today no test does.
2. Every error, warning and notification is rewritten to be direct.

Inventory: a seven-agent sweep found **685 unique user-facing strings** —
212 in the GUI, 431 in the CLI and `install.sh`, 40 reaching only journald.
Measured defects in the GUI set: 21 strings leak internal vocabulary, 9 are
passive, 3 exceed twelve words, 0 apologise or hedge. The copy was never
sycophantic; it was bureaucratic.

## 2. Decisions

1. **Voice:** the ten rules in section 4, applied to every GUI string.
2. **Language:** `CHANGELOG.md`, `docs/adr/0001..0003` and the one affected
   code line are translated to English. The `docs/superpowers/**` tree is left
   as it is — 73 files, ~47,000 lines of past session plans and specs. It is a
   build record, not documentation, and translating it buys no reader.
   New files written there are English; this document and the visual design
   already are.
3. **The word "provider" stays** on screen. It is current in developer tools
   and it keeps the UI aligned with the logs.
4. **The CLI is in scope**, all 431 strings — but under section 7's narrower
   rules, not the GUI voice.
5. **A language gate is added.** Without it the rule is honoured only by
   discipline, which is exactly how the drift happened.

## 3. Language gate

### 3.1 Detection

A tracked text file must contain no **alphabetic non-ASCII character**.

Alphabetic is the operative word. It flags letters such as `U+00E3`, `U+00E7`,
`U+00E9` and `U+03B1`; it does not flag the characters this project
legitimately uses — Nerd Font glyphs (Private Use Area, category `Co`), and the
punctuation and symbols at `U+2014`, `U+00B7`, `U+2026` and `U+231B`. Measured
against the current repository the rule produced 80 hits and exactly **two**
false positives, both legitimately allowlistable. Precision is 97.5% with no
tuning.

This paragraph names those characters by code point rather than by glyph on
purpose: a document that defines the gate should survive it.

### 3.1.1 What the rule does not catch

A Portuguese-word-list approach was tried first. It reported an order of
magnitude fewer lines, which is why the character rule is the primary signal —
but the two methods fail in opposite directions, and the character rule has a
blind spot that must be stated rather than glossed over: **unaccented
Portuguese passes straight through it.**

Measured on the files this design translates:

| File | Lines the rule catches | Portuguese lines it cannot see |
| --- | --- | --- |
| `CHANGELOG.md` | 206 | 16 |
| `docs/adr/0001..0003` | 14 | 0 |

So the rule finds roughly 93% of the Portuguese in the worst file and all of it
in the rest. Two consequences follow, and both are requirements, not caveats:

1. **The one-time cleanup is done by reading, not by chasing the gate.** A
   translator who stops when the test turns green leaves those sixteen lines
   behind. The implementation plan states this explicitly and pairs the gate
   with a word-list sweep as a second pass.
2. **For regression prevention the blind spot is tolerable.** New Portuguese
   prose entering the repository is a paragraph, not a line, and a Portuguese
   paragraph carries an accent with near-certainty. The rule is a tripwire
   against drift, not a translator.

### 3.2 Configuration

- **Scanned:** every file reported by `git ls-files`, excluding binary
  extensions.
- **Excluded path:** `docs/superpowers/**`, per decision 2.
- **Allowlist:** `src/support/redact.rs:97`, where an accented test string is a
  deliberate fixture for the ANSI and control-character stripper. The allowlist
  is path-scoped and each entry carries a one-line reason.
- **Failure output:** file, line, the offending characters, and the line
  content, so the fix is obvious from the test output alone.

### 3.3 Contract change

`CLAUDE.md` currently exempts "historical changelog entries, ADR bodies
0001–0003, and `docs/superpowers/**`". After the translation only the last
exemption is true. The sentence is rewritten to name `docs/superpowers/**`
alone, and to state that the gate enforces the rest.

## 4. Voice rules

The register is a tired senior engineer: say what broke, say what to do, stop
talking.

1. **Name before category.** The title says *what*, not *what kind*.
   `Codex returned no limits`, never `Provider error`.
2. **No internal vocabulary.** `provider` as a *label* is allowed by decision
   3; `adapter`, `schema`, `payload`, `envelope`, `bundle`, `collect`,
   `clause`, `snapshot` are not.
3. **Active voice.** `Codex hit a rate limit`, not `The provider rate-limited
   this request`.
4. **No repetition.** If the title said it, the body is silent. If the button
   said it, the body is silent.
5. **Word ceiling.** Title 5, body 10, button 2.
6. **Fixes are imperative.** `Install it.` — never `You may want to…`
7. **No ceremony.** No apology, no "unfortunately", no "Final confirmation:",
   no exclamation marks.
8. **No invented action.** If the user can do nothing, there is no button.
9. **Fixed punctuation.** Sentences take a period. Titles, labels and buttons
   do not.
10. **Never blame the user.** The failure belongs to the system or the
    provider.

## 5. GUI rewrites

`{Name}` is the real provider name. No current title contains it.

### 5.1 Typed state titles — `CoreView.stateTitle`

| State | Now | New |
| --- | --- | --- |
| `loading` | `Loading` | none; the mode is already a skeleton |
| `ready`, no windows | `No percentage usage` | `{Name} reports no quota` |
| `stale` | `Stale` | none; moves to the stale banner |
| `cli_missing` | `CLI not found` | `{Name} CLI is not installed` |
| `unauthenticated` | `Authentication required` | `Not signed in to {Name}` |
| `rate_limited` | `Rate limited` | `{Name} hit a rate limit` |
| `network_error` | `Network error` | `Cannot reach {Name}` |
| `provider_error` | `Provider error` | `{Name} returned no limits` |
| `unknown` | `Unknown` | `{Name} state is unknown` |

### 5.2 Typed state bodies — `CoreView.stateBody`

| State | Now | New |
| --- | --- | --- |
| `loading` | `Collecting provider status…` | none |
| `ready`, no windows | `Percentage usage is not available for this account` | `This account is billed another way.` |
| `stale` | `Showing the last successful result. {error}` | `Last data {age} ago · {error}` |
| `cli_missing` | `Required CLI was not found.` | `Agent Bar reads the quota through it.` |
| `unauthenticated` | `Sign in to collect usage.` | `Signing in opens the official {Name} CLI.` |
| `rate_limited` | `The provider rate-limited this request. Try again shortly.` | `Try again in a few minutes.` |
| `network_error` | `A temporary network error prevented collection.` | `Check your connection.` |
| `provider_error` | `The provider returned an unusable response.` | none |

The `ready`-with-no-windows body was the only string in the file missing a
final period.

### 5.3 Actions

| Now | New | Reason |
| --- | --- | --- |
| `Connect` | `Sign in` | `Connect` reads as networking; the action is authentication. Amends `UX-054`, which names `Connect` explicitly. |
| `View installation` | `Install guide` | Two words, same meaning. |
| `Retry` | `Retry` | unchanged |
| `Check again` | `Check again` | unchanged |

### 5.4 Bar tooltip — the enum leak

`CoreView.chipTooltip` executes `parts.push(state)` with the raw enum value.
The most-seen surface in the product renders a code identifier:

| Situation | Now | New |
| --- | --- | --- |
| healthy | `Claude · 96% · ready` | `Claude · 96%` |
| signed out | `Claude · — · unauthenticated` | `Claude · signed out` |
| rate limited | `Codex · 98% · rate_limited` | `Codex · 98% · rate limited` |
| no CLI | `Grok · — · cli_missing` | `Grok · no CLI` |
| failed | `Amp · — · provider_error` | `Amp · failed` |

`connectionLabel` would have become dead code when the visual design removed
the meta footer. Instead it is repurposed as this humaniser, in lower case
because it is a trailing qualifier. Its strings become: `` (ready, omitted),
`stale`, `loading`, `no CLI`, `signed out`, `rate limited`, `offline`,
`failed`, `unknown`.

### 5.5 Notifications

| Field | Now | New |
| --- | --- | --- |
| title, warning | `{Name} usage warning` | `{Name} {Window} is running low` |
| title, critical | `{Name} usage critical` | `{Name} {Window} is almost out` |
| body, with reset | `{Window}: {n}% used. Resets 2026-07-30T16:00:00.818843Z.` | `{n}% left. Resets in 3h 1m.` |
| body, no reset | `{Window}: {n}% used.` | `{n}% left.` |
| timestamp fallback | `Resets unknown.` | the clause is omitted |

Two engineering consequences, both in section 6.

The Quattro notification card allows two lines of summary and three of body at
`Style.font.title`. Space was never the constraint.

### 5.6 Maintenance

| Now | New |
| --- | --- |
| `Uninstall agent-bar` | `Uninstall Agent Bar` |
| `Final confirmation: uninstall will remove the plugin, settings, and backups. Click Uninstall again.` | `Deletes Agent Bar, your settings and every backup.` |
| `Final confirmation: uninstall will remove the plugin bundle. Settings stay. Click Uninstall again.` | `Deletes Agent Bar. Your settings stay.` |
| `This removes the Agent Bar plugin bundle. Settings are preserved by default.` | `Removes Agent Bar. Your settings stay.` |
| `Update Agent Bar from {a}. This replaces the plugin bundle, preserves settings, and can roll back on failure.` | `Updates {a} → {b}. Settings stay. Rolls back if it fails.` |
| `Installation type: Plugin bundle` | row deleted; only one type exists and it never varied |
| `Update check returned an unusable response.` | `Update check failed.` — the identical string already exists 34 lines above |

`agent-bar` is the package name. Every other surface says `Agent Bar`.

### 5.7 Settings

| Now | New |
| --- | --- |
| `Chip number` | `Bar shows` |
| `Refresh interval (seconds)` | `Refresh every`, with `seconds` as the field suffix |
| `Usage threshold alerts for enabled providers` | `Warn me before a quota runs out.` |
| `Loading settings…` | `Loading…` |
| `Providers` | unchanged, per decision 3 |

## 6. Cross-cutting engineering

### 6.1 The countdown humaniser must exist in Rust

`CoreView.countdownText` produces `3h 1m` and `2d 18h`. It lives only in QML.
The notification path is Rust and formats RFC3339 instead. A Rust
implementation is added, and a test asserts both produce identical output for
a shared table of inputs — the same treatment the visual design applies to the
severity thresholds.

QML keeps its copy because the popup re-humanises live on a 30-second timer.

### 6.2 One unit across the product

The bar and popup show the metric the user chose in Settings; the notification
showed `used` unconditionally. Two opposite units for one number.

The notification body follows the user's display metric, like every other
surface. If the display metric does not currently reach
`src/notifications/`, threading it there is part of the work; the
notification must not carry a second source of truth for this.

Trigger thresholds stay on `usedPercent` regardless, per the visual design —
what fires the notification and what the notification says are different
concepts.

## 7. CLI rules

The CLI is deliberately excluded from section 4. It is a private helper at
`bin/agent-bar`, its stderr is read while debugging, and Unix convention there
is lower case, no trailing period, no product-name prefix — which the current
code already follows. Applying the GUI voice would make it worse.

The CLI rewrite targets three things only:

1. **Jargon that is not a real CLI concept.** `clause` is parser vocabulary and
   becomes the user-visible word: `unknown status clause 'bogus'` →
   `unknown argument 'bogus' for status`.
2. **Duplicated messages across parse and validate paths.** `setup plugins-dir
   path must be absolute` exists in both `grammar.rs` and `mod.rs`; one wins.
3. **Messages that state a problem without the fix** where the fix is short and
   certain.

`install.sh` (50 strings) and the interactive `update` flow are the exception:
those are read by a human installing the product for the first time, and they
follow section 4.

## 8. Test impact

Locked exact strings: about 16 assertions in `tests/*.rs`, about 42 inline
tests across `src/`, plus the QML suites — `tst_ProviderStates.qml` and
`tst_Popup.qml` carry the typed state copy directly.

New tests:

- The language gate of section 3.
- Rust/QML countdown equivalence, section 6.1.
- A test asserting no GUI string matches the internal-vocabulary word list of
  rule 2, so the leak cannot return.

`notify_send_argv_shape` currently asserts `"Claude usage warning"` and a body
containing `"91% used"`. It is rewritten against the new format, and is the
canary for section 6.2.

## 9. Suggested phasing

1. **Language gate plus translation** (section 3, decision 2). Independent of
   everything else and it stops the drift immediately.
2. **GUI copy** (section 5), landed with the visual design's phase 3 and 4 so
   the rewritten states ship with the layout that hosts them.
3. **Cross-cutting engineering** (section 6): the Rust countdown and the
   notification metric. These are behaviour, not wording, and carry their own
   tests.
4. **CLI and `install.sh`** (section 7). Largest string count, smallest user
   impact; last, and safe to defer.

## 10. Out of scope

- No change to what triggers a notification, only to what it says.
- No change to provider states, the status schema, or exit codes.
- No translation of `docs/superpowers/**`, per decision 2.
- No renaming of `provider` in code, per decision 3.
