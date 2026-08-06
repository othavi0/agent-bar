# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Distribution is entirely git-native: install with `omarchy plugin add
  https://github.com/othavi0/omarchy-agent-bar.git`, update with
  `omarchy plugin update agent-bar.usage`, and remove with
  `omarchy plugin remove agent-bar.usage`. The Settings update and uninstall
  buttons delegate to those same commands as a detached transient systemd
  unit instead of staging and swapping the plugin directory themselves.
- `update apply` no longer takes a version argument; it fast-forwards
  unconditionally to whatever the distribution repository currently
  publishes. `update check` reads that repository's `bundle.json` directly
  and reports `reinstallRequired` for a pre-conversion (non-git) install
  instead of a false "up to date".
- If you installed Agent Bar before this release, the update button shows a
  one-time migration notice. Run `omarchy plugin remove agent-bar.usage`
  then `omarchy plugin add https://github.com/othavi0/omarchy-agent-bar.git`.
  Settings, cache, and backups live outside the plugin directory and survive
  the swap.

### Removed

- `install.sh` and the tarball/GitHub-Releases-asset bootstrap. Releases no
  longer attach any archive; the release tag and notes are published
  alongside a plain commit pushed to the distribution repository.
- The staged plugin-directory transaction machinery (worker copy, journal,
  `renameat2` exchange/quarantine, health-IPC polling) that used to back
  in-app update and uninstall. The Omarchy CLI now owns that mutation.

## [10.3.2] - 2026-08-06

### Changed

- docs: record chip tooltip removal
- feat: remove bar chip hover tooltip
- docs: add chip tooltip removal plan
- docs: add chip tooltip removal design spec

## [10.3.1] - 2026-08-06

### Changed

- fix: unpin release identity test
- docs: repoint MIG matrix evidence after removal
- refactor: drop unused symlink/fs path helpers
- docs: fix drift after transaction removal
- refactor: remove dormant transaction paths
- docs: fix drift in v10 arch/testing specs
- docs: describe git-native distribution
- feat: publish releases to dist repo
- feat: maintenance ui delegates via omarchy cli
- fix: close kind/id/.git mirror gaps in dist test
- feat: assemble full dist tree with metadata
- refactor: drop single-variant SetupOptions
- feat: remove tarball distribution machinery
- feat: reduce setup to settings migration
- fix: resolve uninstall tools before purge
- feat: narrow uninstall to purge plus delegation
- feat: delegate update apply to omarchy cli
- feat: update check reads dist repo receipt
- docs: add git distribution spec and plan
- docs: rewrite README with screenshot demo
- fix: fetch deps before offline release cut
- docs: rewrite README user-first
- docs: require qt6 qml verification commands
- docs: fix manifest example and qt6 qmllint
- docs: document service module decomposition
- docs: match new-provider guide to real adapter
- fix: insert release entry below unreleased
- docs: rewrite releasing for auto-release
- docs: add ADR 0005 auto-release decision
- docs: document cache TTLs and apply lock
- docs: fix lock wait and codex rpc method name
- docs: correct lock semantics in spec and plan
- docs: fix doctor scope and codex fallback tiers
- docs: correct uninstall and exit code contract
- docs: rebuild documentation index by audience
- docs: archive v10 handoff and QA snapshots
- docs: move engineering docs to docs/dev
- docs: move operator docs to docs/guide
- docs: plan documentation restructure
- docs: spec documentation audit and restructure
- feat: tighten chip numeral to text width
- feat: refresh tooltip clock at hover time
- feat: chip tooltip names window and reset
- docs: plan chip tooltip window and reset
- docs: spec chip tooltip window and reset
- fix: point auto release at master
- feat: cut a release on every product merge
- feat: lead reset shows locale wall-clock time
- fix: read current claude limits vocabulary
- fix: tighten gap between provider chips
- fix: left-align chip numeral beside icon
- fix: apply persisted settings at service start
- fix: parse real update check document in QML

## [10.3.0] - 2026-08-01

The visual and copy pass over the whole product: severity you can see, one
window that leads the popup, and every message rewritten in plain words.
Same Omarchy Quattro plugin `agent-bar.usage` with the private Rust helper at
`bin/agent-bar`; no breaking changes.

### Added

- Severity. A window at or above 95% used is Critical and at or above 90% is
  Low. The popup header carries the word, the lead window's numeral and track
  turn urgent, and a ready provider shows `!` on its bar chip. Every level
  carries a word, never colour alone. The thresholds are the notification
  ones, shared with the Rust notifier and pinned by a test that reads both
  sides, so the bar and the alert can never disagree about what counts as
  critical.
- Lead-window election. The popup used to render large whichever windows
  matched a hardcoded list of ids, silently demoting anything else. It now
  elects: a critical window leads, otherwise the one resetting soonest, with
  ties keeping the order the provider delivered. A window id nobody
  anticipated can lead without a code change.
- A usage track on every window row, not only the large one.
- The reset countdown now exists in Rust as well as QML, pinned to one shared
  table of inputs, so a notification and the popup can never render the same
  duration differently.

### Changed

- Notifications say what is running out, in your unit: `Claude Session (5h)
  is almost out` with a body of `4% left. Resets in 3h 1m.` The body follows
  the used/remaining choice from Settings instead of always saying `used`.
  What triggers the alert is still the used percentage — what fires it and
  what it says are different questions.
- The popup leads with one large window and renders every other as a compact
  row: label, track, value, countdown. Reset times read as a countdown.
- Provider states speak plainly: `Not signed in to Claude`, `Codex hit a rate
  limit`, `This account is billed another way.` The two actions are now
  `Sign in` and `Install guide`, end to end including the helper's payload.
