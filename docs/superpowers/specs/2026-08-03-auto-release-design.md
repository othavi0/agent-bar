# Auto-release on master — design

Owner decision (2026-08-03): the Settings update button must offer an update
after every product merge to `master`, not only after a hand-cut release. The
button's contract does not change: it keeps consuming official GitHub
releases with archive + sha256 + metadata, semver strictly-newer gating, and
rollback. What changes is that releases stop being manual.

## Shape

One new workflow, `.github/workflows/auto-release.yml`, replaces
`publish.yml` (its bundle/release-file steps move in; the `release:
published` trigger dies so the two paths cannot double-upload).

Trigger: `push` to `master` filtered to product paths (`src/**`, `assets/**`,
`scripts/**`, `Cargo.toml`, `Cargo.lock`), plus `workflow_dispatch` for a
manual cut. A guard skips the run when the head commit is the workflow's own
`chore: release` bump, and a concurrency group serializes rapid merges.

Steps, in an order chosen so failure never half-publishes:

1. `scripts/agent-bar-cut-release` bumps the patch version in `Cargo.toml`
   (+ lockfile), writes `docs/releases/{v}.md` from the Conventional Commit
   subjects since the last tag, and prepends a matching CHANGELOG section.
   The script is Bash, argv-safe, ShellCheck-clean, and has `--dry-run`.
2. Rust gates run against the bumped tree (`fmt`, `test`, `clippy`). A red
   gate stops everything; `master` is untouched because nothing was pushed.
3. The bump is committed (`chore: release {v}`) and tagged `v{v}` locally,
   so the bundle's `sourceCommit` names the exact tagged commit.
4. The plugin bundle and release files are built and verified exactly as
   `publish.yml` did (assemble, inventory, `check-version`, sha256 check).
5. Only then: push `master` with the tag, and `gh release create v{v}` with
   the archive, sha256, metadata, and LICENSE attached in the same call.
   A public release therefore never exists without its assets — the update
   check treats an asset-less claiming release as a command error, so this
   ordering is what protects every installed copy.

## Non-goals

- No dev channel, no rolling tag, no update-from-commit in the product:
  BUNDLE-024/025 stand unchanged.
- No minor/major automation: an auto-cut is always a patch bump. Human
  releases for minor/major remain possible via the same script + dispatch.
- Docs-only merges cut nothing (path filter).

## Failure modes

- Gates red → run fails before any push; next merge tries again.
- Release creation fails after the push → `master` carries a bumped version
  with no release; re-running via `workflow_dispatch` cuts the next patch.
  Gap versions are harmless under strictly-newer semver.
