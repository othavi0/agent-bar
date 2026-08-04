# Documentation Restructure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure `docs/` by audience (guide/dev/history), correct every
audit-confirmed defect, rewrite `README.md` user-first, and fix the
CHANGELOG-insertion defect in `scripts/agent-bar-cut-release`.

**Architecture:** Documentation-only change set plus one Bash awk fix and one
test path constant. Spec: `docs/superpowers/specs/2026-08-04-docs-restructure-design.md`.
Moves land first (with reference updates keeping the link gate green per
commit), content corrections follow per file, README and index rewrites come
after the tree is final, the script fix is independent, and a final sweep
closes.

**Tech Stack:** Markdown, Bash (ShellCheck-clean), Rust test edits only in
`tests/cli_vocabulary.rs`.

## Global Constraints

- Branch: `feat/docs-restructure` (exists; spec+plan committed there).
- All new/edited active documentation is English (`tests/active_language.rs`).
- Conventional Commit subjects in English, ≤ 50 characters. Never bypass
  hooks. No AI attribution anywhere.
- `docs/specs/**` is the product contract: the ONLY permitted edits there are
  path-only link fixes for files this plan moves (no wording changes).
- `docs/superpowers/**`, `docs/adr/0001–0003` bodies, `docs/releases/*.md`
  dated notes, and CHANGELOG release sections are historical: never edit
  (exception: `docs/releases/README.md` process paragraph, Task 8; new ADR
  0005 file, Task 13).
- `docs/releases/` and `docs/adr/` directories do not move (pinned by
  `scripts/agent-bar-cut-release:50`, `.github/workflows/auto-release.yml`,
  `tests/active_docs.rs:595`, and exact-path exclusions in
  `tests/active_docs.rs:67-69` / `tests/active_legacy_scan.rs:121-123`).
- Doc gates that run inside `cargo test` and how they see your text:
  - Link gate: every relative Markdown link must resolve. Any moved file
    breaks its inbound links until you update them — same commit.
  - Grammar gate: in any fence, a line starting with `"$PLUGIN"`, `$PLUGIN`,
    or bare `agent-bar ` is parsed with the real CLI parser and must be a
    valid command. Lines containing `|`, `<`, `>`, `[`, `]`, or `...` are
    treated as synopsis and skipped.
  - JSON gate: a fenced JSON object is validated as settings only when it
    has `schemaVersion: 1` AND `providers` AND `display` keys, and as
    status only when `schemaVersion: 2` AND `helperVersion` AND
    `providers`. Other JSON fences are not validated.
  - Banner gate: never write any phrase from `TARGET_ONLY_BANNERS`
    (`tests/active_docs.rs:319-331`), e.g. "Target documentation for v10",
    "not yet implemented on the".
  - Legacy gate: do not introduce removed-feature vocabulary (Waybar, TUI,
    spend/credits, etc.) into active docs.
  - `tests/cli_vocabulary.rs` re-parses `docs/guide/commands.md` (after
    Task 1) and rejects the word "clause" in it.
- Verification for every task: `cargo test` (carries all doc gates) and
  `git diff --check`. Tasks touching `scripts/` add
  `shellcheck scripts/agent-bar-cut-release`. No QML/plugin verification
  block is needed — no QML changes exist in this plan.
- Work from the repository root. Use `git mv` for moves.

## File Map (final state)

| Path | Origin | Content change |
| --- | --- | --- |
| `docs/guide/integration.md` | `docs/integration.md` | move only |
| `docs/guide/runtime.md` | `docs/runtime.md` | + TTLs, apply fail-fast |
| `docs/guide/troubleshooting.md` | `docs/troubleshooting.md` | doctor scope, Codex tiers, lock cause |
| `docs/guide/commands.md` | `docs/commands.md` | uninstall confirmation, exit codes |
| `docs/dev/architecture.md` | `docs/architecture.md` | + Core*.js decomposition, click forwarding |
| `docs/dev/json-output.md` | `docs/json-output.md` | move only |
| `docs/dev/new-provider.md` | `docs/new-provider.md` | real trait, adapter notes |
| `docs/dev/omarchy-shell.md` | `docs/omarchy-shell.md` | manifest placeholder, Qt6 qmllint |
| `docs/dev/releasing.md` | `docs/releasing.md` | full rewrite |
| `docs/dev/agents/*.md` | `docs/agents/*.md` | move only |
| `docs/history/handoff-v10-post-merge.md` | `docs/handoff-v10-post-merge.md` | + snapshot banner |
| `docs/history/qa/v10.0.0-live-qa-2026-07-27.md` | `docs/qa/…` | + snapshot banner |
| `docs/adr/0005-auto-release-on-product-merge.md` | new | new ADR |
| `docs/README.md` | rewrite | audience index |
| `README.md` | rewrite | user-first |
| `CONTRIBUTING.md` | edit | Qt6 verification block |
| `CLAUDE.md` | edit | pointer paths only |
| `scripts/agent-bar-cut-release` | edit | awk insertion fix |
| `tests/cli_vocabulary.rs` | edit | commands.md path |

---

### Task 1: Move operator docs to `docs/guide/`

**Files:**
- Move: `docs/integration.md`, `docs/runtime.md`, `docs/troubleshooting.md`,
  `docs/commands.md` → `docs/guide/`
- Modify: `tests/cli_vocabulary.rs:256`, `docs/README.md`, `README.md`,
  `CLAUDE.md` (Pointers), plus any file the grep below reveals

