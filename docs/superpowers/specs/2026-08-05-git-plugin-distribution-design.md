# Git-native plugin distribution (v11 distribution conversion)

Status: approved design, pending implementation plan.
Decision date: 2026-08-05. Approved by the repository owner in session.

Agent Bar becomes 100% an Omarchy plugin distributed through the native
`omarchy plugin add` / `omarchy plugin update` / `omarchy plugin remove`
flow. The tarball bootstrap (`install.sh`), the GitHub-Releases-asset
distribution, and the in-app self-update machinery are removed.

## Verified constraints (from Omarchy source, read on 2026-08-05)

- `omarchy plugin add <git-url>` clones the URL, runs
  `omarchy-plugin-validate` against the clone root, and moves the whole
  clone to `$HOME/.config/omarchy/plugins/<manifest id>`. The manifest
  must sit at the cloned repo root; every entry point must exist; no
  symlinks outside `.git`; id must not start with `omarchy.`.
- `omarchy plugin update <id>` runs `git fetch` + `git merge --ff-only
  FETCH_HEAD` + re-validation, with `git reset --hard ORIG_HEAD` on
  validation failure. It refuses non-git plugin directories. Bulk
  `omarchy plugin update` silently skips non-git directories.
- Any non-fast-forward event on the distribution branch permanently
  breaks `omarchy plugin update` for every install. The distribution
  history must be append-only forever.
- `omarchy plugin remove <id>` disables the plugin (removing its
  `bar.layout` entry via the shell IPC), deletes a git checkout with
  `rm -rf`, and rescans. It has no purge concept for XDG state.
- No step of the native flow builds anything. The compiled helper must
  be committed inside the distribution tree.
- Validation rejects neither executables nor large files; git preserves
  the 0755 mode bit. `bundle.json` and other extra files are allowed.
- Marketplace (HANCORE-linux/omarchy-plugin-marketplace) requires: one
  plugin per public repo, `manifest.json` at the repo root, a root
  README with installation and removal instructions, a root license
  file, optional root `preview.png`. Submission is a GitHub issue
  titled `[Plugin]: ...` with category and 1-3 tags; AI agents must get
  explicit owner approval of the exact issue body before creating it.

## Goals

1. Install: `omarchy plugin add https://github.com/othavi0/omarchy-agent-bar.git`.
2. Update: `omarchy plugin update agent-bar.usage` (or the Settings
   button, which delegates to it).
3. Remove: `omarchy plugin remove agent-bar.usage` (or the Settings
   button, which delegates to it after the optional purge).
4. Marketplace listing of the distribution repo.
5. Delete the tarball pipeline: `install.sh`, release archive assets,
   download/verify/swap code, transaction worker, `tar`/`zstd` deps.

## Non-goals

- No change to provider collection, status schema v2, cache, settings,
  notifications, or popup/bar UI outside the Maintenance surface.
- No multi-architecture builds (stays x86_64-unknown-linux-gnu).
- No version pinning for users (git flow always fast-forwards to the
  latest published state; `AGENT_BAR_VERSION` pinning dies with
  `install.sh`).

## Architecture

### Distribution repo: `othavi0/omarchy-agent-bar`

Root layout = exactly the tree `agent-bar-bundle assemble` produces
today, plus three distribution files:

```
manifest.json            # version stamped, + barWidget.defaultSection "right"
Service.qml  BarWidget.qml  Popup.qml  ...  components/  icons/
Core*.js
bin/agent-bar            # compiled helper, 0755, committed
scripts/agent-bar-open-terminal
bundle.json              # build receipt; also the update-check discovery document
README.md                # short user README: what it is, install, update, remove
LICENSE                  # MIT, copied from the product repo
preview.png              # marketplace listing image (docs/media/demo.png)
```

`README.md`, `LICENSE`, and `preview.png` become inputs of the
`assemble` step so the `bundle.json` receipt stays a complete
inventory. Tree validation (helper-side `validate_tree`, `doctor`)
must ignore a `.git` directory at the tree root, because the installed
plugin dir is now a git checkout.

### CI: one writer, append-only

`auto-release.yml` keeps steps 1-9 (cut release, gates, commit+tag the
bump to the product repo, build helper, assemble, sanity assertions)
and replaces the archive/asset steps with:

1. Clone `othavi0/omarchy-agent-bar` (write access via a dedicated
   SSH deploy key stored as an Actions secret; `GITHUB_TOKEN` cannot
   push cross-repo).
