# Releasing Agent Bar

> v10 release preparation only. Publishing requires separate explicit
> authorization.

The release is one architecture-specific Omarchy plugin bundle. There is no
standalone binary tarball, AUR package, cargo-binstall metadata, or global
installation.

## Release identity

The following must match exactly:

- `Cargo.toml` package version;
- `manifest.json` version;
- `bundle.json` version;
- private helper `version` output;
- archive filename;
- metadata version, target, Omarchy contract, minimum Quickshell version,
  source commit, archive size, and archive SHA-256;
- checksum sidecar and metadata archive SHA-256;
- metadata release-notes URL and the authorized release tag;
- Git tag after authorization.

Initial target:

```text
agent-bar.usage-10.0.0-x86_64-unknown-linux-gnu.tar.zst
agent-bar.usage-10.0.0-x86_64-unknown-linux-gnu.tar.zst.sha256
agent-bar.usage-10.0.0-x86_64-unknown-linux-gnu.metadata.json
LICENSE
```

## Prepare

1. Complete every checkpoint and live QA.
2. Update `CHANGELOG.md` and migration guide.
3. Prepare English release notes.
4. Run the complete acceptance matrix.
5. Assemble the plugin twice from a clean source state.
6. Compare inventory, modes, IDs, versions, and content hashes.
7. Validate archive traversal/link/device protections.
8. Verify the checksum and closed metadata equality.

The tracked release-notes source is `docs/releases/10.0.0.md`. After every
tracked release change is committed, require a clean worktree and run:

```bash
SOURCE_COMMIT="$(git rev-parse HEAD)"
cargo run --bin agent-bar-bundle -- assemble \
  output target/release-candidate/agent-bar.usage \
  source-commit "$SOURCE_COMMIT"
cargo run --bin agent-bar-bundle -- release \
  bundle target/release-candidate/agent-bar.usage \
  output target/release-candidate/files \
  source-commit "$SOURCE_COMMIT" \
  release-notes docs/releases/10.0.0.md
```

The builder refuses a dirty/wrong HEAD and produces archive, checksum,
metadata JSON, and LICENSE from that exact commit.

## Required isolated gates

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
git diff --check

cargo build --release
cargo run --bin agent-bar-bundle -- \
  assemble target/release/agent-bar.usage

omarchy plugin validate target/release/agent-bar.usage
find target/release/agent-bar.usage -type f -name '*.qml' -exec \
  qmllint -I /usr/share/omarchy/shell {} +
shellcheck target/release/agent-bar.usage/scripts/agent-bar-open-terminal
target/release/agent-bar.usage/bin/agent-bar version
readelf -h target/release/agent-bar.usage/bin/agent-bar
```

Also run QML behavior/screenshot, migration, transaction fault matrix, docs,
legacy, and dependency gates from the canonical acceptance specification.

## Authorization boundary

The implementation worker may:

- bump target version on the feature branch;
- prepare archive/checksum and release notes;
- push the feature branch;
- open the final ready PR.

The worker may not:

- merge;
- create or push a tag;
- publish a GitHub Release;
- distribute an archive;
- update any package repository;
- skip final Codex review or live rollback evidence.

After the user merges and separately authorizes release, follow the approved
GitHub release procedure for the exact reviewed commit. Do not rebuild from a
different source state.