**Interfaces:**
- Produces: the four operator docs at `docs/guide/<name>.md` — Tasks 5–7 edit
  them at these new paths.

- [ ] **Step 1: Move the files**

```bash
mkdir -p docs/guide
git mv docs/integration.md docs/runtime.md docs/troubleshooting.md docs/commands.md docs/guide/
```

- [ ] **Step 2: Run the gate to enumerate the breakage (expected failure)**

Run: `cargo test --test active_docs --test cli_vocabulary`
Expected: FAIL — link-gate errors for every inbound reference to the four
old paths, and `cli_vocabulary` failing to read `docs/commands.md`.

- [ ] **Step 3: Find every inbound reference**

```bash
rg -n "docs/(integration|runtime|troubleshooting|commands)\.md" \
  --glob '!docs/superpowers/**' --glob '!.worktrees/**' \
  --glob '!CHANGELOG.md' --glob '!docs/releases/1*.md'
```

Update each hit to the `docs/guide/` path. Known hits and their new values:
- `tests/cli_vocabulary.rs:256` → `.join("docs/guide/commands.md")` (also
  update the `docs/commands.md` mentions in that file's comments at lines
  247, 251, 257, 265 to `docs/guide/commands.md`).
- `docs/README.md` index entries → `guide/commands.md`, `guide/runtime.md`,
  `guide/integration.md`, `guide/troubleshooting.md` (path-only swap now;
  full index rebuild happens in Task 4).
- `README.md:110` and the Documentation list (lines 128–134) → path-only
  swap (full rewrite happens in Task 14).
- `CLAUDE.md` Pointers section → `docs/guide/commands.md — private helper
  contract`, `docs/guide/runtime.md — paths, settings, cache, and privacy`.
- References from files inside `docs/guide/` to each other (e.g.
  troubleshooting linking commands) are same-directory after the move —
  verify the grep output and adjust any `../` prefixes.
- Do NOT edit hits inside `docs/specs/**` in this task unless they point at
  one of the four moved files; if they do, apply a path-only fix.

- [ ] **Step 4: Verify green**

Run: `cargo test`
Expected: PASS (all doc gates, cli_vocabulary reading the new path).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "docs: move operator docs to docs/guide"
```

---

### Task 2: Move engineering docs to `docs/dev/`

**Files:**
- Move: `docs/architecture.md`, `docs/json-output.md`,
  `docs/new-provider.md`, `docs/omarchy-shell.md`, `docs/releasing.md` →
  `docs/dev/`; `docs/agents/` → `docs/dev/agents/`
- Modify: `docs/README.md`, `CONTRIBUTING.md`, `CLAUDE.md` (Pointers),
  `src/app_identity.rs:25` (comment), plus grep hits

**Interfaces:**
- Produces: engineering docs at `docs/dev/<name>.md` — Tasks 8–11 edit them
  at these new paths.

- [ ] **Step 1: Move the files**

```bash
mkdir -p docs/dev
git mv docs/architecture.md docs/json-output.md docs/new-provider.md docs/omarchy-shell.md docs/releasing.md docs/dev/
git mv docs/agents docs/dev/agents
```

- [ ] **Step 2: Run gate to enumerate breakage (expected failure)**

Run: `cargo test --test active_docs`
Expected: FAIL with link errors for inbound references.

- [ ] **Step 3: Find and update every inbound reference**

```bash
rg -n "docs/(architecture|json-output|new-provider|omarchy-shell|releasing)\.md|docs/agents/" \
  --glob '!docs/superpowers/**' --glob '!.worktrees/**' \
  --glob '!CHANGELOG.md' --glob '!docs/releases/1*.md'
