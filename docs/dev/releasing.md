# Releasing Agent Bar

Releases are automatic. Every push to `master` that touches a product path
(`src/**`, `scripts/**`, `Cargo.toml`, `Cargo.lock`, `*.qml`, `Core*.js`,
`components/**`, `icons/**`, `manifest.json`) triggers
`.github/workflows/auto-release.yml`, which cuts a patch release, stamps
the release artifacts into the repository root, and publishes the product
release in a single run. Docs-only merges cut nothing. See
[ADR 0006](../adr/0006-single-repository-distribution.md), which
supersedes the two-repository mechanics of
[ADR 0005](../adr/0005-auto-release-on-product-merge.md) (the
release-on-every-product-merge policy of 0005 stands).

The repository root IS the Omarchy plugin tree. There is no distribution
repository: `bin/agent-bar`, `bundle.json`, and the versioned
`manifest.json` are stamped straight into this repository's `master` by
CI, in the same commit as everything else that shipped. There is no
standalone binary tarball, AUR package, cargo-binstall metadata, or global
installation.

## Automatic pipeline

1. `scripts/agent-bar-cut-release` bumps the patch version in `Cargo.toml`,
   the lockfile, and `manifest.json`, writes `docs/releases/{version}.md`
   from the Conventional Commit subjects since the last tag, and prepends
   a matching CHANGELOG section below `[Unreleased]`. Preview locally:

   ```bash
   scripts/agent-bar-cut-release --dry-run
   ```

2. Rust gates run against the bumped tree: `cargo fmt --check`,
   `cargo test`, `cargo clippy --all-targets -- -D warnings`. A red gate
   stops the run before anything is committed.
3. The helper builds for `x86_64-unknown-linux-gnu`.
4. `agent-bar-bundle stamp source-commit <hex>` stamps the release
   artifacts directly into the repository root: it copies the built
   helper to `bin/agent-bar` (mode `0755`), refreshes `preview.png`,
   normalizes the shipped tree's file modes, and writes `bundle.json`
   from the current, already-versioned root tree. `scripts/check-version`
   then confirms `Cargo.toml`, `manifest.json`, `bundle.json`, and the
   stamped helper's own `version` output all agree.
5. A root inventory step checks required files, executable modes, and the
   target architecture directly against the working tree (no separate
   assembled output directory to check).
6. Everything the bump and stamp touched —
   `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, `docs/releases/{version}.md`,
   `manifest.json`, `bundle.json`, `preview.png`, `bin/agent-bar` — is
   committed as one `release: v{version}` commit, tagged `v{version}`, and
   pushed straight to `master`. The GitHub Release is created from that
   same tag with generated notes and no attached files; the commit itself
   is the release.

Guards:

- The workflow skips its own `release:` commit, so a cut cannot
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
`cargo test --test root_tree_validate` mirrors `omarchy-plugin-validate`
against the repository root, and branch protection keeps `master`
append-only and fast-forwardable. Nothing extra is manual at that stage.

After merging:

1. **Watch the `Auto release` run to completion.** The release exists only
   when the run is green:

   ```bash
   gh run list --workflow "Auto release" --limit 1
   ```

   The run takes a few minutes after the merge. Checking an install before
   it finishes reports "up to date" — that is timing, not a defect.

2. **Confirm `master` gained exactly one `release: v{version}` commit** on
   top of the merge (never a rewrite):

   ```bash
   git fetch origin && git log --oneline -3 origin/master
   ```

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

## Append-only rule

This repository's `master` branch is append-only. Never force-push to it,
from the workflow or by hand. Every release adds exactly one new commit on
top of the previous history.

An installed plugin's `omarchy plugin update` pulls this repository
fast-forward only. A force-push that rewrites `master` breaks that pull
for every existing install: the local clone can no longer fast-forward,
and the update fails. There is no remote-side recovery for an affected
install short of a manual reinstall, so this rule has no exception. The
distribution repository this repository replaced (ADR 0006) needed the
exact same rule; it now applies to this repository's own default branch
instead of a second one.

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
- `manifest.json` version;
- `bundle.json` version;
- private helper `version` output;
- the release tag.

`scripts/check-version` confirms all of these against the repository root
in one call; the release workflow runs it as part of the stamp step.

## Manual boundary

Automatic cuts are always patch bumps. Minor and major releases remain
human-driven: set the version deliberately, then run the same pipeline via
`workflow_dispatch`. Merging to `master` is the release decision for patch
versions; there is no separate per-release authorization step.

## Local reproduction

`agent-bar-bundle stamp` mutates the repository root in place — it is the
same step CI runs, so running it locally is the fastest way to reproduce a
release-artifact bug. It stamps whatever version is already in
`Cargo.toml`; it does not bump anything.

```bash
cargo build --release
cargo run --bin agent-bar-bundle -- stamp source-commit "$(git rev-parse HEAD)"
```

This overwrites three tracked files in place — `bin/agent-bar`,
`preview.png`, `bundle.json` — and normalizes the file mode of every
shipped file under the root inventory (`0755` for `bin/agent-bar` and
`scripts/agent-bar-open-terminal`, `0644` for the rest). On a tree that
already matches the last release, that mode normalization is a no-op;
confirm with `git status --porcelain` and discard the stamp's output with:

```bash
git status --porcelain           # confirm nothing else changed
git checkout -- bin/agent-bar bundle.json preview.png
```

If `git status --porcelain` shows anything beyond those three paths, the
tree had a stray mode difference before the stamp ran — check it out
individually too rather than reaching for a blanket `git checkout -- .`,
which would also discard unrelated in-progress work.
