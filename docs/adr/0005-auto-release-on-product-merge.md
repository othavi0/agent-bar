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