```

Known hits:
- `docs/README.md` → path-only swap to `dev/…`.
- `README.md` Documentation list → path-only swap.
- `CONTRIBUTING.md:72` (`docs/new-provider.md`) → `docs/dev/new-provider.md`;
  `CONTRIBUTING.md:94` (`docs/releasing.md`) → `docs/dev/releasing.md`.
- `CLAUDE.md` Pointers → `docs/dev/architecture.md`,
  `docs/dev/new-provider.md`.
- `src/app_identity.rs:25` doc comment mentions `docs/releasing.md` →
  `docs/dev/releasing.md` (comment only; run `cargo fmt --check` after).
- Inside the moved files, RELATIVE links change depth: in
  `docs/dev/architecture.md:153` the link `specs/v10/02-target-architecture.md`
  becomes `../specs/v10/02-target-architecture.md`. Check every moved file:

```bash
rg -n "\]\((?!http)" docs/dev/ --pcre2
```

- `docs/specs/**` hits that point at moved files (e.g.
  `docs/specs/v10/09-implementation-plan.md` referencing `docs/agents/…`):
  path-only fixes.

- [ ] **Step 4: Verify green**

Run: `cargo test && cargo fmt --check`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "docs: move engineering docs to docs/dev"
```

---

### Task 3: Archive handoff and QA snapshots in `docs/history/`

**Files:**
- Move: `docs/handoff-v10-post-merge.md` →
  `docs/history/handoff-v10-post-merge.md`; `docs/qa/` → `docs/history/qa/`
- Modify: both moved files (banner), `docs/README.md`,
  `docs/specs/v10/README.md` (pointer path only), grep hits

- [ ] **Step 1: Move**

```bash
mkdir -p docs/history
git mv docs/handoff-v10-post-merge.md docs/history/
git mv docs/qa docs/history/qa
```

- [ ] **Step 2: Prepend the snapshot banner to `docs/history/handoff-v10-post-merge.md`**

Insert immediately after the H1 title line:

```markdown
> Historical snapshot (2026-07-27), kept as a delivery record. Later
> releases (10.1.0–10.3.0) and subsequent master work supersede parts of
> this document. The `appliedSettings` cold-start residual listed below was
> fixed on 2026-08-01 (commit f48f233, requirement SET-023); see
> `CHANGELOG.md` for the current state.
```

- [ ] **Step 3: Prepend the snapshot banner to `docs/history/qa/v10.0.0-live-qa-2026-07-27.md`**

Insert immediately after the H1 title line:

```markdown
> Historical snapshot (2026-07-27) of the v10.0.0 live QA. Known-residual
> item 1 (`appliedSettings` not loaded until Settings open) was fixed on
> 2026-08-01 (commit f48f233, requirement SET-023).
```

- [ ] **Step 4: Update inbound references**

```bash
rg -n "handoff-v10-post-merge|docs/qa/" \
  --glob '!docs/superpowers/**' --glob '!.worktrees/**' --glob '!CHANGELOG.md'
```

Known hits: `docs/README.md` (QA entry) and `docs/specs/v10/README.md`
("Post-merge notes and residuals" pointer) — path-only swaps to
`history/…`. Also fix relative links INSIDE the two moved files (depth
changed for the QA doc only if it links upward; run the gate to confirm).

- [ ] **Step 5: Verify green**

Run: `cargo test`
Expected: PASS, including the banner gate (the wording above contains no
`TARGET_ONLY_BANNERS` phrase).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "docs: archive v10 handoff and QA snapshots"
```

---

### Task 4: Rebuild `docs/README.md` as the audience index

**Files:**
- Rewrite: `docs/README.md`

- [ ] **Step 1: Replace the entire file with:**

```markdown
# Documentation

Active product and engineering documentation for Agent Bar.

## User and operator guide

- [Integration](guide/integration.md) — install, migration, update,
  uninstall, ownership, and rollback.
- [Runtime](guide/runtime.md) — owned paths, settings, cache, privacy, and
  state.
- [Troubleshooting](guide/troubleshooting.md) — typed provider and plugin
  failures.
- [Commands](guide/commands.md) — private helper diagnostics and recovery
  grammar.

## Engineering

- [Architecture](dev/architecture.md) — shared service, Rust helper, and
  data flow.
- [JSON output](dev/json-output.md) — status schema v2.
- [New provider](dev/new-provider.md) — adapter and fixture checklist.
- [Omarchy integration](dev/omarchy-shell.md) — Quattro plugin contract.
- [Releasing](dev/releasing.md) — automatic release pipeline and manual
  boundary.
- [Agent process](dev/agents/domain.md) — reading order, plus the
  [issue tracker](dev/agents/issue-tracker.md) and
  [triage labels](dev/agents/triage-labels.md).
- [ADRs](adr/README.md) — durable architectural decisions.
- [Domain vocabulary](../CONTEXT.md) — canonical terms.

## Releases

- [Release notes](releases/README.md) — one tracked file per published
  version, consumed by the automatic release pipeline.

## Canonical v10 package

- [Specification index](specs/v10/README.md)
- [Implementation plan](specs/v10/09-implementation-plan.md)
- [Grok runbook](specs/v10/10-grok-execution-runbook.md)
- [Requirements matrix](specs/v10/REQUIREMENTS_MATRIX.md)

## Historical records

- [v10 post-merge handoff](history/handoff-v10-post-merge.md) — 2026-07-27
  snapshot.
- [v10.0.0 live QA](history/qa/v10.0.0-live-qa-2026-07-27.md) — post-release
  TEST-035…042 matrix and screenshots.

`CHANGELOG.md` release sections 9.0.0 and older, ADR bodies 0001–0003, the
dated snapshots under `docs/history/`, and `docs/superpowers/**` preserve
earlier design and delivery history. The Unreleased changelog section and
the ADR index remain active.
```

- [ ] **Step 2: Verify green**

Run: `cargo test`
Expected: PASS (every listed link resolves).

- [ ] **Step 3: Commit**

```bash
git add docs/README.md
git commit -m "docs: rebuild documentation index by audience"
```

---

### Task 5: Correct `docs/guide/commands.md` (uninstall + exit codes)

**Files:**
- Modify: `docs/guide/commands.md`

Ground truth: `src/cli/mod.rs:560` (confirm_uninstall unconditional),
`src/cli/mod.rs:461-499` (non-TTY JSON path), `src/plugin/maintenance.rs:939`
(TTY phrase), `tests/cli.rs:1144-1173` (JSON confirmation matrix),
`src/cli/mod.rs:890-928` (login exit passthrough), `src/cli/mod.rs:863-873`
(status → SERIALIZATION), `src/cli/exit.rs` (reserved codes).

- [ ] **Step 1: Replace the Uninstall section body**

Current text (lines 105-106): "Standard uninstall preserves settings and
migration backups. Purge requires an explicit UI selection or interactive
terminal confirmation." Replace with:

````markdown
Both forms require confirmation before any mutation:

- On a TTY, type the exact phrase `uninstall agent-bar` at the prompt.
- On non-TTY stdin, provide a strict JSON confirmation document:

  ```json
  {
    "schemaVersion": 1,
    "operation": "uninstall",
    "confirmed": true,
    "purgeSettingsAndBackups": false
  }
  ```

  `purgeSettingsAndBackups` must match the invoked form (`true` only for
  `uninstall purge`), `confirmed` must be `true`, and trailing bytes after
  the JSON object are rejected.

Standard uninstall preserves settings and migration backups. Purge
additionally removes settings and owned backups.
````

(In the target doc the JSON fence is indented inside the list item as
shown; keep the JSON keys exactly as written — the JSON gate will not try
to validate it because it has no `providers`/`display` keys.)

- [ ] **Step 2: Replace the exit-code table and add the login note**

Replace the current table (lines 126-134) with:

```markdown
| Code | Meaning |
| --- | --- |
| `0` | Request processed; provider failures may still be typed data |
| `1` | Generic operation failure, including login pre-flight failures |
| `2` | CLI grammar or unsupported value |
| `3` | Settings/input validation surfaced by `config` commands |
| `4` | Status/schema/serialization invariant; `status` also exits 4 when settings fail to load |
| `5` | Plugin integration or transaction failure |
| `70` | Unexpected internal failure |

`login <provider>` passes the delegated provider CLI's own exit code
through verbatim when the login command runs and fails; the reserved codes
above apply to the helper's own failures.
```

- [ ] **Step 3: Verify green**

Run: `cargo test --test active_docs --test cli_vocabulary && cargo test`
Expected: PASS (no "clause" wording introduced; all command examples
unchanged and still parse).

- [ ] **Step 4: Commit**

```bash
git add docs/guide/commands.md
git commit -m "docs: correct uninstall and exit code contract"
```

---

### Task 6: Correct `docs/guide/troubleshooting.md`

**Files:**
- Modify: `docs/guide/troubleshooting.md`

Ground truth: `src/plugin/doctor.rs:36-113`, `src/cli/mod.rs:389-403`,
`src/providers/adapters.rs:282-305`, `src/settings/store.rs:92-98`.

- [ ] **Step 1: Replace the doctor description (lines 15-17)**

Current: "Doctor is read-only. It reports bundle/version integrity,
settings/cache validity, shell entry placement, provider discovery, legacy
ownership, and incomplete transactions without printing credentials or
account identifiers." Replace with:

```markdown
Doctor is read-only. `doctor scan` checks a fixed list of legacy artifact
paths from previous Agent Bar generations, classifies each through the
ownership rules, and reports the evidence. It does not check bundle
integrity, settings or cache validity, shell entry placement, or
transaction journals, and it never prints credentials or account
identifiers.
```

- [ ] **Step 2: Replace the Codex paragraph (lines 44-46)**

Replace with:

```markdown
Ensure the Codex CLI is logged in. Agent Bar collects Codex rate limits in
three ordered tiers: `codex app-server` JSON-RPC `account/rateLimits/read`
first, then an explicit `~/.codex/rate-limits.json` read when the file
exists, and finally the newest valid rate-limit events under
`~/.codex/sessions`.
```

- [ ] **Step 3: Extend "Settings do not save" (after line 80)**

Append to that section:

```markdown
While a maintenance operation (update or uninstall) holds the exclusive
maintenance lock, `config apply` waits for the lock after validating; it
completes once maintenance finishes, and the settings file is untouched
until then.
```

- [ ] **Step 4: Verify green and commit**

Run: `cargo test`
Expected: PASS.

```bash
git add docs/guide/troubleshooting.md
git commit -m "docs: fix doctor scope and codex fallback tiers"
```

---

### Task 7: Extend `docs/guide/runtime.md`

**Files:**
- Modify: `docs/guide/runtime.md`

Ground truth: `src/providers/catalog.rs:182,199,230,257` (TTLs),
`src/settings/store.rs:92-98,263-283` (apply gate).

- [ ] **Step 1: Extend the Cache section (after the stale sentence, line 68)**

```markdown
Per-provider cache TTLs are fixed in the catalog: Claude 300 seconds;
Codex, Amp, and Grok 90 seconds each.
```

- [ ] **Step 2: Extend the Settings section (after "File mode is `0600`.")**

```markdown
While maintenance holds the exclusive lock, apply validates first and then
waits for the lock; the settings file is untouched until the lock is
granted.
```

- [ ] **Step 3: Verify green and commit**

Run: `cargo test`
Expected: PASS.

```bash
git add docs/guide/runtime.md
git commit -m "docs: document cache TTLs and apply lock"
```

---

### Task 8: Rewrite `docs/dev/releasing.md`; fix `docs/releases/README.md`

**Files:**
- Rewrite: `docs/dev/releasing.md`
- Modify: `docs/releases/README.md` (lines 12-14)

Ground truth: `.github/workflows/auto-release.yml`,
`scripts/agent-bar-cut-release`,
`docs/superpowers/specs/2026-08-03-auto-release-design.md`.

- [ ] **Step 1: Replace `docs/dev/releasing.md` entirely with:**

````markdown
# Releasing Agent Bar

Releases are automatic. Every push to `master` that touches a product path
(`src/**`, `assets/**`, `scripts/**`, `Cargo.toml`, `Cargo.lock`) triggers
`.github/workflows/auto-release.yml`, which cuts a patch release and
publishes it with all assets in a single run. Docs-only merges cut nothing.
See [ADR 0005](../adr/0005-auto-release-on-product-merge.md).

The release is one architecture-specific Omarchy plugin bundle. There is no
standalone binary tarball, AUR package, cargo-binstall metadata, or global
installation.

## Automatic pipeline

1. `scripts/agent-bar-cut-release` bumps the patch version in `Cargo.toml`
   and the lockfile, writes `docs/releases/{version}.md` from the
   Conventional Commit subjects since the last tag, and prepends a matching
   CHANGELOG section below `[Unreleased]`. Preview locally:

   ```bash
   scripts/agent-bar-cut-release --dry-run
   ```

2. Rust gates run against the bumped tree: `cargo fmt --check`,
   `cargo test`, `cargo clippy --all-targets -- -D warnings`. A red gate
   stops the run before anything is pushed.
3. The bump is committed as `chore: release {version}` and tagged
   `v{version}` locally, so the bundle's source commit names the exact
   tagged commit.
4. The plugin bundle and release files are built and verified: assemble,
   inventory and mode checks, `scripts/check-version`, and a sha256
   self-check.
5. Only then: push `master` with the tag and create the GitHub release with
   the archive, checksum, metadata, and LICENSE attached in one call. A
   public release never exists without its assets.

Guards:

- The workflow skips its own `chore: release` commit, so a cut cannot
  re-trigger itself.
- A no-cancel concurrency group serializes rapid merges into one queue.
- `workflow_dispatch` runs the same pipeline manually.

The QML/Quattro gates (`omarchy plugin validate`, Qt6 `qmllint`, ShellCheck
of the bundled terminal helper) do not run on the Ubuntu release runner,
which has no Omarchy runtime. They run at the pre-merge checkpoints on
Omarchy hosts; the release consumes that accepted evidence.

## CHANGELOG convention

`CHANGELOG.md` keeps a permanent `## [Unreleased]` section (the active-doc
gates read only that slice). It stays empty in the normal flow: release
sections are generated at cut time from commit subjects. Do not hand-write
entries that a later cut would duplicate.

## Release identity

The following must match exactly:

- `Cargo.toml` package version;
- `manifest.json` version (substituted from the build-time placeholder);
- `bundle.json` version;
- private helper `version` output;
- archive filename;
- metadata version, target, Omarchy contract, minimum Quickshell version,
  source commit, archive size, and archive SHA-256;
- checksum sidecar and metadata archive SHA-256;
- the release tag.

Artifact set for a version:

```text
agent-bar.usage-{version}-x86_64-unknown-linux-gnu.tar.zst
agent-bar.usage-{version}-x86_64-unknown-linux-gnu.tar.zst.sha256
agent-bar.usage-{version}-x86_64-unknown-linux-gnu.metadata.json
LICENSE
```

## Manual boundary

Automatic cuts are always patch bumps. Minor and major releases remain
human-driven: set the version deliberately, then run the same pipeline via
`workflow_dispatch`. Merging to `master` is the release decision for patch
versions; there is no separate per-release authorization step.

Local bundle reproduction for debugging:

```bash
SOURCE_COMMIT="$(git rev-parse HEAD)"
cargo run --bin agent-bar-bundle -- assemble \
  output target/release-candidate/agent-bar.usage \
  source-commit "$SOURCE_COMMIT"
```
````

- [ ] **Step 2: Fix `docs/releases/README.md` lines 12-14**

Replace the sentence "The release builder and `publish.yml` consume this
path. When the file is absent at tag publish time, CI materializes notes
from the GitHub release body (or a short placeholder) so the builder path
remains satisfiable." with:

```markdown
The release builder and `.github/workflows/auto-release.yml` consume this
path. The automatic cut writes the file before committing; the workflow
fails if it is missing.
```

- [ ] **Step 3: Verify green and commit**

Run: `cargo test`
Expected: PASS. (The ADR 0005 link target is created in Task 13 — if the
link gate fails on it, reorder: run Task 13 first, then this task. Both
orders are acceptable; just keep each commit green.)

```bash
git add docs/dev/releasing.md docs/releases/README.md
git commit -m "docs: rewrite releasing for auto-release"
```

---

### Task 9: Correct `docs/dev/new-provider.md`

**Files:**
- Modify: `docs/dev/new-provider.md`

Ground truth: `src/providers/adapter.rs:72-98`,
`src/providers/adapters.rs:28-46,96-99,261-264,282-305,335-338`.

- [ ] **Step 1: Replace the Adapter section's trait block (lines 12-23)**

Replace with:

````markdown
Implement the two required methods; `discover` and `login_command` have
catalog-driven default bodies that none of the four shipped adapters
override:

```rust
pub trait ProviderAdapter: Send + Sync {
    fn descriptor(&self) -> &'static ProviderDescriptor;

    // Defaulted: catalog-driven discovery.
    fn discover(
        &self,
        env: &ExecutionEnvironment,
    ) -> Result<Discovery, CatalogError> { /* default body */ }

    // Defaulted: login argv from the catalog descriptor.
    fn login_command(
        &self,
        discovery: &Discovery,
    ) -> Result<ProcessSpec, LoginError> { /* default body */ }

    fn collect<'a>(
        &'a self,
        context: &'a CollectionContext<'a>,
        discovery: &'a Discovery,
    ) -> BoxFuture<'a, ProviderResult>;
}
```
````

(The `/* default body */` comments are intentional documentation elision —
the real bodies live in `src/providers/adapter.rs` and stay out of the
doc.)

- [ ] **Step 2: Extend the Discovery section (after line 57)**

```markdown
Consulting the collection executable is itself optional: Claude and Grok
collect purely from credential files plus HTTP and never read the
collection-discovery result; only Amp and Codex resolve the discovered
executable.
```

- [ ] **Step 3: Add adapter operational notes (new subsection after Discovery)**

```markdown
## Process invocation notes

