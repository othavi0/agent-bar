# Monorepo Migration Design

Date: 2026-08-11
Status: approved design, pending implementation
Supersedes: the two-repository distribution model in ADR 0005 and
`docs/dev/releasing.md` (both amended by this work).

## Goal

Collapse the two-repository setup (`othavi0/agent-bar` source +
`othavi0/omarchy-agent-bar` distribution artifact) into a single public
repository that is simultaneously:

- the development home the community sees (source, issues, PRs, history);
- the installable Omarchy plugin repo (`omarchy plugin add <url>` target);
- the update origin for every existing install, **without breaking their
  fast-forward update path and without requiring reinstalls**.

The final repository lives at `othavi0/omarchy-agent-bar` (the URL existing
installs already point at). `othavi0/agent-bar` becomes a GitHub rename
redirect to it.

## Verified constraints (from the Omarchy plugin manager source)

1. `omarchy-plugin-add` is `git clone <url>` of the default branch;
   `manifest.json` must sit at the clone root. No subdirectory, branch,
   tarball, or build-step support.
2. `omarchy-plugin-validate` requires: `schemaVersion == 1`, required
   manifest fields, entry points as safe relative paths that exist
   (subdirectory paths are allowed), an entry point per kind, and **no
   symlinks anywhere in the tree** (`.git` excluded).
3. `omarchy-plugin-update` is `git fetch origin HEAD` + `merge --ff-only
   FETCH_HEAD` + re-validation with automatic rollback on failure. The
   default branch is therefore append-only forever.
4. The installed helper's `update check` fetches the hardcoded receipt URL
   `https://raw.githubusercontent.com/othavi0/omarchy-agent-bar/master/bundle.json`
   and compares `version` against `CARGO_PKG_VERSION`. This URL must keep
   serving a valid schema-1 receipt at all times.
5. omarchyplugins.com requires a public repo with root `manifest.json`
   (real version string), README, and license.

## Target repository shape (root layout)

The plugin tree lives at the repository root; QML source **is** the shipped
QML — no assembled copy, no duplication:

```
omarchy-agent-bar/
├── manifest.json            # real version, bumped by CI each release
├── bundle.json              # update-check receipt, regenerated each release
├── Service.qml BarWidget.qml Popup.qml ...   # moved from assets/omarchy/
├── components/  icons/
├── bin/agent-bar            # release binary, committed by CI only
├── preview.png              # marketplace preview (from docs/media/demo.png)
├── README.md  LICENSE       # one README serving marketplace and GitHub
├── src/  Cargo.toml  Cargo.lock          # Rust source
├── docs/  tests/  scripts/  schemas/  .github/
└── CHANGELOG.md  CLAUDE.md  AGENTS.md  ...
```

Decisions folded into this shape:

- `assets/omarchy/` is eliminated; its files move to the root and become
  both source and product. `assets/dist/README.md` is eliminated; the root
  README is rewritten to serve end users first (install/update/remove) with
  development content below or linked.
- `manifest.json` at root carries the real released version (no
  `__AGENT_BAR_VERSION__` placeholder in the committed tree); CI bumps it
  in the release commit alongside `Cargo.toml`.
- `.gitignore` keeps ignoring `/target`; `bin/agent-bar` at root is
  committed (release commits only). `docs/media/demo.png` remains the
  editable source of `preview.png`; CI copies it at release.
- No symlinks may ever enter the tree (validate hard-fails). Verified: the
  current source tree has none.

## History graft (the update-compatibility mechanism)

The migration branch merges the dist repository's current `master` into the
source history with `--allow-unrelated-histories`, resolving the tree to
the target shape above. Consequence: the dist tip (`release: v10.3.8`)
becomes an **ancestor** of the new `master`, so every existing install
fast-forwards into the monorepo on its next `omarchy plugin update`. No
reinstall, no manual step; the one-time update diff is large (the source
tree arriving) but valid, and `--yes` hook users never see it.

## Release pipeline (auto-release.yml rework)

Same trigger model (product-path pushes to `master`, `workflow_dispatch`
for manual cuts). The dist-repo push and its deploy key are deleted. New
release step sequence:

1. `agent-bar-cut-release` bumps `Cargo.toml` + lockfile, writes release
   notes/CHANGELOG (unchanged), and now also stamps `manifest.json`.
