# Documentation Audit and Restructure — Design

- Date: 2026-08-04
- Status: approved by owner (structure, README direction, script fix); pending
  written-spec review
- Scope: documentation content and layout, plus one release-script fix. No
  product Rust/QML behavior changes.

## 1. Goal

A 15-agent adversarially-verified audit (2026-08-04) cross-checked every
active doc against the code and confirmed 20 defects with 0 refutations, plus
17 user-visible behaviors no doc mentions. The docs describe the v10.0.0-era
project; master has shipped 10.1.0–10.3.0 plus 17 unreleased commits
(tooltips, severity cues, auto-release CI). This work makes the documentation
match the code again, restructures `docs/` by audience, and rewrites
`README.md` user-first.

## 2. Audit results

### 2.1 Confirmed defects (20 findings, consolidated by file below)

**README.md**

- Install example pins `v10.0.0` — three releases behind, a build with the
  Claude collection bug fixed in 10.1.0. `install.sh` itself recommends the
  `master` bootstrap with `AGENT_BAR_VERSION` override and defaults to the
  latest GitHub release (`install.sh:3-16`, `resolve_version()`).
- No mention of the chip hover tooltip (two lines since commits
  6886741/d84e325/f294561) or the severity cues (`!` at critical threshold,
  hourglass when stale — `CoreView.js:173-209`, CHANGELOG 10.3.0).

**CONTRIBUTING.md**

- QML verification commands use bare PATH `qmllint`/`qmltestrunner`. The PATH
  `qmllint` is a stub (reports 1.0, silent on undefined types); the PATH
  `qmltestrunner` is Qt5 and fails silently. CHANGELOG 10.1.0 records the fix
  (Qt6 binaries + `QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1`) but
  CONTRIBUTING.md was never updated.

**docs/releasing.md**

- Describes a fully manual, human-authorized release flow.
  `.github/workflows/auto-release.yml` (merged 2026-08-01) bumps, tags, and
  publishes a GitHub Release on every product-path push to master with no
  per-release human gate. The doc never mentions the automatic path, the
  `chore: release` self-skip guard, the product path filter, the concurrency
  queue, or `workflow_dispatch` for manual minor/major cuts.
- Lists `omarchy plugin validate`, `qmllint`, and `shellcheck` as required
  release gates; `auto-release.yml` explicitly skips all three (Ubuntu
  runners lack the Omarchy runtime; it consumes checkpoint evidence instead).

**docs/releases/README.md, 10.0.0.md–10.3.0.md**

- Five references to `.github/workflows/publish.yml`, which was replaced by
  `auto-release.yml` in commit 4a49012. The described release-body fallback
  no longer exists; the new workflow hard-fails when the notes file is
  missing.

**docs/troubleshooting.md**

- Oversells `doctor scan`: claims bundle/version integrity, settings/cache
  validity, shell entry placement, provider discovery, and transaction checks.
  Reality: read-only ownership/legacy scan over `default_legacy_candidates()`
  only (`src/plugin/doctor.rs:36-58`; help text agrees).
- Codex fallback chain documented as two tiers; the code has three
  (app-server → explicit `~/.codex/rate-limits.json` → bounded session-log
  scan, `src/providers/adapters.rs:282-305`).

**docs/commands.md**