- Amp runs its CLI with `NO_COLOR=1` and `TERM=dumb` forced into the
  environment to guarantee plain non-interactive output.
- Codex retries the app-server RPC once manually (short sleep, one re-run)
  when it times out — independent of, and in addition to, the catalog-level
  retry policy used for HTTP providers.
```

- [ ] **Step 4: Verify green and commit**

Run: `cargo test`
Expected: PASS.

```bash
git add docs/dev/new-provider.md
git commit -m "docs: match new-provider guide to real adapter"
```

---

### Task 10: Extend `docs/dev/architecture.md`

**Files:**
- Modify: `docs/dev/architecture.md`

Ground truth: `assets/omarchy/Service.qml:1-8`,
`assets/omarchy/BarWidget.qml:141-245`.

- [ ] **Step 1: Add the module decomposition (end of "Quickshell ownership", after line 52)**

```markdown
`Service.qml` stays declarative by delegating its logic to four JS modules
loaded beside it: `CoreService.js` (polling, generations, forced-refresh
coalescing), `CoreSettings.js` (draft and persisted settings flow),
`CoreMaintenance.js` (update and uninstall flow), and `CoreView.js`
(chip and popup presentation data, tooltips, severity cues).
```

- [ ] **Step 2: Extend the foreign-monitor overlay bullet (lines 45-46)**

Replace the bullet with:

```markdown
- optional foreign-monitor overlay when the popup is owned elsewhere: a
  click on that monitor's own chip strip is forwarded to the chip under the
  cursor, and any other press dismisses the popup (`dismissPopup()` clears
  ownership);