2. Rust gates run (unchanged).
3. Build `bin/agent-bar` for `x86_64-unknown-linux-gnu`; regenerate
   `bundle.json`. Its `files` inventory keeps the current scope — only the
   files the plugin loads or ships (QML/JS, `components/`, `icons/`,
   `bin/agent-bar`, `manifest.json`, `preview.png`, `README.md`,
   `LICENSE`, `scripts/agent-bar-open-terminal`), never the Rust source,
   docs, or tests. Run the root-tree validation (see Testing).
4. One commit `release: vX.Y.Z` containing the version bumps,
   `bin/agent-bar`, and `bundle.json`; tag `vX.Y.Z`; push `master` + tag
   with the default `GITHUB_TOKEN`; publish the GitHub release.

`src/bin/agent-bar-bundle.rs` and `src/plugin/bundle.rs` are repurposed:
`assemble` no longer materializes a separate output tree; it writes the
release artifacts (binary placement, manifest stamp, `bundle.json`) into
the repo root and validates the result. `RELEASE_NOTES_URL_PREFIX` moves to
`https://github.com/othavi0/omarchy-agent-bar/releases/tag/v` (old
binaries keep working via GitHub's rename redirect).

Append-only rule now applies to this repository's `master`: no rebase or
force-push, ever, enforced by branch protection. This is already the de
facto workflow (PRs merged forward).

## Testing

- `tests/dist_tree_validate.rs` becomes a root-tree validation: mirrors
  `omarchy-plugin-validate` against the repository root and checks the
  shipped-file inventory, modes, and architecture, replacing the assembled
  tree checks.
- QML gates run against the root: `qmllint` over root QML files,
  `qmltestrunner -import <repo root>`, `omarchy plugin validate .`.
- A new test asserts `manifest.json`, `bundle.json`, and
  `Cargo.toml` versions agree (release identity, minus the placeholder
  substitution which no longer exists).
- Existing provider/domain tests are untouched.

## Documentation and contract amendments

- New ADR: single-repository distribution (supersedes ADR 0005's
  two-repo mechanics; the auto-release-on-merge policy itself stands).
- `docs/dev/releasing.md`: rewritten for the single-repo pipeline;
  update-path verification checklist survives (it caught real failures).
- `CLAUDE.md` + `AGENTS.md`: amend the Quattro contract section (install
  URL, no dist repo, root layout, append-only master) and the
  verification commands' paths.
- Root `README.md`: user-facing first, development second.

## Cutover sequence (Phase 2 — separate authorization)

Out of scope for the implementation branch, specified here for the record:

1. Merge the migration branch into `master` (green gates).
2. Rename `othavi0/omarchy-agent-bar` → `othavi0/omarchy-agent-bar-legacy`
   and archive it (kept as escape hatch).
3. Rename `othavi0/agent-bar` → `othavi0/omarchy-agent-bar`. The real name
   overrides the legacy redirect, so the existing install/update URL now
   serves the monorepo; `agent-bar` URLs redirect (issues, stars, PRs
   preserved).
4. Immediately cut the first monorepo release as a **minor** bump
   (v10.4.0) via `workflow_dispatch`.
5. Verify: fresh `omarchy plugin add`, `update check` on an existing
   install reports v10.4.0, `omarchy plugin update` fast-forwards, chips
   render live. Update the omarchyplugins.com listing (same URL; resubmit
   only if the marketplace requires it).

Between steps 3 and 4 the receipt URL serves the grafted (valid) v10.3.8
`bundle.json`; worst case in that window is "no update visible", never an
error.

## Risks accepted

- **Frozen `master`**: ff-only updates forbid history rewrites forever.
- **Binary growth**: ~4 MB per release in history (already true of the
  dist repo; now shared by contributor clones).
- **Skew window**: between a product merge and its release commit, `master`
  tip has new QML/source with the previous binary. Window is minutes when
  the pipeline is green; a red run extends it, which the existing
  post-merge verification checklist already guards.
- **Noisier interactive update diffs**: source changes appear in the
  confirmation diff users see.

## Out of scope

- Any change to provider logic, status schema, settings, or UI behavior.
- omarchyplugins.com listing edits (Phase 2, manual).
- Deleting the legacy repository (it is archived, not deleted).
