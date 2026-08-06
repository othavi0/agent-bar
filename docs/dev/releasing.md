# Releasing Agent Bar

Releases are automatic. Every push to `master` that touches a product path
(`src/**`, `assets/**`, `scripts/**`, `Cargo.toml`, `Cargo.lock`) triggers
`.github/workflows/auto-release.yml`, which cuts a patch release, pushes the
assembled plugin tree to the distribution repository, and publishes the
product release in a single run. Docs-only merges cut nothing. See
[ADR 0005](../adr/0005-auto-release-on-product-merge.md).

The distribution artifact is the assembled Omarchy plugin tree, pushed as a
single commit to
[`othavi0/omarchy-agent-bar`](https://github.com/othavi0/omarchy-agent-bar).
There is no standalone binary tarball, AUR package, cargo-binstall metadata,
or global installation.

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
   `v{version}` locally, so the assembled tree's source commit names the
   exact tagged commit.
4. The plugin tree is built and verified: the helper builds for
   `x86_64-unknown-linux-gnu`, the assemble step writes
   `target/release/othavi0.agent-bar` and runs `scripts/check-version`, and
   an inventory step checks required files, executable modes, and the
   target architecture.
5. The assembled tree is pushed to the distribution repository first. A
   failed push there aborts the run before the product tag or GitHub
   release exists (see "Append-only rule" below). Only then does the
   workflow push `master` with the tag and create the GitHub release with
   its notes; the release carries no attached files.

Guards:

- The workflow skips its own `chore: release` commit, so a cut cannot
  re-trigger itself.
- A no-cancel concurrency group serializes rapid merges into one queue.
- `workflow_dispatch` runs the same pipeline manually.

The QML/Quattro gates (`omarchy plugin validate`, Qt6 `qmllint`, ShellCheck
of the bundled terminal helper) do not run on the Ubuntu release runner,
which has no Omarchy runtime. They run at the pre-merge checkpoints on
Omarchy hosts; the release consumes that accepted evidence.

## Update-path verification

Every release must end with proof that installed plugins can actually
receive it. A green merge is not that proof: the auto-release run has
failed silently in the past (three consecutive releases before the fix in
PR #50), and a red run means the Settings update button simply never sees
the new version. Run this checklist after every product merge.

Before merging, the standing gates already cover the update contract:
`cargo test --test dist_tree_validate` mirrors `omarchy-plugin-validate`
against the assembled tree, and the append-only rule keeps the dist
repository fast-forwardable. Nothing extra is manual at that stage.

After merging:

1. **Watch the `Auto release` run to completion.** The release exists only
   when the run is green:

   ```bash
   gh run list --workflow "Auto release" --limit 1
   ```

   The run takes a few minutes after the merge. Checking an install before
   it finishes reports "up to date" — that is timing, not a defect.

2. **Confirm the distribution repository advanced by one fast-forward
   commit** (one `release: v{version}` commit on top of the previous
   history, never a rewrite).

3. **On an Omarchy host with the plugin installed, exercise the consumer
   paths in order:**

   ```bash
   # The Settings button's first stage: must report the new version.
   ~/.config/omarchy/plugins/othavi0.agent-bar/bin/agent-bar update check

   # The apply path (the Settings button delegates to this same command).
   omarchy plugin update othavi0.agent-bar

   # Must now report available: false with current == the new version.
   ~/.config/omarchy/plugins/othavi0.agent-bar/bin/agent-bar update check
   ```

   Then glance at the bar: chips must render with live data after the
   automatic shell rescan.

`omarchy update` (the system-wide update) does not update plugins by
design; installs that want it hook `omarchy-plugin-update --yes` into
`~/.config/omarchy/hooks/post-update.d/`. That path is the same
`omarchy plugin update` exercised above, so it needs no separate check.
When a user reports an update error, read `/tmp/omarchy-update.log`
first — the failure is frequently an unrelated package in the same
system update.

## Distribution repository deploy key

The dist push authenticates over SSH with a dedicated deploy key scoped to
`othavi0/omarchy-agent-bar` only, never the product repo's default token.
To provision or rotate it:

1. Generate a fresh ed25519 key pair with an empty passphrase, so the
   workflow can use it unattended:

   ```bash
   ssh-keygen -t ed25519 -f dist_key -N ""
   ```

2. Add the public half (`dist_key.pub`) to the distribution repository:
   `othavi0/omarchy-agent-bar` -> Settings -> Deploy keys -> Add deploy key,
   with "Allow write access" checked.
3. Store the private half (`dist_key`) as an Actions secret on the product
   repository: `othavi0/agent-bar` -> Settings -> Secrets and variables ->
   Actions -> `OMARCHY_AGENT_BAR_DEPLOY_KEY`.
4. Delete the local key files once both halves are stored. Nothing on the
   generating machine needs to keep a copy.

The workflow writes the secret to `~/.ssh/dist_key` at mode `0600` on the
runner, points `GIT_SSH_COMMAND` at it exclusively (`IdentitiesOnly=yes`),
and never logs its contents.

## Append-only rule

The distribution repository's `master` branch is append-only. Never
force-push to it, from the workflow or by hand. Every release adds exactly
one new commit on top of the previous history.

An installed plugin's `omarchy plugin update` pulls the distribution
repository fast-forward only. A force-push that rewrites history breaks
that pull for every existing install: the local clone can no longer
fast-forward, and the update fails. There is no remote-side recovery for an
affected install short of a manual reinstall, so this rule has no
exception.

## Branch protection

`othavi0/omarchy-agent-bar` has branch protection on `master` denying force
pushes, set directly in the repository's GitHub settings, independent of
the workflow. This is a second guard, not a substitute for the append-only
discipline above: the workflow must never attempt a force-push in the
first place.

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
- the release tag.

## Manual boundary

Automatic cuts are always patch bumps. Minor and major releases remain
human-driven: set the version deliberately, then run the same pipeline via
`workflow_dispatch`. Merging to `master` is the release decision for patch
versions; there is no separate per-release authorization step.

Local bundle reproduction for debugging:

```bash
SOURCE_COMMIT="$(git rev-parse HEAD)"
cargo run --bin agent-bar-bundle -- assemble \
  output target/release-candidate/othavi0.agent-bar \
  source-commit "$SOURCE_COMMIT"
```