```

- [ ] **Step 3: Verify green and commit**

Run: `cargo test`
Expected: PASS.

```bash
git add docs/dev/architecture.md
git commit -m "docs: document service module decomposition"
```

---

### Task 11: Correct `docs/dev/omarchy-shell.md`

**Files:**
- Modify: `docs/dev/omarchy-shell.md`

Ground truth: `assets/omarchy/manifest.json:5`, `src/plugin/bundle.rs:25`,
project `CLAUDE.md` Verification section.

- [ ] **Step 1: Fix the manifest example (line 20)**

Change `"version": "10.0.0",` to `"version": "__AGENT_BAR_VERSION__",` and
add directly under the fence:

```markdown
`__AGENT_BAR_VERSION__` is a build-time placeholder substituted with the
crate version when the bundle is assembled.
```

- [ ] **Step 2: Fix the Validation fence (lines 115-119)**

Replace with:

```bash
omarchy plugin validate /path/to/agent-bar.usage
# PATH qmllint is a stub; the Qt6 binary path is mandatory
find /path/to/agent-bar.usage -type f -name '*.qml' -exec \
  /usr/lib/qt6/bin/qmllint -I /usr/share/omarchy/shell {} +
```

- [ ] **Step 3: Verify green and commit**

Run: `cargo test`
Expected: PASS.

```bash
git add docs/dev/omarchy-shell.md
git commit -m "docs: fix manifest example and qt6 qmllint"
```

---

### Task 12: Fix `CONTRIBUTING.md` verification commands

**Files:**
- Modify: `CONTRIBUTING.md` (lines 39-52)

- [ ] **Step 1: Replace the "QML/plugin verification" fence with the canonical block**

Copy exactly from project `CLAUDE.md` Verification:

```bash
# PATH qmllint is a stub reporting version 1.0 that stays SILENT even on an
# undefined type — the Qt6 binary path is mandatory here too
find assets/omarchy -type f -name '*.qml' -exec \
  /usr/lib/qt6/bin/qmllint -I /usr/share/omarchy/shell {} +