2. Remove all tracked content except `.git`; copy the assembled tree in.
3. Commit `release: vX.Y.Z (agent-bar@<source-commit>)`; push. Never
   force. The existing `concurrency: group: auto-release,
   cancel-in-progress: false` remains the single-writer guarantee.
4. `gh release create vX.Y.Z` on the product repo keeps publishing the
   tag and release notes (the UI's release-notes link target) but
   uploads no archive assets.

The product repo remains the only place development happens; the
distribution repo receives only CI commits, one per release.

### Helper (Rust) changes

Removed outright:

- GitHub Releases discovery and download surface: `RELEASES_API_URL`,
  `RELEASE_DOWNLOAD_PREFIX`, `GitHubRelease`/`GitHubAsset`,
  `download_with_policy`, redirect/asset URL validators,
  `ReleaseHttp`/`ReqwestReleaseHttp`/`ScriptedReleaseHttp`.
- The staged-transaction self-update: `MaintenanceWorker` handoff,
  worker self-copy, systemd journal transactions, `renameat2`
  exchange/quarantine of the plugin directory, `stage_update_bundle`,
  archive extract/inspect (`tar`/`zstd` leave `Cargo.toml`).
- `agent-bar-bundle release` subcommand and `ReleaseBuilder`
  (`write_tar_zst`, `ReleaseMetadata`, `.sha256` sidecar).
- `setup plugins-dir` tree-copy install and
  `resolve_plugin_source_root`.
- `install.sh`.

Rewritten:

- `update check`: discovery becomes one HTTPS GET of
  `https://raw.githubusercontent.com/othavi0/omarchy-agent-bar/master/bundle.json`,
  which already carries version, Omarchy contract, and minimum
  Quickshell version. The stdout contract keeps `schemaVersion`,
  `checkedAt`, `current`, `available`, `latestCompatible{version,
  omarchyContract, minimumQuickshellVersion, releaseNotesUrl}`;
  `archiveUrl`/`checksumUrl`/`archiveSha256`/`sourceCommit` are
  removed (no consumer remains). `releaseNotesUrl` is computed as
  `https://github.com/othavi0/agent-bar/releases/tag/v<version>`.
  New typed state: when the local plugin root has no `.git`, the check
  reports `reinstallRequired: true` so the UI can render the one-time
  migration instruction instead of a false "up to date".
- `update apply`: no version argument. Preflight (omarchy CLI present,
  shell ping), then detach `systemd-run --user -- omarchy plugin
  update agent-bar.usage --yes`. Detachment is required because the
  update rescan can tear down the QML service that pressed the button.
- `uninstall [purge]`: keeps the typed stdin confirmation document.
  Step 1 (only with purge): remove Agent Bar's own XDG state
  (settings, cache, backups, notification state) — nothing under the
  plugin directory. Step 2: detach `systemd-run --user -- omarchy
  plugin remove agent-bar.usage --yes`, which owns disable +
  directory removal + rescan. The helper never touches `shell.json`
  anymore.
- `setup`: survives as settings migration only (v9 -> v10
  `migrate_live_paths`); no tree copy, no enable/rescan calls.
- `doctor`: unchanged in purpose; its tree validation learns to ignore
  `.git`.

### QML (Settings / Maintenance) changes

The Maintenance view keeps its shape: Check for updates, Update to X,
Release notes, Uninstall with two-click arm and purge toggle. Changes:

- `CoreMaintenance.js` argv builders: `updateApplyArgv` loses the
  version argument; `uninstallArgv` unchanged in shape (helper still
  receives `uninstall [purge]` + stdin confirmation).
- `maintenanceUiFromCheck` learns the `reinstallRequired` state and
  renders the migration instruction (`omarchy plugin remove
  agent-bar.usage` + `omarchy plugin add <dist-url>`).
- Copy changes: the update confirm message describes the git
  fast-forward semantics ("Fast-forwards to the latest release.
  A failed validation rolls back automatically.") instead of the
  tarball swap wording.
- The stdin plumbing for the uninstall confirmation stays (the helper
  still consumes it); the update path never used stdin.

### Manifest

`assets/omarchy/manifest.json` gains
`"barWidget": { "defaultSection": "right" }` so `omarchy plugin add
--enable` (and the interactive placement prompt) defaults to the right
bar section, where status chips live.

## Migration of existing installs

Pre-conversion installs are tarball trees without `.git`; the native
updater refuses them (targeted) or silently skips them (bulk). Path:

1. `update check` detects the missing `.git` and the Settings UI shows
   the reinstall instruction (see above). The README and the release
   notes for the first converted release carry the same two commands.
2. `omarchy plugin remove agent-bar.usage` backs up a non-git tree
   rather than deleting it (verified Omarchy behavior), so the step is
   safe; settings live outside the plugin dir and survive.

## Documentation and contract amendments

The v10 spec change-control gate (explicit owner approval) is
satisfied by this approved design. Amendments land with the
implementation:

- `docs/specs/v10/01-product-contract.md`: PROD-007 (update/uninstall
  remain UI journeys, now delegating to the Omarchy CLI), PROD-010
  ("bundle" = the distribution repo tree), Maintain journey.
- `03-cli-and-json-contract.md`: grammar (`update apply` loses the
  version; `setup plugins-dir` removed) and CLI-024..031 maintenance
  contract.
- `06-migration-and-legacy-removal.md`: MIG-020..026 replaced by the
  delegation model + reinstall detection.
- `08-plugin-bundle-and-release.md`: release-files/update-transaction
  sections replaced by the distribution-repo model; BUNDLE-013
  (install.sh) deleted.
- `docs/specs/v10/README.md`: superseding note pointing here.
- Project `CLAUDE.md`: fix L65 (existing installs update via `omarchy
  plugin update`, not `rescan`), product boundaries, verification
  pointer for the retired bundle matrix.
- Guides: README install section, `docs/guide/commands.md`,
  `runtime.md` (bundle/backups/transactions), `troubleshooting.md`
  (update-failed narrative), `docs/dev/architecture.md` (plugin
  maintenance), `docs/dev/releasing.md`, `docs/guide/integration.md`,
  PRODUCT.md, CONTRIBUTING.md release section, CHANGELOG entry.

## Test plan (enforcement surface, from the audit)

- `tests/active_legacy_scan.rs`: add `install.sh` to
  `LOCKED_DELETION_PATHS`; drop the install.sh-content assertion; ban
  new legacy tokens (`tar.zst`, releases-download URL, worker unit
  name); drop `tar`/`zstd` from the dependency-owner map.
- `tests/cli.rs`: rewrite `update`/`uninstall`/`setup` grammar cases
  (no `update apply <semver>`, no `setup plugins-dir`); keep the
  migration test retargeted at `setup`.
- `tests/update_check_parity.rs` + fixtures: new document shape
  including the `reinstallRequired` fixture.
- `tests/agent_bar_bundle_cli.rs`: `assemble`-only grammar.
- QML: `tst_Maintenance.qml`, `tst_Service.qml`, `tst_ServiceRaces.qml`
  re-pinned to the delegation argvs and the new copy;
  screenshot inventory updated if dialog copy changes.
- New: a test that the assembled tree passes a faithful local mirror of
  `omarchy-plugin-validate` rules (root manifest, entry points exist,
  no symlinks) so CI cannot push an uninstallable state.
- `tests/active_docs.rs` remains the acceptance gate for every doc
  example touching the CLI.

## Rollout order

1. Rust strip + rewrite (helper), with tests.
2. QML maintenance rework, with tests.
3. Assemble gains README/LICENSE/preview inputs; new validate-mirror test.
4. CI: dist-push step + deploy key; delete archive steps; `install.sh`
   removal; docs/spec amendments (same release train).
5. First converted release pushes the initial state of
   `othavi0/omarchy-agent-bar`.
6. Live QA on this machine: migrate the local install through the
   documented path; verify add/update/remove end to end.
7. Marketplace submission (category `Widgets`, tags `ai`, `bar`,
   `quickshell`), issue body approved by the owner before creation.

## Risks and mitigations

- Force-push on the dist repo bricks every install's updates: CI is
  the only writer, serialized by the existing concurrency group; the
  repo carries a branch-protection rule against force pushes.
- Unbounded dist-repo growth (one committed ELF per release): accepted;
  the helper is ~4.5 MB stripped and release cadence is low. Never
  rewrite history to "clean" it — that is the one forbidden fix.
- Stale window between product push and dist push: does not exist for
  users; installs only ever see dist-repo states, which are complete
  release states by construction.
- Purge/remove ordering: purge runs before the detached remove; the
  purge never touches the plugin dir, the remove never touches XDG
  state — disjoint by design.
- Deploy-key rotation: documented in `docs/dev/releasing.md`; a failed
  dist push fails the workflow loudly (release is not published
  half-way: the `gh release create` step moves after the dist push).
