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