omarchy plugin validate assets/omarchy
# PATH qmltestrunner is Qt5 and fails SILENTLY (errors only in journald) —
# the Qt6 binary path and both env vars below are mandatory
QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
  /usr/lib/qt6/bin/qmltestrunner \
  -input tests/qml \
  -import /usr/share/omarchy/shell \
  -import assets/omarchy \
  -o -,txt
```

- [ ] **Step 2: Verify green and commit**

Run: `cargo test`
Expected: PASS.

```bash
git add CONTRIBUTING.md
git commit -m "docs: require qt6 qml verification commands"
```

---

### Task 13: Add ADR 0005

**Files:**
- Create: `docs/adr/0005-auto-release-on-product-merge.md`
- Modify: `docs/adr/README.md` (table row)

- [ ] **Step 1: Create the ADR with this content:**

```markdown
# 0005 — Automatic release on every product merge

- Status: Accepted (2026-08-03)

## Context

Through 10.3.0 every release was hand-cut: a human bumped the version,
authored notes, tagged, and published, under the original v10 authorization
boundary. The Settings update button consumes official GitHub releases, so
users only received merged fixes after someone remembered to cut a release.

## Decision

`.github/workflows/auto-release.yml` (which replaced `publish.yml`) cuts
and publishes a patch release on every push to `master` that touches a
product path (`src/**`, `assets/**`, `scripts/**`, `Cargo.toml`,
`Cargo.lock`): version bump and notes via `scripts/agent-bar-cut-release`,
Rust gates, a `chore: release {version}` commit plus tag, bundle build and
verification, then a push and a GitHub release created with all assets in
one call. A guard skips the workflow's own release commit; a no-cancel
concurrency group serializes rapid merges; `workflow_dispatch` allows a
manual run. Automatic cuts are always patch bumps; minor and major releases
remain human-driven.

