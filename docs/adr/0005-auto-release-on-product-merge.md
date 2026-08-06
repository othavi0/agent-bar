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

## 2026-08-05 amendment

The git-native plugin distribution conversion
(`docs/superpowers/specs/2026-08-05-git-plugin-distribution-design.md`)
replaced the "GitHub release with all assets in one call" step. The
workflow now pushes the assembled plugin tree as one commit to the
distribution repository (`othavi0/omarchy-agent-bar`) before publishing the
product tag and GitHub Release, and the release carries no attached
archive. A failed distribution push aborts the run before the tag or
release exists, so a public release still never exists without its
distributable tree. The decision and consequences above describe the
original asset-attached model and are left as recorded; see
[docs/dev/releasing.md](../dev/releasing.md) for the current pipeline.