- Implies uninstall confirmation is purge-only. Both forms require the same
  gate: exact TTY phrase `uninstall agent-bar`, or a strict JSON confirmation
  document on non-TTY stdin (`src/cli/mod.rs:560`, help: "Both forms require
  confirmation.").
- Exit-code table wrong: `login` passes through the delegated provider CLI's
  raw exit code (only pre-flight failures use 1); a settings-validation
  failure exits 4 via `status` but 3 via `config show/apply`.

**docs/new-provider.md**

- Shows a `ProviderAdapter` trait that does not compile and misstates the
  contract: only `descriptor` and `collect` lack default bodies;
  `discover`/`login_command` have defaults no shipped adapter overrides;
  return types differ (`Result<Discovery, CatalogError>`,
  `Result<ProcessSpec, LoginError>`).
- Never says `CollectionAvailability` is optional — Claude and Grok ignore it
  entirely; only Amp and Codex consult `collection_exe()`.

**docs/omarchy-shell.md**

- Manifest example shows literal `"version": "10.0.0"`; the shipped manifest
  uses the build-time placeholder `__AGENT_BAR_VERSION__`
  (`src/plugin/bundle.rs:25`).

**docs/README.md (index)**

- Missing entries: `docs/handoff-v10-post-merge.md` (linked from the v10 spec
  README but absent here) and the three `docs/agents/` files (orphaned).

**docs/handoff-v10-post-merge.md and docs/qa/v10.0.0-live-qa-2026-07-27.md**

- Present themselves as current state but are 2026-07-27 snapshots. The
  `appliedSettings`-on-cold-start residual both list as open was fixed by
  commit f48f233 (SET-023, tested in `tst_Settings.qml`). Three releases and
  the v11 chip work happened after their scope.

### 2.2 Undocumented behavior worth documenting (17)

Chip two-line tooltip (window + reset countdown + locale clock, refreshed at
hover); chip severity cues (`!` critical, hourglass stale, accessible labels);
cross-monitor click forwarding/dismiss overlay; `Service.qml` delegating to
`CoreService/CoreSettings/CoreMaintenance/CoreView.js`; non-TTY uninstall JSON
confirmation document (strict parse, no trailing bytes); confirmation required
for both uninstall forms; per-provider cache TTLs (Claude 300 s, Codex/Amp/
Grok 90 s); Codex three-tier fallback; `config apply` validating before the
lock and waiting behind exclusive maintenance (correction 2026-08-04: the
audit initially misread this as fail-fast — `SettingsStore::apply` uses the
blocking `lock_shared`); Amp invoked with `NO_COLOR=1 TERM=dumb`;
Codex manual one-shot retry on app-server timeout; `agent-bar-cut-release
--dry-run`; auto-release self-skip guard; product path filter; concurrency
queue; auto-generated notes format (git subjects); the auto-release decision
itself has no ADR.

### 2.3 Version and release state

Cargo.toml 10.3.0; latest tag v10.3.0 (2026-08-01); HEAD 17 commits past the
tag; `CHANGELOG.md [Unreleased]` empty while 13 product commits are pending;
no release doc past 10.3.0.

## 3. Approved decisions

1. **Restructure by audience** (owner choice): `docs/guide/` (operator),
   `docs/dev/` (engineering), `docs/history/` (dated snapshots). `docs/adr/`,
   `docs/releases/`, `docs/specs/`, `docs/superpowers/` stay at their paths
   (script/CI/test pins; contract; build record).
2. **README rewritten user-first and version-agnostic** ("What it shows", not
   "What v10 shows"); install section follows `install.sh`'s own contract
   (latest release by default, `AGENT_BAR_VERSION` to pin).
3. **Screenshot deferred**: the owner will capture and provide the image
   later. The README structure reserves a spot; the image reference lands
   only together with the file (the link gate forbids dangling refs). Target
   path in `docs/assets/` with a name distinct from the legacy-pinned
   `docs/assets/agent-bar-banner.png`.
4. **No manual CHANGELOG backfill**: `[Unreleased]` must stay (active-slice
   gates fail closed without it) and stays empty; the next auto-cut generates
   entries from the same git subjects and would duplicate any backfill.
5. **Include the release-script fix** (owner choice): `agent-bar-cut-release`
   currently inserts the new section before the first `## ` heading, i.e.
   above `[Unreleased]`; fix the awk to insert after the `[Unreleased]`
   block. Consequence, stated and accepted: merging this work touches
   `scripts/`, so the merge itself triggers an automatic 10.3.1 cut that
   includes the 17 pending commits (which also fills the missed CHANGELOG
   entries).
6. **New ADR 0005** records the auto-release-on-product-merge decision.

## 4. Target tree

```
README.md                 rewritten, user-first
CONTRIBUTING.md           Qt6 verification commands (parity with CLAUDE.md)
PRODUCT.md CONTEXT.md     unchanged content, links refreshed if needed
CHANGELOG.md              untouched except by the release script
docs/
├── README.md             rebuilt complete index, by audience
├── guide/
│   ├── integration.md        from docs/integration.md
│   ├── runtime.md            from docs/runtime.md  (+ cache TTLs, lock)
│   ├── troubleshooting.md    from docs/troubleshooting.md (doctor scope,
│   │                         Codex 3 tiers, maintenance-lock cause)
│   └── commands.md           from docs/commands.md (uninstall confirmation,
│                             exit-code table corrected)
├── dev/
│   ├── architecture.md       + Core*.js decomposition, cross-monitor overlay
│   ├── json-output.md        moved unchanged (content already accurate)
│   ├── new-provider.md       real trait, operational notes
│   ├── omarchy-shell.md      manifest placeholder example
│   ├── releasing.md          rewritten around auto-release
│   └── agents/               from docs/agents/ (domain, issue-tracker,
│                             triage-labels), indexed
├── history/
│   ├── handoff-v10-post-merge.md    + dated snapshot banner
│   └── qa/v10.0.0-live-qa-2026-07-27.md  + dated snapshot banner
├── adr/                  stays; + 0005-auto-release-on-product-merge.md
├── releases/             stays (pinned by script/CI/test)
├── specs/                untouched except the handoff pointer path in
│                         specs/v10/README.md (sole spec-package touch)
└── superpowers/          untouched
```

## 5. Work specification per file

- **README.md** — full rewrite, order: what it is → what it shows (chips,
  two-line tooltip, severity cues, popup, typed states, `—` for connected
  accounts without a percentage window) → interaction table (with tooltip/
  cues) → requirements → installation (latest release; `AGENT_BAR_VERSION`
  pin) → settings → private helper → development (correct Qt6 commands,
  pointer to CONTRIBUTING) → documentation links (new tree) → license.
- **CONTRIBUTING.md** — replace the QML verification block with the Qt6
  binary paths and both env vars, matching CLAUDE.md's Verification section;
  update doc pointers to the new tree.
- **docs/guide/troubleshooting.md** — doctor section rewritten to the real
  scope; Codex chain with all three tiers; "Settings do not save" gains the
  exclusive-maintenance-lock cause.
- **docs/guide/commands.md** — uninstall: both forms confirmed, TTY phrase
  and non-TTY JSON document with its exact schema; exit-code table reworded
  to match dispatch reality (login passthrough; status 4 vs config 3 for
  settings failures).
- **docs/guide/runtime.md** — add per-provider cache TTLs and the
  `config apply` wait-behind-maintenance behavior.
- **docs/guide/integration.md** — move; refresh references (auto-release
  replaces publish.yml where mentioned).
- **docs/dev/architecture.md** — add the `Core*.js` module decomposition and
  the cross-monitor forwarding/dismiss overlay.
- **docs/dev/new-provider.md** — real trait shape (required: `descriptor`,
  `collect`; defaulted: `discover`, `login_command`), compiling signatures;
  note `CollectionAvailability` is optional (Claude/Grok ignore it); Amp env
  forcing; Codex manual retry.
- **docs/dev/releasing.md** — rewrite: automatic patch release on every
  product-path merge to master (path filter, self-skip, concurrency,
  `--dry-run`); manual minor/major via `workflow_dispatch`; honest statement
  that QML/shellcheck/plugin-validate gates run at checkpoints, not in the
  release runner; auto-generated notes format; CHANGELOG convention
  (`[Unreleased]` stays empty; sections are script-generated).
- **docs/dev/omarchy-shell.md** — manifest example uses
  `__AGENT_BAR_VERSION__` and explains the substitution.
- **docs/releases/README.md** — drop the publish.yml/release-body-fallback
  paragraph; describe auto-release.yml's hard requirement on the notes file.
  Historical release notes files (10.0.0–10.3.0) keep their `publish.yml`
  sentence — they are dated cuts; correcting history is out of scope.
- **docs/README.md** — rebuilt index: guide/dev/history sections, agents docs
  and handoff listed, historical-records paragraph updated.
- **docs/history/** — move handoff + QA doc; prepend a dated
  "Historical snapshot (2026-07-27)" banner noting later releases and the
  f48f233 fix; banner wording must pass the active-docs banner gate (check
  the test's pattern during implementation).
- **docs/adr/0005-auto-release-on-product-merge.md** — new ADR: context
  (manual flow superseded), decision (auto patch cut per product merge),
  consequences (no per-release human gate; checkpoint evidence replaces
  runner-side QML gates; manual path retained for minor/major).
- **docs/specs/v10/README.md** — update the handoff pointer path only.
- **CLAUDE.md / AGENTS.md** — update doc paths where they reference moved
  files (CLAUDE.md Pointers section; AGENTS.md only if it names any).
- **scripts/agent-bar-cut-release** — awk fix: insert the new section after
  the `[Unreleased]` block (before the second `## ` heading); ShellCheck.
- **tests/cli_vocabulary.rs** — update the `docs/commands.md` path constant.

## 6. Gates and constraints

- `tests/active_docs.rs`: relative links must resolve (covers every move);
  CLI examples must parse under the v10 grammar; JSON examples must validate;
  no target-only banners (check exact pattern before wording the history
  banner).
- `tests/active_language.rs`: everything new is English.
- `tests/active_legacy_scan.rs`: no removed-feature vocabulary in active
  docs; ADR 0001–0003 body paths and `docs/superpowers/` exclusions are
  unaffected (paths unchanged).
- `docs/releases/{next}.md` path and `docs/releases/10.0.0.md` existence are
  pinned — directory does not move.
- No production Rust/QML changes; the only non-doc diffs are the script awk
  fix and the one test path constant.

## 7. Verification

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
git diff --check
shellcheck scripts/agent-bar-cut-release
scripts/agent-bar-cut-release --dry-run   # sanity: version/notes computation
```

No QML changes, so the qmltestrunner/qmllint/plugin-validate block is not
required for this change set.

## 8. Delivery

Branch `feat/docs-restructure` from current master; Conventional Commits in
English (≤ 50-char subjects); PR for owner review. No merge without explicit
authorization. Reminder: the merge will trigger auto-release 10.3.1 (scripts/
path filter) carrying the 17 pending commits.

## 9. Out of scope

- Rewriting historical records (CHANGELOG 9.x sections, dated release notes,
  ADR 0001–0003 bodies, `docs/superpowers/**`, QA evidence content).
- Product behavior changes, new screenshots (owner provides later), CI gate
  additions (QML gates on the release runner), and any release/tag/publish
  action itself.