## Consequences

- A public release never exists without its assets.
- Merging to `master` is the release decision for patch versions; there is
  no per-release human authorization step.
- Release notes are generated from Conventional Commit subjects since the
  last tag, not hand-authored prose.
- QML/Quattro gates do not run on the Ubuntu release runner; the release
  consumes checkpoint evidence accepted on Omarchy hosts before merge.
- Docs-only merges cut nothing.
```

- [ ] **Step 2: Add the index row in `docs/adr/README.md`**

```markdown
| [0005](0005-auto-release-on-product-merge.md) | Automatic release on every product merge | Accepted |
```

- [ ] **Step 3: Verify green and commit**

Run: `cargo test`
Expected: PASS.

```bash
git add docs/adr/0005-auto-release-on-product-merge.md docs/adr/README.md
git commit -m "docs: add ADR 0005 auto-release decision"
```

---

### Task 14: Rewrite `README.md`

**Files:**
- Rewrite: `README.md`

- [ ] **Step 1: Confirm the install contract before writing**

Run: `rg -n "AGENT_BAR_VERSION|resolve_version" install.sh | head`
Expected: matches showing the `AGENT_BAR_VERSION` override and a
latest-release default. If the variable name differs, use the real one in
the text below.

- [ ] **Step 2: Replace the entire file with:**

````markdown
# Agent Bar

Agent Bar is an Omarchy Quattro Quickshell plugin that shows normalized
quota and reset information for Claude, Codex, Amp, and Grok.

The product is one plugin, `agent-bar.usage`. Its Quickshell UI contains
compact provider chips, a consolidated popup, Settings, connection actions,
update, and uninstall. A private Rust helper ships inside the plugin for
provider collection, cache, settings, and safe maintenance.

## What it shows

- One bar chip per enabled provider: icon and used or remaining percentage.
- A hover tooltip: provider name, percentage, and state on the first line;
  the active window's label with its reset countdown and local clock time
  on the second, refreshed at hover time.
- Severity cues on the chip: `!` when a ready provider crosses the critical
  threshold, an hourglass glyph when data is stale.
- Plan tag and typed provider states.
- Normalized quota windows and reset times; the lead window shows both the
  countdown and the wall-clock reset.
- Loading, stale, missing CLI, unauthenticated, rate-limit, network, and
  provider-error states.
- A safe action when login, installation guidance, or retry is available.

Agent Bar does not show session history, charts, token costs, currency,
provider spend, balances, or credits. When a connected account exposes no
percentage window, the chip shows `—`.

## Interaction

| Action | Result |
| --- | --- |
| Hover a chip | Tooltip with state and the active window's reset |
| Left click | Open that provider; click it again to close |
| Middle click | Force one refresh of all enabled providers |
| Right click | Open Settings |
| Mouse wheel on chip | No action |

While the popup is open, clicking outside it on any monitor dismisses it;
clicks landing on another monitor's chips still reach those chips.

The popup has a vertical icon rail, one provider view at a time, one lead
percentage window with every other window as a compact row, a usage track
on every row, content-fit height, overflow-only scrolling, complete
keyboard navigation, and active Omarchy theme tokens.

## Requirements

- Omarchy Quattro with Quickshell.
- Linux x86_64 using the GNU target.
- The provider CLIs or local provider data you want to monitor.
- `curl`, `tar`, `zstd`, and `sha256sum` for the release bootstrap.
- Omarchy's `xdg-terminal-exec` route for interactive provider login.

Agent Bar never installs provider CLIs and never handles credentials.

## Installation

The bootstrap installs the latest published release:

```bash
curl -fsSLO https://raw.githubusercontent.com/othavi0/agent-bar/master/install.sh
less install.sh
bash install.sh
```

Pin a specific version with `AGENT_BAR_VERSION=10.3.0 bash install.sh`.

The bootstrap installs one verified directory:

```text
~/.config/omarchy/plugins/agent-bar.usage/
```

It does not install a global executable or package. Update and uninstall
live in the plugin Settings UI; a release is cut automatically from every
product merge, so the Settings update check always offers the latest.

## Settings

Settings are stored at:

```text
$XDG_CONFIG_HOME/agent-bar/settings.json
```

When `XDG_CONFIG_HOME` is unset, the default is
`$HOME/.config/agent-bar/settings.json`.

Fresh defaults:

```text
Providers: Claude, Codex, Amp, Grok
Order: Claude, Codex, Amp, Grok
Display: remaining
Refresh: 60 seconds
Notifications: enabled
```

Settings supports provider enablement/order, used versus remaining, refresh
interval, and one notification toggle. Persisted settings apply from
service start.

## Private helper

The bundled helper lives at:

```text
~/.config/omarchy/plugins/agent-bar.usage/bin/agent-bar
```

Quickshell uses its strict word-based CLI and JSON schema v2. Users
normally do not need it. Diagnostic examples:

```bash
PLUGIN="$HOME/.config/omarchy/plugins/agent-bar.usage/bin/agent-bar"
"$PLUGIN" status format human
"$PLUGIN" status provider claude format json cache bypass
"$PLUGIN" doctor scan
```

See [docs/guide/commands.md](docs/guide/commands.md) for the recovery
contract.

## Development

The repository uses Rust/Cargo and QML. No Node runtime or test toolchain
is used.

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

QML verification requires the Qt6 binaries — the bare PATH tools on Arch
are silent stubs. See [CONTRIBUTING.md](CONTRIBUTING.md) for the exact
commands.

## Documentation

- [Product](PRODUCT.md)
- [Documentation index](docs/README.md)
- [Architecture](docs/dev/architecture.md)
- [Helper commands](docs/guide/commands.md)
- [Runtime](docs/guide/runtime.md)
- [Troubleshooting](docs/guide/troubleshooting.md)
- [Canonical v10 specification](docs/specs/v10/README.md)

## License

MIT. See [LICENSE](LICENSE).
````

A screenshot section is intentionally absent: the owner will provide a
sanitized capture later; it will be added under `docs/assets/` (any name
except the legacy-pinned `agent-bar-banner.png`) together with its README
reference in one commit.

- [ ] **Step 3: Verify green and commit**

Run: `cargo test`
Expected: PASS (links resolve; helper examples parse; the settings text
block is `text`, not JSON, so no schema validation applies).

```bash
git add README.md
git commit -m "docs: rewrite README user-first"
```

---

### Task 15: Fix the CHANGELOG insertion point in `scripts/agent-bar-cut-release`

**Files:**
- Modify: `scripts/agent-bar-cut-release:79-84`

**Why:** the awk inserts the new release section before the FIRST `## `
heading, which is `## [Unreleased]` — leaving `[Unreleased]` stranded below
the newest release. Correct behavior: insert before the first non-Unreleased
`## ` heading (i.e., directly below the `[Unreleased]` block).

