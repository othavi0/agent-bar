# Testing and Acceptance

## Universal gates

- `TEST-001`: `cargo fmt --check` passes.
- `TEST-002`: `cargo test` passes.
- `TEST-003`: `cargo clippy --all-targets -- -D warnings` passes.
- `TEST-004`: `git diff --check` passes.
- `TEST-005`: The worktree contains no unrelated changes.
- `TEST-006`: Tests use fake provider executables, isolated XDG roots, a fake
  clock, and no live credentials or provider network.

## Rust contract tests

- `TEST-007`: Table-test every valid CLI clause ordering.
- `TEST-008`: Reject every duplicate, missing value, unknown word, legacy
  command, and unsupported double-dash flag.
- `TEST-009`: Validate schema-v2 success, partial failure, all-failure, stale,
  malformed percentage, bad timestamp, unknown action, and serialization
  failure.
- `TEST-010`: Cover every provider state for every provider with fixtures.
- `TEST-011`: Verify single-provider and all-provider normalization are equal.
- `TEST-012`: Verify provider timeout, process termination, output limits, retry
  classification, and login exit status.
- `TEST-013`: Verify cache hit, expiry boundary, corruption, concurrent
  cache-use, concurrent cache-bypass, and exactly-one pending forced generation.
- `TEST-014`: Verify settings read purity, strict validation, lock behavior,
  atomic replacement, permissions, and failure preservation.
- `TEST-015`: Verify notification escalation, deduplication, persistence,
  recovery, reset, stale suppression, and disabled behavior.
- `TEST-016`: Verify ownership classification, archive traversal rejection,
  symlink rejection, migration idempotency, interrupted transaction recovery,
  update rollback, and uninstall preservation/purge.

Mandatory known-defect regressions:

- Claude `token_expired` is never cached as success.
- Provider-reported spend and credit fields are not serialized or rendered.
- Cached token accounting is not retained through the removed cost subsystem.
- Single-provider requests use the same timeout and retry policy.
- Settings reads never rewrite unknown data.
- Forced refresh during a collection is not discarded.
- Provider order has one source.
- Serialization failure never writes blank stdout.
- Login cannot report success after nonzero provider exit.
- Setup never resets an existing bar position or inline layout.
- Update always activates the new QML through safe rescan.

## QML and plugin tests

- `TEST-017`: `qmllint` passes for every QML file with the installed Quattro
  import paths.
- `TEST-018`: Omarchy validates a fully assembled temporary plugin bundle.
- `TEST-019`: A fake helper drives deterministic ready, loading, stale,
  missing, unauthenticated, rate-limited, network, and provider-error states.
- `TEST-020`: Two widget instances share one service and one polling process.
- `TEST-021`: Left, middle, and right click contracts pass.
- `TEST-022`: Open/same-close/switch/monitor-transfer behavior passes.
- `TEST-023`: Delayed config load cannot overwrite a newer draft.
- `TEST-024`: Close/reopen and multiple-save races preserve the matching
  generation and immutable payload.
- `TEST-025`: Mouse wheel, touchpad-equivalent input, keyboard scroll, bounds,
  content clamping, PageUp/PageDown/Home/End, and scrollbar visibility pass.
- `TEST-026`: Tab order, Shift+Tab, Enter, Space, Escape, `j`/`k`, arrows, `r`,
  `s`, editor shortcut suppression, and visible focus pass.
- `TEST-027`: Every interactive control exposes an accessible name, role, and
  action.
- `TEST-028`: Untrusted strings render as plain text.
- `TEST-029`: Agent Bar declares no custom QML animations, and at least one
  light and one dark Quattro theme render correctly.

Required screenshot states:

```text
ready-light.png
ready-dark.png
loading-dark.png
refreshing-with-data-dark.png
stale-dark.png
cli-missing-dark.png
unauthenticated-dark.png
rate-limited-dark.png
network-error-dark.png
provider-error-dark.png
settings-clean-dark.png
settings-dirty-dark.png
settings-invalid-dark.png
maintenance-update-dark.png
uninstall-confirmation-dark.png
```

Screenshots must use real rendered QML with deterministic fixture data. HTML
mockups are design references, not acceptance evidence.

Checkpoint 2 and checkpoint 4 both run:

```bash
scripts/verify-v10-ui
```

The script recreates `target/v10-ui-evidence/`, invokes
`tst_Screenshots.qml` through `qmltestrunner`, verifies the exact inventory
above with no extra PNG, and writes sorted
`target/v10-ui-evidence/SHA256SUMS`. The checkpoint records that directory and
hash file.

## Legacy and documentation gates

- `TEST-030`: Active source, tests, active docs, manifests, workflows, and
  scripts contain no TUI, Waybar, history, chart, BRL/currency, Redb, Postcard,
  legacy status/config schema, AUR, cargo-binstall, or standalone-product
  behavior.
- `TEST-031`: The gate excludes only changelog release sections 9.0.0 and
  older, ADR bodies 0001–0003, and `docs/superpowers/**`. It scans Unreleased,
  the ADR index, and ADR 0004.
- `TEST-032`: Every active command example is exercised by CLI parser tests.
- `TEST-033`: Every active JSON example validates against the checked-in
  schema.