- Settings reads `Bar shows`, `Refresh every` with `seconds` beside the
  field, and `Warn me before a quota runs out.`
- Maintenance drops the ceremony: `Deletes Agent Bar. Your settings stay.`
  and `Updates 10.2.0 to 10.3.0. Settings stay. Rolls back if it fails.` The
  second destructive click is still required and is carried by the button.
- The bar chip is built on the host's `WidgetButton`, so it sits on the same
  icon grid as neighbouring modules and inherits the host's motion. Codex
  ships a mark-grade icon; monochrome marks take the theme ink.
- The plan badge is a tag, and the rail shares one inset with the content
  instead of drawing its own frame.
- CLI errors say `argument` where they said `clause`, and four of them now
  name the fix. `install.sh` is rewritten for someone reading it once.

### Fixed

- Light themes. Sixteen `Qt.darker` calls became alpha over the foreground,
  so secondary text stops outranking primary. On the `white` theme the two
  had collapsed to identical pixels.
- The stale banner carries the last-success age, the safe error summary and
  Retry, in both stale modes rather than only one.

### Removed

- The popup's meta footer, and the absolute clock beside reset times.
- `PRIMARY_WINDOW_IDS`, the window-id allowlist the election replaces.
- The `Installation type` row, which only ever displayed one value.

## [10.2.0] - 2026-07-29

### Changed

- Window labels renamed across every provider: `Session (5h)`, `Daily (1d)`,
  `Weekly (7d)`, and bare `{n}m` for other Codex durations. Removes the
  duplicated "Reset" between the popup kicker and the humanized reset line.
- Chip tooltip reduced to `Name · percent`; the typed state is appended only
  when it is not `ready`, and the reset summary moved out (it lives in the
  popup).

### Fixed

- Settings save works: the settings write process now closes stdin after
  writing, so `config apply stdin` receives EOF instead of hanging forever.
  The hang also left the write lane busy, silently rejecting every later
  save until a shell restart.