- [ ] **Step 1: Reproduce the defect on a fixture (expected wrong order)**

```bash
S=/tmp/claude-1000/-home-othavio-Projects-agent-bar/54b50c9a-71bc-4f5c-b0a1-1b463c57532b/scratchpad
mkdir -p "$S" && cd "$S"
printf '# Changelog\n\nIntro.\n\n## [Unreleased]\n\n## [10.3.0] - 2026-08-01\n\n- old\n' > cl.md
awk -v entry='## [10.3.1] - TEST' '
  BEGIN { inserted = 0 }
  /^## / && !inserted { print entry; print ""; inserted = 1 }
  { print }
  END { if (!inserted) { print ""; print entry } }
' cl.md
```

Expected: `## [10.3.1] - TEST` printed BEFORE `## [Unreleased]` — the
defect.

- [ ] **Step 2: Verify the fixed awk on the same fixture**

```bash
awk -v entry='## [10.3.1] - TEST' '
  BEGIN { inserted = 0 }
  /^## / && !inserted && $0 !~ /^## \[?Unreleased/ {
    print entry
    print ""
    inserted = 1
  }
  { print }
  END { if (!inserted) { print ""; print entry } }
' cl.md
```

Expected order: `## [Unreleased]`, then `## [10.3.1] - TEST`, then
`## [10.3.0] - 2026-08-01`.

- [ ] **Step 3: Apply the same change to the script**

In `scripts/agent-bar-cut-release`, change the awk condition line

```text
  /^## / && !inserted { print entry; print ""; inserted = 1 }
```

to

```text
  /^## / && !inserted && $0 !~ /^## \[?Unreleased/ { print entry; print ""; inserted = 1 }
```

(keeping the rest of the awk program identical).

- [ ] **Step 4: Verify**

```bash
shellcheck scripts/agent-bar-cut-release
scripts/agent-bar-cut-release --dry-run
cargo test
```

Expected: ShellCheck clean; dry-run prints `next-version: 10.3.1`, a notes
path, and a notes body (no files mutated); cargo test green.

- [ ] **Step 5: Commit**

```bash
git add scripts/agent-bar-cut-release
git commit -m "fix: insert release entry below unreleased"
```

Note for the reviewer: because this touches `scripts/`, merging the branch
will trigger an automatic 10.3.1 cut carrying the pending commits — decided
and accepted by the owner in the spec.

---

### Task 16: Final sweep

**Files:** none new — verification only (plus fixes for anything found).

- [ ] **Step 1: Stale-path sweep**

```bash
rg -n "docs/(integration|runtime|troubleshooting|commands|architecture|json-output|new-provider|omarchy-shell|releasing|agents|qa)[/.]|docs/handoff-v10" \
  --glob '!docs/superpowers/**' --glob '!.worktrees/**' \
  --glob '!CHANGELOG.md' --glob '!docs/releases/1*.md' --glob '!docs/history/**'
```

Expected: no hits outside `docs/guide|dev|history` self-references. Fix any
stragglers.

- [ ] **Step 2: publish.yml sweep**

```bash
rg -n "publish\.yml" --glob '!docs/superpowers/**' --glob '!CHANGELOG.md' \
  --glob '!docs/releases/1*.md' --glob '!.worktrees/**'
```

Expected: no hits in active docs (dated release notes keep theirs).

- [ ] **Step 3: Full checkpoint**

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
git diff --check
shellcheck scripts/agent-bar-cut-release
```

Expected: all green.

- [ ] **Step 4: Review the complete diff**

```bash
git log --oneline master..HEAD
git diff master --stat
```

Check: no secrets, no unrelated changes, no edits under `docs/superpowers/`
or to spec wording (path-only link fixes excepted), no AI attribution
anywhere.

- [ ] **Step 5: Commit any sweep fixes**

```bash
git add -A
git commit -m "docs: final restructure sweep"
```

(Skip the commit if the sweep found nothing.)