- `TEST-034`: Every direct Cargo dependency has a documented production or
  test owner.

Legitimate v1 contracts are exact and individually allowlisted:

```text
assets/omarchy/manifest.json             Quattro manifest schema 1
settings.json                            Agent Bar settings schema 1
bundle.json                              Agent Bar bundle receipt schema 1
*.metadata.json                          Agent Bar release metadata schema 1
update check                             Agent Bar update document schema 1
notification-state-v1.json               notification-state schema 1
uninstall confirmation stdin             confirmation schema 1
```

Legacy scans target removed v9 quota/config bridge shapes and identifiers, not
the number `1` or `schemaVersion` in isolation.

The active legacy gate scans source, tests, active docs, QML/assets, manifests,
workflows, packaging, and scripts. Its forbidden behavioral set is closed:

```text
src/tui                 src/usage               src/waybar
custom/agent-bar        format waybar           format_for_waybar
render_pango            waybar_contract         waybar_integration
WAYBAR_                 SIGUSR2                 menu-font
action-right            --waybar-dir            agent-bar menu
usage.redb              UsageRecord             UsageSummary
ExtraUsage              fxRate                  creditsBalance
freeRemaining           AGENT_BAR_DATA          AGENT_BAR_ASSET_DIR
AGENT_BAR_FORCE_COMPILED InstallKind::System     InstallKind::DevGit
InstallKind::ManagedGit InstallKind::Standalone ManagedGit
/usr/bin/agent-bar      .local/bin/agent-bar    .local/share/agent-bar
agent-bar-bin           package.metadata.binstall
packaging/aur           x86_64-unknown-linux-musl cargo-zigbuild
cargo binstall agent-bar BIN_DIR="$HOME/.local/bin" wire Waybar
Waybar is Wayland-only
ratatui                 crossterm               tui-input
throbber-widgets-tui    tachyonfx               redb
postcard
```

Path-aware checks also require every locked deletion path to be absent and no
direct Cargo dependency to lack a v10 owner. Active documentation may contain
these tokens only inside explicit negative-removal/history-policy statements;
production Rust/QML/workflows/scripts may not.

Since git-plugin-distribution (2026-08-05), `install.sh` is a locked
deletion path: the gate requires it absent rather than scanning its
content, and the four installer strings above are enforced only as
ordinary forbidden tokens across the active surface. Final `Cargo.toml`
must contain exactly
`description = "LLM quota monitor for Claude, Codex, Amp, Grok, and Antigravity."`, contain
no `package.metadata.binstall`, and declare no standalone/AUR metadata. These
checks avoid a useless global rejection of the word `Waybar` while closing
positive production surfaces.

Exact raw-input allowlists are `amp usage`, Codex `session_log`, normalized
window ID `session`,
`tests/fixtures/amp/usage-legacy-dollars.txt`,
`tests/fixtures/amp/usage-free-pct.txt`,
`tests/fixtures/status-v2/money-field.json`, and
`tests/fixtures/migration/**`. These fixtures prove rejection/migration; tests
must assert their legacy or monetary fields never reach `ProviderResult`,
schema v2, QML, cache, or logs. The bare words `usage`, `history`, `cost`,
`credits`, `TUI`, and `Waybar` are not global regexes because negative
documentation and normalized provider concepts can contain them.

## Mandatory checkpoints

1. Backend contract: CLI, JSON, providers, settings, cache, notifications.
2. Quickshell: service, widgets, popup, Settings, Maintenance, scrolling,
   icons, accessibility.
3. Migration and cleanup: transactions, v9 migration, deletion, bundle,
   active docs.
4. Final release candidate: complete isolated verification before live QA.

At each checkpoint Grok writes the approved checkpoint template and stops.
Codex independently reviews the commit range, tests, screenshots, deviations,
and requirement coverage. Blocking findings stop the next checkpoint.

## Live Omarchy QA

Live QA is authorized only after checkpoint 4 isolated gates pass.

- `TEST-035`: Back up the exact current plugin, settings, shell entry, and
  transaction state.
- `TEST-036`: Install the candidate transactionally without touching unrelated
  plugins or layout.
- `TEST-037`: In a real Quickshell/Wayland session, verify two monitors,
  exclusive map-time focus, outside-click routing, popup transfer and geometry,
  all pointer interactions, keyboard, focus, scrolling, themes, refresh,
  settings, provider states, and notifications.
- `TEST-038`: For the unpublished 10.0.0 candidate, exercise the live UI update
  check/no-update state and UI uninstall. Update apply/rollback remains a
  complete isolated end-to-end transaction gate. A real self-update smoke test
  requires a separately authorized post-release newer version.
- `TEST-039`: Capture sanitized logs and real screenshots.
- `TEST-040`: Execute and verify rollback.
- `TEST-041`: Restore the original environment after any failure.
- `TEST-042`: Do not modify general Hyprland, theme, terminal, system package,
  or other plugin configuration.

## Definition of done

v10 is done only when:

- every requirement maps to implementation and a passing acceptance check;
- every mandatory test and screenshot exists and passes;
- no blocker, undocumented deviation, skipped gate, or active legacy path
  remains;
- final Codex review accepts the release candidate;
- live QA and rollback pass;
- the user explicitly approves the final result.