- `agent-bar update check`/`apply` work against real GitHub (#31): the
  release metadata download now follows redirects under the closed host
  policy (≤5 HTTPS hops, `github.com`/`*.githubusercontent.com` only) like
  the archive and checksum downloads already did.
- The popup no longer opens with a few pixels of phantom scroll: the card
  height accounts for the panel border inset.

## [10.1.0] - 2026-07-29

### Fixed

- Claude usage collection works again: the OAuth request now sends the
  `Bearer` prefix, reads the `seven_day_oauth_apps` weekly bucket, dedupes
  window ids across dynamic and legacy fields, and accepts epoch `resets_at`.
- Expired Claude sessions (detected client-side before HTTP or reported by
  the server) are retryable and retain cached windows as stale instead of
  wiping them.
- Interactive `update` validates TTY before any network access, matching the
  CLI contract.
- Documented QML test runner invocation now targets the Qt6 binary with the
  required environment (the bare `qmltestrunner` on Arch is Qt5 and fails
  silently).

### Changed

- Window labels are duration-based across providers: `5h Reset`, `7d Reset`,
  `1d Reset`, `{n}m Reset`; per-model windows use the plain model name.
- Popup redesigned in layers: large primary windows with theme-accent
  tracks, per-model quiet list, stale banner with retry gated by
  `error.retryable`, and a meta footer; reset and update times are
  humanized (`2h 30m · 14:59`, `5m ago`); the stale chip cue is `⌛`.
- Claude plan badge formats the real rate-limit tier (for example
  `Max 20x`).
- Claude and Grok HTTP collection share one transient-retry helper.
- `ServiceCore.js` split into five concern modules; the QML enum mirrors are
  locked to the Rust schema by a contract test.

### Removed

- Dormant `anyhow` dependency (the dependency scan now verifies real usage)
  and the unused forced-targets cache subsystem.

## [10.0.0] - 2026-07-27

### Added

- Omarchy Quattro plugin `agent-bar.usage` with one shared `Service.qml`,
  monitor-local `BarWidget.qml` chips, consolidated popup, Settings, update,
  and uninstall.
- Private Rust helper at plugin path `bin/agent-bar` for provider collection,
  cache, settings, notifications, doctor, and transactional maintenance.
- Claude, Codex, Amp, and Grok percentage quota windows with typed states
  (`ready`, `stale`, `cli_missing`, `unauthenticated`, `rate_limited`,
  `network_error`, `provider_error`).
- Status JSON schema v2 and strict word-based helper grammar.
- Transactional setup, v9-to-v10 settings migration, ownership classification,
  backups, journaled update/uninstall, health verification, and rollback.
- Plugin-scoped `install.sh` bootstrap and release bundle builder for
  `x86_64-unknown-linux-gnu`.
- Active documentation, legacy scan, and executable doc contract tests.

### Removed

- TUI and terminal dashboard.
- Waybar and Pango output.
- Session history, charts, and local or provider-reported monetary data.
- Schema-v1 status compatibility.
- Standalone global install, AUR packaging, and cargo-binstall metadata.
- Permanent daemon and global `agent-bar` executable.

### Breaking

- Status stdout is schema v2 only.
- Product artifact is only the Omarchy plugin bundle.
- Settings live solely in `$XDG_CONFIG_HOME/agent-bar/settings.json`.

Authored release notes live at `docs/releases/10.0.0.md`.

## [9.0.0] - 2026-07-21

Complete popup redesign (Omarchy-shell) + Waybar demoted to legacy tier.
Product milestone — no JSON contract change.

### Added
- **Redesigned Widget.qml**: hero % per provider matching the chip,
  `agent-bar` title + a relative last-updated label (elapsed minutes), top
  actions become real Unicode buttons (↻ refresh, ⚙︎ settings, ❯ open TUI)
  instead of text/link. One card per provider, fixed-column grid
  (label · bar · % · reset), countdown (`1h 46m · 18:30` /
  `7d 0h · seg 16:43`) across every window. Width `540` (previously `370`),
  the same in both modes.
- **`extra` visible in the popup**: Amp credits (`$X · replenish`),
  Grok's sessions/turns/model, Claude's extra usage when present —
  previously these only existed in `--format json`, never reaching the
  screen.
- **Popup motion**: bars fill on open (M1, stagger), `↻` spins during the
  fetch (M2), button hover (M4) — gated by `menu.animations` (exposed
  read-only in `config show` as `menuAnimations`).
- **`windowKind`** (`"fiveHour" | "sevenDay" | "daily" | "context" |
  "other"`) on `QuotaWindow`, decided once in Rust per provider;
  display-level dedup (`(windowKind, resetsAt, remaining)`) consumed by
  the TUI and by Widget.qml — makes the tripled "Weekly" on Codex's Plus
  plan disappear.
- **Countdown and local timezone in the TUI**: Detail's `fmt_reset` now
  uses `format_reset_time`/`format_eta` (previously it sliced the raw
  UTC ISO string, with no countdown).
- Popup settings gain Providers (toggle + reorder), Display (segmented
  remaining/used + live bar preview), and Alerts & update (notify +
  interval) as their own panels.
- **`platform::detect()`** (`src/platform.rs`) the single Omarchy/Waybar
  decision point, used by `setup`, `update` (both branches), and TUI
  Config's Save — none of the three creates `~/.config/waybar/` from
  scratch anymore on an Omarchy-only machine.
- **`agent-bar update` reinstalls the Omarchy plugin** when the shell is
  detected, eliminating the binary↔QML drift that previously required
  manually running `setup`. `doctor` gained a check of the installed
  manifest version vs the binary.
- `src/waybar/` module grouping the Waybar contract (formerly
  `waybar_contract.rs`/`waybar_integration.rs`) as an isolated legacy
  tier.

### Changed
- **Fixed Codex mislabeling**: `build_model_windows` no longer forces
  `primary→fiveHour`/`secondary→sevenDay` when classification diverges
  — a window outside tolerance becomes `other` with a label from its
  real duration (e.g. "1h window").
- **Popup settings mode**: saving is now **button-only** — the `s`
  keyboard shortcut for saving was removed; the text hint footer becomes
  real clickable buttons.
- Settings migration **v2 → v3**: `waybar.show_percentage` is silently
  dropped and the file is rewritten at the new version.
- TUI Config hides the Waybar-exclusive fields (separators, signal,
  Waybar interval) when `platform::detect()` reports Omarchy-only.
- `docs/waybar-contract.md` and `README.md` mark Waybar as **legacy
  tier**: it works, receives fixes, gets no new features.

### Removed
- Dead legacy: `Command::Terminal` variant,
  `waybar_contract::get_all_provider_ids`, `install::ensure_amp_cli`,
  duplicated `amp_cli::AMP_INSTALL_COMMAND`, the `tokio-util` dependency,
  `ConfigField::settings_key()`, 7 orphaned `Icon` variants.

### Breaking
- None to the `--format json` contract — `windowKind` is additive,
  `schemaVersion` stays `1`.

## [8.5.0] - 2026-07-21

### Added
- **Native settings in omarchy-shell.** Right click on the widget opens
  the same popup in settings mode (`model-usage`-style): per-provider
  toggles, ↑↓ reordering, remaining/used display, notify on/off, and
  refresh interval. Left = usage (wider popup); middle = refresh. A
  “Abrir menu (TUI)” link in the footer; the full TUI still lives at
  `agent-bar menu`.
- **`agent-bar config show` / `config apply`.** JSON mini-API for the
  editable subset (`providers`, `providerOrder`, `displayMode`, `notify`)
  of `settings.json`, with normalization and validation. The plugin
  writes the interval only via `updateEntryInline` in `shell.json`
  (dual-write). `config apply` **does not** reload Waybar.

### Changed
- **Trimmed CLI help.** Showcase: menu, status, config, setup, update,
  uninstall, doctor. Internal commands (`action-right`, `assets`,
  `export`, `menu-font`) remain parseable but stay out of help. `remove`
  becomes an alias for `uninstall --yes`; `-t`/`--terminal` becomes an
  alias for `status`.
- **Docs** (`commands.md`, `omarchy-shell.md`, `architecture.md`, README)
  aligned with the Omarchy clicks and the CLI taxonomy.

### Fixed
- Left click with settings open goes back to usage without closing the
  popup.
- stdout flush barrier in the QML `config apply` (avoids an `onExited`
  vs `StdioCollector` race).

## [8.4.1] - 2026-07-21

### Fixed
- **omarchy-shell widget:** `refreshIntervalSec` now also respects the
  schema ceiling (3600s) when `shell.json` is edited by hand — previously
  only the floor (30s) was applied.

### Changed
- **Automatic AUR publishing.** The release workflow gained a
  `publish-aur` job: on every GitHub Release, CI fills in
  `pkgver`/`pkgrel`/`sha256sums` and pushes `agent-bar-bin` to the AUR
  (new version → `pkgrel=1`; same version with changed packaging →
  `pkgrel+1`; no change → skip). The package's `pkgdesc` and
  install/upgrade messages now mention omarchy-shell.
- Internal dedup of the PATH scan for the `omarchy` CLI
  (`omarchy_integration::cli_on_path`).

## [8.4.0] - 2026-07-21

### Added
- **Omarchy 4 support (omarchy-shell/Quickshell).** Omarchy 4 replaced
  Waybar with omarchy-shell; agent-bar now installs as a native third-party
  bar-widget plugin (`agent-bar.usage`): one chip per provider (icon + %
  remaining, shell-theme colors, severity mirroring the TUI) and a native
  popup with primary/secondary windows, per-model breakdown, and reset
  times. Left click opens the popup, right click opens the TUI, middle click
  forces a refresh. `agent-bar setup` detects the available bar (Waybar,
  omarchy-shell, or both) and writes the plugin as a drop-in at
  `~/.config/omarchy/plugins/agent-bar.usage/` — QML files embedded in the
  binary, version-locked to the `--format json` schema.
  `uninstall`/`remove` unregister and delete the drop-in; `update` warns to
  re-run `setup` when the drop-in exists. New flag
  `--omarchy-plugins-dir <path>` (setup, for tests/CI). Docs:
  `docs/omarchy-shell.md`.
- **`severity` documented in the JSON contract** (`docs/json-output.md`):
  optional `Window` field, coming from the provider's API, with a local
  threshold fallback (≥60/30/10) for consumers.

### Fixed
- Waybar integration untouched; the existing contract stays as it was.

## [8.3.0] - 2026-07-21

### Fixed
- **False "disconnected" with a logged-in user.** Grok considers a user
  logged in when `auth.json` has a `key` — the 6h access token (renewed by
  the Grok CLI via refresh token) no longer logs the bar out. An
  `amp usage` timeout became a transient error instead of "Not logged in".

### Added
- **Stale cache fallback:** on a transient error (network timeout, expired
  Claude token), the bar serves the last good cached data with the warning
  `⚠️ Cached data — {motivo}` in the tooltip, instead of the disconnected
  icon. `disconnected` is now reserved for a real logout. New optional
  `staleReason` field in `--format json` (docs/json-output.md).
- **Amp:** daily reset ETA (midnight UTC, the official frontend's rule) on
  the free tier, an `↻ auto-replenish` indicator, and a fallback to the
  server's raw lines in the tooltip for future `amp usage` formats.

## [8.2.2] - 2026-07-19

### Changed
- **Standalone `agent-bar update`** now re-copies icons and the terminal
  helper to the Waybar paths (`~/.config/waybar/agent-bar/icons` and
  `…/scripts`) after downloading the release. Does not re-patch
  config/modules/CSS (use `setup` if the integration changed).

## [8.2.1] - 2026-07-19

### Fixed
- **Grok's Waybar icon** — replaced the “G” placeholder with the official
  `Grok_Logomark_Light` logomark from the xAI brand pack
  (`SpaceXAI_Grok_Assets.zip`). No CSS/file-contract change
  (`grok-icon.svg`).

## [8.2.0] - 2026-07-17

**Grok** provider (Grok Build CLI) in the bar and the TUI.

### Added
- **Grok provider (Grok Build CLI)** — OAuth via `~/.grok/auth.json` and
  the sessions' `signals.json`; the bar's % is the **remaining context of
  the recent session** (not an xAI plan quota). Login via `grok login` in
  the TUI. Waybar module, icon, builder, and regression fixtures.
  **New installs** list `grok` by default; **existing settings** need to
  enable it in Config (no auto-insertion).

## [8.1.0] - 2026-07-17

Code foundations, readability hardening, and TUI polish (tracks A/B/C
after 8.0.0).

### Added
- Amp regression fixtures (`tests/fixtures/amp/`) for the legacy `$X/$Y`
  format and free-tier `% remaining`.
- Dual token label in Detail's totals: primary = input+output, suffix
  `(+N cache)` when cache exists.
- Chart legend with a `…+N` indicator when series don't fit the width.
- Help discoverability: `? ajuda` chip in Detail and a hint on the
  frame's bottom border.
- Specs: foundations/trust, product hardening, TUI polish.

### Changed
- **Codex / TUI update / Detail** modularized (no product-contract change
  from the splits).
- TUI config now shows human-readable field labels in Portuguese (at the
  time) instead of raw setting keys; the technical key still appears in
  the field's hint.
- Collapsed sidebar uses `≡` / `→` / `⚙` instead of H/L/C.
- Detail's chart uses `Min(6)` on narrow panels (&lt; 72 cols); `Min(9)`
  otherwise.
- Color tokens with ≥4.5:1 contrast against the background (`Comment`,
  `Red`, chart series).
- Pricing revalidated against the official Anthropic table (2026-07-17).

### Fixed
- **Amp Free in `% remaining`** (current CLI): only parsed the `$X/$Y`
  format and left the primary window empty in the percentage free tier.
- Waybar serialization on serde failure: stdout is never empty again;
  degraded payload with `class: agent-bar disconnected`.
- Architecture docs no longer carry TypeScript residue (`main.rs` /
  `notify.rs`).

## [8.0.0] - 2026-07-11

Complete TUI redesign (v8) + reliable usage numbers.

> **The historical numbers shown CHANGE in this update**: streaming dedup
> corrects Claude's token count (the same request was being summed N
> times — an expected ~1/3 drop in totals) and the new pricing corrects
> the cost. This is a correction, not data loss.

### Added
- **Per-model column chart** in Detail and History (√ scale, minimum
  series always visible, CVD-validated One Dark Turbo colors).
- **History with expandable days and sessions**: each day opens the
  session list (time, project, model, tokens, cost), derived from the
  session logs.
- **Persistent parse cache** (`usage.redb`): history warm-start drops from
  ~8s to ~150ms; the cache version invalidates everything when parse
  semantics change; safe degradation on corruption/lock.
- **Right-click on the Waybar module opens the TUI** focused on the
  provider, with the cache invalidated before routing.
- TUI config with waybar/tui sections and a reload-signal hint.
- 2026-07 pricing: Fable/Mythos, Sonnet 5 with automatic switchover from
  the introductory price on 2026-09-01, legacy Opus (≤4.1) separated from
  4.5+, 5m/1h cache tiers, fast mode (Opus 4.7/4.8), and `inference_geo`.
- Humanized model names on screen ("Fable 5", "Opus 4.8"…).

### Changed
- **Overview removed — the TUI now boots straight into the provider**
  (sidebar navigation; no tabs).
- **The `waybar.interval` setting now reaches the Waybar config**;
  effective default 60s — anyone who never configured the value will see
  the interval change from 120s to 60s.
- One Dark Turbo palette throughout the TUI; solid gauge with eighth-step
  precision.
- README rewritten in Portuguese; URLs migrated to `othavi0/agent-bar`.

### Fixed
- **Streaming dedup in the Claude parser**: 1 record per request (the
  last entry wins) — previously every partial streaming entry counted
  again.
- **Codex logged in with an expired token**: the app-server's JSON-RPC
  error response now fails fast instead of spinning until the 4s timeout.
- The chart's downsampled week covers all 7 days; chart empty-state
  centered; clean collapse on short terminals.
- Rare test flake touching PATH (now serialized via a shared lock).

## [7.1.0] - 2026-07-02

Round of TUI visual adjustments after real hands-on testing of the redesign.

### Added
- **"Hoje (24h)" panel on Overview**: when height remains below the cards,
  the space becomes the last-24h braille chart (same visualization as
  History) with today/7d totals in the footer. On short terminals the old
  layout stays intact.

### Changed
- **Screen-transition animation (coalesce) removed** — it failed in real
  use; navigation now switches screens instantly. Fetch sweep, critical
  pulse, and cost count-up remain.
- The help popup (`?`) now sizes itself to its content (it used to be
  60%x70% of the frame and cut off the final sections on smaller
  terminals) and dims the screen underneath while open.
- TUI copy revised: fixed accent bugs across several Portuguese labels,
  and localized the "Waybar Config" label to Portuguese.

### Fixed
- **"hoje 0 tok" / "sem uso de tokens" during loading**: while session-log
  parsing hasn't finished, History, Detail, and the cards now say
  "coletando…" instead of asserting zero about data that simply hasn't
  arrived yet.
- Session-log parsing now runs **once** per refresh (it used to run twice —
  once for today's window, once for the 7-day one), cutting history
  load time in half.
- The new panel's footer uses the same token vocabulary as the History
  table (input+output), avoiding contradictory totals between screens.

## [7.0.1] - 2026-07-02

Distribution hotfix: `agent-bar update` and `install.sh`.

### Fixed
- **`agent-bar update` was broken on every standalone install** since
  6.0.0: detection used a compile-time path (`CARGO_MANIFEST_DIR`, the CI
  runner's directory) and fell into a legacy npm mode trying to read a
  nonexistent `package.json`. Detection now starts from the real binary
  (`current_exe`): dev checkout → git flow; system/AUR install → points to
  the package manager; standalone → **real self-update** (downloads the
  latest release, mandatorily verifies sha256, atomically replaces the
  binary, and mirrors the assets).
- **A standalone `agent-bar setup` failed on standalone installs** — asset
  resolution gained the `~/.local/share/agent-bar` candidate (respecting
  `AGENT_BAR_DATA`/`XDG_DATA_HOME`), unified between update and setup.
- **`install.sh` now migrates old installations on its own**: automatic
  upgrade when the version differs (without requiring `--force`), sound
  detection of TypeScript-era binaries (which responded to `--version` with
  the module's JSON), best-effort removal of the legacy npm package
  (`@noctuacore/agent-bar`) and of old symlinks. `--force` now only serves
  to reinstall the same version.

### Changed
- The npm/bun update path removed entirely (dead legacy code).
- Self-update temporary directory now via `tempfile` (0700, atomic
  creation) instead of a manual path in `/tmp`.

## [7.0.0] - 2026-07-02

Complete redesign of the `agent-bar menu` TUI (spec and plan in
`docs/superpowers/`). The Waybar contract (modules, JSON, CSS, tooltips) is
**intact** — the change is entirely in the interactive menu.

### Added
- **Single sidebar navigation** (General / providers / History / Login /
  Waybar) with per-provider drill-down — the old 4 tabs were removed.
- **Full mouse support**: click selects/activates (sidebar, cards, chips),
  hover highlights, wheel scrolls (cards and the history table).
- **Full coverage of Claude's `/usage`**: provider migrated to the new
  OAuth API blocks `limits[]` + `spend` — session, week, per-model weekly
  limit (name from the API), and extra usage/credits, with official API
  severity and transparent fallback to the legacy fields.
- **Overview screen** with one dense card per provider: gauges with a
  per-cell gradient, real tokens/h sparkline (24h), day's cost, and a
  reliable login state; states designed for logged-out/loading/empty.
- **History screen** with a braille area chart (24h/7d via the `t` key) and
  a day × provider × tokens × cost table with scroll; Amp's row with a real
  balance and a note on the absence of local logs.
- **Hourly bucketing** for history (`usage::buckets`) — charts with real
  hourly data instead of 7 stretched daily points.
- **Motion** gated by `menu.animations`: user-initiated fetch sweep,
  coalesce on screen change, pulse/blink on critical quota (<10%), and
  cost count-up (tachyonfx).
- **Nerd Font icons** with Unicode fallback via `glyphMode`.
- **Configurable menu font**: `menu.fontFamily` (default "IBM Plex Mono")
  and `menu.fontSize`, applied by the helper via terminal flags
  (alacritty/kitty/foot/ghostty); internal command `agent-bar menu-font`.
- New settings: `menu.animations`, `menu.fontFamily`, `menu.fontSize`.

### Changed
- **Fetch, login, and save moved off the event loop**: the TUI never freezes
  anymore — spinner and per-provider progress genuinely appear, keys respond
  during the fetch, and the post-login refetch is automatic.
- **Login state derived from a real fetch** (5 states, including `erro`
  distinct from `deslogado`) — the end of `[ok]` based on file existence.
- **Terminal helper cascade** (`agent-bar-open-terminal`): honors
  `$TERMINAL` (launches the preferred terminal with font flags when
  supported; unknown → xdg path); direct alacritty now comes before the
  uwsm/xdg path to apply the font while preserving Hyprland's float.
- **MSRV corrected to 1.88** (the dependencies' real floor); new
  dependencies: `tachyonfx`, `tui-scrollview`; `tui-popup` removed.
- History's "pico HHh" and axis labels are now in local time (previously UTC).

### Fixed
- **The `r` (refresh) key never worked** — it set "Loading" without
  triggering a fetch; it now triggers a real refetch (with a guard against
  duplicate fetches).
- **The Codex login command was invalid** (`codex auth login` doesn't exist
  in the CLI; fixed to `codex login`).
- **Screen corrupted when returning from login** — a full repaint
  resynchronizes the terminal buffer.
- **Help overlay corrupted the table underneath** (text leaking "pr"/"sto")
  — area cleared with `Clear` before the popup.
- The detail's "tokens/h" sparkline was an identical hardcoded placeholder
  for every provider — replaced with real per-provider data.
- Truncated names with an abrupt cut ("Free Tie") — truncation now uses `…`.
- Race between overlapping fetch waves corrupting spinner/`last_update`.
- Token `abbrev` overflowed the unit boundary ("1000.0K" → "1.0M").
- Extra usage enabled without a configured limit rendered a
  self-contradictory gauge ("$X de $0.00") — now "usado · sem limite".

## [6.0.1] - 2026-06-21

### Fixed
- **`install.sh` corrupted the binary during `setup`.** `create_symlink` created
  a symlink `~/.local/bin/agent-bar` pointing to itself (dangling) when the
  binary was already at that path — the `install.sh` case — destroying the
  executable. Now it detects (via `canonicalize`) that the binary is already
  at the destination and skips the symlink. `cargo install` / `cargo binstall`
  (which install to `~/.cargo/bin`) were unaffected.

## [6.0.0] - 2026-06-21

Complete rewrite from TypeScript/Bun to **Rust** (single binary), preserving
byte-exact parity of the Waybar/Pango contract and the `--format json` output.

### Changed
- **Rust runtime.** The monitor is now a single Rust binary (tokio +
  reqwest/rustls) replacing the TypeScript/Bun runtime. Waybar/CLI behavior
  unchanged — byte-exact parity locked by golden snapshots against the TS
  output.
- **Full-screen TUI** rewritten in ratatui (Dashboard / Waybar / History /
  Login tabs) with a cost engine via local session logs (US$/R$). The event
  loop parses logs in the background, keeping the UI responsive from boot.
- **Distribution via static musl binary.** `install.sh` downloads the
  prebuilt tarball from the GitHub Release (sha256-verified); the AUR
  (`agent-bar-bin`) and `cargo binstall` also install the binary. Release
  build via `cargo-zigbuild`.

### Removed
- **Bun / Node / npm at runtime.** No JS runtime dependency. The
  `@noctuacore/agent-bar` npm package was discontinued — the last TypeScript
  version is preserved at tag `v5.3.0-ts-final`.

### Migration
- Old npm installations: `agent-bar doctor` detects and cleans up the
  leftovers; reinstall via `install.sh`, AUR, or `cargo binstall`.

## [5.3.0] - 2026-06-18

### Added
- **AUR package `agent-bar-bin`** (Arch). Installs a standalone binary
  (`bun build --compile`) downloaded from the GitHub Release and sha256-verified
  — without requiring Bun at the user's runtime and **without building in the
  PKGBUILD** (mitigating the "Atomic Arch" supply-chain vector). Usage:
  `paru -S agent-bar-bin && agent-bar setup`. The release workflow now builds
  and attaches the `agent-bar-<ver>-x86_64.tar.gz` tarball (+ `.sha256`) to the
  GitHub Release. PKGBUILD, `.install`, and `.SRCINFO` are versioned in
  `packaging/aur/`.

### Changed
- **System install recognized throughout the app.** A compiled binary is
  detected by `isCompiledBinary()` (the `/$bunfs` marker from
  `bun --compile`): `agent-bar setup` reads assets from
  `/usr/share/agent-bar`, generates the Waybar module with
  `exec: agent-bar` (resolved via PATH), and **skips** the `~/.local/bin`
  symlink; `agent-bar update` points to the package manager (e.g.
  `paru -Syu`) instead of trying `bun add -g`. Existing installs
  (managed/npm/dev) are unchanged.

## [5.2.0] - 2026-06-18

### Added
- **Single-provider Waybar output exposes `percentage` and `alt`** to unlock
  `format-icons`. `alt` carries the health state (`ok` / `low` / `warn` /
  `critical`, or `disconnected`) for `format-icons` keyed by state;
  `percentage` is the displayMode-aware value (the same number as `text`),
  clamped to `0..100`, for `{percentage}` in `format` or `format-icons` as an
  array. Both are **omitted** when the provider is connected but has no
  quota data — a missing window never reports `ok`. The aggregated module and
  the `--format json` contract remain unchanged.
- **On-demand refresh via `signal`** (opt-in). The new `waybar.signal` setting
  (`1..30`, default off) injects `signal: N` into every generated module;
  Waybar re-executes the module on receiving `SIGRTMIN+N`. Since the module's
  `exec` reads the 5-minute cache, a plain signal only re-renders cached data —
  to force a **fresh** fetch use the documented recipe:
  `agent-bar -p <provider> -r && pkill -RTMIN+<N> waybar` (with a Claude Code
  Stop-hook example in `docs/waybar-contract.md`).

### Docs
- `docs/waybar-contract.md`: `percentage`/`alt` output fields, `format-icons`
  examples (by state via `alt` and by percentage via array), and the
  `signal` refresh configuration + recipe.

## [5.1.0] - 2026-06-17

### Added
- **Claude: plan tier in the tooltip** (`Max 5x` / `Max 20x`). The Claude
  tooltip header now reads `rateLimitTier` from `~/.claude/.credentials.json`
  (e.g. `default_claude_max_5x`) and surfaces the multiplier that
  `subscriptionType` ("max") discarded — it previously showed just "Max".
  Plans without a multiplier (Pro, Free) are unchanged. Logic isolated in
  `deriveClaudePlan()`.
- **`docs/architecture.md`**: complete data-flow (Waybar poll → provider →
  cache → formatter → JSON/Pango output), the distinction between the two
  caches (5 min cross-process quota cache in `cache.ts` vs 5 s in-process
  settings cache in `formatters/waybar.ts`), and `BaseProvider` vs direct
  `ClaudeProvider`. Now published in the npm package.
- Documentation for the internal `action-right` command (Waybar right-click)
  in `docs/commands.md` and `docs/waybar-contract.md`, with the
  refresh-or-login logic and the generated module's fields.

### Changed
- **Claude: expired-token short-circuit.** When `expiresAt` (epoch-ms from
  the credentials) has already passed, the provider returns the expired-token
  error **without** calling the Anthropic API — the call would fail anyway
  and agent-bar never refreshes the token (the single-use refresh runs with
  Claude Code). Same error string and same login routing as `action-right`;
  just faster and works offline.

## [5.0.0] - 2026-06-17

### Removed
- **Copilot provider removed entirely** (breaking). The Copilot CLI's
  `--headless --stdio` interface is hidden/fragile (disappears without notice
  in auto-updates) and was unused. Removed: provider, CLI locator, builder,
  icon, `CopilotQuota*` types, config paths, registries (tooltip/terminal/
  TUI), the `WAYBAR_PROVIDERS` entry, and the v1→v2 settings migration that
  only existed to inject Copilot. Supported providers now: Claude, Codex, Amp.

### Added
- **Desktop notifications for low/critical quota** via `notify-send`: alerts
  when any quota window crosses 90% used (low) or 95% (critical), including
  Claude's per-model weekly windows. Piggybacks on the Waybar poll with
  dedup by state (`~/.cache/agent-bar/notify-<provider>.json`), re-arms on
  recovery, escalates low→critical. Best-effort: does nothing if `notify-send`
  is absent and only fires when the output is consumed by Waybar.
  Controlled by `notify.enabled` in settings.

### Breaking
- The `copilot` provider no longer exists. Settings that listed `copilot`
  have the entry removed automatically on load; no action needed.
- **Notifications are enabled by default** (`notify.enabled: true`). After
  updating, low-quota alerts start appearing without opt-in — disable with
  `"notify": { "enabled": false }` in `~/.config/agent-bar/settings.json`.

## [4.2.0] - 2026-06-17

### Added
- `--format json`: versioned, Pango-free JSON contract that mirrors the internal
  quota model, for non-Waybar bars (Quickshell, Eww, Ironbar). Emits all
  registered providers (`--provider <id>` for a single one); independent of the
  `waybar.providers` setting. Schema, stability policy, and a Quickshell QML
  example in [`docs/json-output.md`](docs/json-output.md).
- `--watch [--interval <seconds>]`: long-running NDJSON stream (one envelope per
  line), backpressure-aware scheduling, EPIPE-safe, fails fast on unknown
  provider.
- `--version` / `-V` flag.
- `engines.bun` in `package.json`.

### Changed
- Copilot and Amp providers now follow the `BaseProvider` `fetchRaw`/`buildQuota`
  contract — the cache stores raw provider data instead of a pre-built quota.
- Copilot "used" percentage is computed at the provider layer
  (`QuotaWindow.used`); the Waybar renderer reuses `render-pango`'s span/escape
  boundary instead of a divergent local copy.

### Fixed
- Claude: send `User-Agent: claude-code/<version>` to avoid the aggressive
  rate-limit bucket (persistent 429s) on the OAuth usage endpoint; keep the
  request abort timer armed through the response-body read.
- Waybar config patcher: bracket-aware array matching that respects strings and
  JSONC comments. The previous non-greedy regex could corrupt `config.jsonc`
  when `modules-right`/`include` contained nested brackets, and could rewrite
  commented-out lines. `removeWaybarIntegration` now backs up before mutating.
- Amp: the `amp usage` subprocess now has a timeout and is killed on hang (no
  more zombie processes per Waybar poll); auth failures are no longer cached.
- Cache writes are atomic (temp file + rename).
- CLI: explicit error on `assets`/`export` without a valid subcommand.

### Removed
- The CI-only `bun-publish-with-npm-token` helper is no longer shipped in the
  npm `files` allowlist.

## [4.1.0] - 2026-05-23

### Added
- `agent-bar doctor` command: detects and cleans `@noctuacore/agent-bar`
  leftovers (`package.json`, lockfiles, `node_modules/@noctuacore/`) in `$HOME`
  caused by `bun add` / `npm i` without `-g`.
- `setup` now warns when `$HOME` has leftover install artifacts and points to
  `agent-bar doctor`.
- Bin shim (`scripts/agent-bar`) now detects install pollution in `$HOME` on
  every invocation and prints a warning suggesting `agent-bar doctor`. Warns at
  most once per hour per UID (cached in `$XDG_RUNTIME_DIR`) so Waybar logs stay
  clean.
- `install.sh` hosted installer: zero-pollution install path via
  `curl -fsSL .../install.sh | bash`. Clones to `~/.agent-bar`, installs deps,
  and optionally runs `agent-bar setup`. Adopts the curl|bash pattern used by
  bun, deno, rustup, uv, and other serious CLI tools.

### Changed
- README now promotes the hosted install script as the primary install path.
  `bun add -g` remains documented as an alternative with explicit warning about
  the `-g` flag.
- Documentation refresh: `CONTRIBUTING.md` rewritten in English and trimmed,
  with a new "Dev install" section explaining how to wire a local checkout
  straight into Waybar. `docs/runtime.md`, `docs/integration.md`,
  `docs/commands.md`, and `docs/troubleshooting.md` updated to drop the
  outdated "legacy" label on `~/.agent-bar`, reflect `install.sh` as the
  primary install path, and document `$HOME` pollution handling.

### Removed
- `preinstall` script from `package.json` — Bun does not execute lifecycle
  scripts of dependencies by default, so the guard was silent theater. Replaced
  by a Bash-level detector in the bin shim that runs on every invocation
  regardless of package manager.

## [4.0.2] - 2026-05-19

### Changed

- `agent-bar update` now detects npm/Bun installations and updates the
  global package with `bun add -g`, instead of only handling the legacy
  `~/.agent-bar` checkout.

### Fixed

- TUI logo showed `QBAR` (the project's old name) when opening the menu. Replaced with the `AGENT BAR` block-art.

## [4.0.0] - 2026-05-15

### Added

- Setting `waybar.displayMode` (`remaining` | `used`) with toggle via TUI Configure Layout. When `used`, percentages and the bar reflect consumed quota (0% = nothing used, 100% = exhausted); colors and CSS classes remain based on health. Default: `remaining` (previous behavior preserved).

### Changed

- Renamed the project to `agent-bar` (previously `qbar`, then `agent-bar-omarchy`). Runtime state now lives under `~/.config/agent-bar` and `~/.cache/agent-bar`; Waybar module IDs use the `agent-bar` namespace.

### Removed

- Removed the `qbar` and `agent-bar-omarchy` compatibility layer entirely:
  legacy identity constants, settings/cache path migration, Waybar legacy-asset
  cleanup, the `agent-bar-omarchy` CLI symlink and `bin` alias, and the `snippets/`
  manual examples.

### Breaking

- The `agent-bar-omarchy` command no longer exists. Installations still using the
  old name must reinstall as `agent-bar`; old settings/cache under the previous
  names are not migrated.

## [3.0.0] - 2026-03-27

### Added

- Amp provider with free/credits monitoring and SVG icon
- Interactive Waybar layout configuration via `qbar setup`
- Per-provider model selection with `Configure Models`
- Window policies for quota display (both, five_hour, seven_day)
- Settings schema versioning with validation and atomic writes
- Bun dependency check at startup
- Cache management improvements with configurable TTL (5 min default)
- Codex app-server integration with dynamic window labels
- Auto-activate provider in Waybar after login
- Right-click action shows full provider info

### Changed

- Removed Antigravity provider in favor of direct Claude/Codex/Amp integration
- Streamlined cache invalidation across providers
- Updated Waybar integration to flat-onedark theme
- Improved CLI help output with better formatting
- Simplified provider integration architecture

### Fixed

- Waybar module rendering and provider toggle behavior
- Amp icon display and tooltip tree connectors
- Cache invalidation now properly deletes stale entries
- Action-right routing for provider-specific actions

## [2.0.0] - 2026-02-09

### Added

- Complete TypeScript rewrite with Bun runtime
- Interactive TUI menu with clack/prompts
- Provider architecture: Claude, Codex, Antigravity as pluggable modules
- `qbar setup` for automated Waybar configuration (config.jsonc + style.css)
- `qbar uninstall` to cleanly remove all integration files
- `qbar update` command for self-update
- Beautiful `--help` UI matching hover/status style
- Smart context detection: shows help in interactive terminal, JSON in Waybar
- Extra Usage support with timeline visualization
- Separate Waybar modules per provider with PNG icons via CSS
- Rich Catppuccin-themed tooltips with model grouping
- Provider login/logout flows with automatic Waybar refresh
- Antigravity native OAuth login and token auto-refresh
- Per-module visual separators (pill, gap, bare, glass, shadow, none)
- Ora spinner for refresh actions
- Disconnected state indicator with red icon

### Changed

- Renamed project from llm-usage to qbar
- Cache directory moved to `~/.cache/qbar/`
- Tooltip layout redesigned with box drawing characters
- Terminal output now matches hover/tooltip style
- Waybar interval set to 2 minutes

### Fixed

- Tooltip newline handling and JSON escaping
- Cache invalidation deletes file instead of writing empty object
- Null remainingFraction treated as 0% (exhausted)
- Login terminal stays open during OAuth flows
- Antigravity percentages normalization and tier grouping
- Bar rendering when filled/empty segments are zero
- Bun PATH resolution in Waybar environment

## [1.0.0] - 2026-02-04

### Added

- Initial release as Waybar LLM usage monitor
- Claude and Codex quota monitoring via shell scripts
- Antigravity cloud fallback helper scripts
- Right-click menu for login and refresh actions
- Waybar tooltip with usage bars and reset times
- Provider visibility toggling (hide when logged out)
- Logout submenu with per-provider cache cleanup
- Auto-refresh Waybar after login/logout actions
- Monospace tooltip formatting with Pango markup
- Documentation in English and PT-BR
