# 0006 — Single-repository distribution

- Status: Accepted (2026-08-11)
- Supersedes the two-repository mechanics of ADR 0005 (the
  release-on-every-product-merge policy of 0005 stands).

## Context

`omarchy plugin add`/`omarchy plugin update` consume a plain git repository
whose root is the plugin tree, fast-forward only. Through v10.3.8, Agent Bar
shipped that tree from a separate artifact repository
(`othavi0/omarchy-agent-bar`): the auto-release workflow assembled the
plugin bundle from the source tree at `othavi0/agent-bar` and pushed it as
one commit to the distribution repository on every release (the
2026-08-05 amendment recorded in ADR 0005).

That split had three costs. The community-facing source repository was
invisible to `omarchy plugin add` — installers only ever saw the assembled
tree, with no link back to the code, issues, or history that produced it.
The push authenticated with a dedicated deploy key
(`OMARCHY_AGENT_BAR_DEPLOY_KEY`) scoped to the distribution repository,
which was one more credential to provision, rotate, and keep off every
other workflow. And `docs/dev/releasing.md` carried an entire section on
generating, installing, and rotating that key, in addition to the
append-only rule the distribution repository needed on its own.

## Decision

The repository root IS the plugin tree. There is no separate distribution
repository, no deploy key, and no assemble/push step. Source and shipped
artifacts live in the same git history, in the same commit sequence, under
the same branch protection.

- QML, the manifest, `bin/agent-bar`, `bundle.json`, and `preview.png` all
  live at the repository root, alongside `src/`, `docs/`, and the rest of
  the source tree. `Cargo.toml`/`Cargo.lock` and the plugin tree are
  siblings, not parent and assembled-child.
- CI (`.github/workflows/auto-release.yml`) stamps release artifacts
  in place at every automatic patch cut: `scripts/agent-bar-cut-release`
  bumps `Cargo.toml` and `manifest.json`, the helper is built for
  `x86_64-unknown-linux-gnu`, and
  `agent-bar-bundle stamp source-commit <hex>` copies the built binary to
  `bin/agent-bar`, stamps `preview.png`, and writes `bundle.json` — all
  directly into the working tree.
- Those stamped files, plus the version bump and generated release notes,
  are committed as one `release: v{version}` commit on `master`, tagged
  `v{version}`, and published as the GitHub Release in the same run. A
  guard skips the workflow's own `release:` commit so a cut cannot
  re-trigger itself.
- `bin/agent-bar` update discovery (`DIST_RECEIPT_URL`) and release notes
  (`RELEASE_NOTES_URL_PREFIX`) now point at `othavi0/omarchy-agent-bar` —
  the final name this repository is published under — rather than a
  second repository. `omarchy plugin add`/`update` clone and fast-forward
  that same repository directly.
- The distribution repository's prior history was grafted onto this
  repository as an ancestor of the first single-repository release commit,
  so every existing install's `omarchy plugin update` still finds a
  fast-forward path from whatever commit it last synced.
- `master` is append-only, protected against force-push, forever — the
  same rule ADR 0005's distribution repository needed, now applied to this
  repository's own default branch instead of a second one. There is no
  deploy key: the workflow pushes with its ambient `GITHUB_TOKEN`.
- `tests/dist_tree_validate.rs` is replaced by `tests/root_tree_validate.rs`,
  which validates the repository root itself against the same inventory,
  mode, architecture, and version matrix `omarchy-plugin-validate` checks,
  rather than a separately assembled output directory.

## Consequences

Positive:

- Contributors and installers share one git history; an install's
  `omarchy plugin add` clone is also a full source checkout, with commit
  log, issues, and code visible in the same place a user landed on.
- One credential surface: no deploy key to provision, store as an Actions
  secret, or rotate. The release commit authenticates the same way any
  other workflow push does.
- One less pipeline stage that can fail independently: a release either
  produces one `release:` commit with correct artifacts, or it does not
  run at all. There is no separate "push to dist repo succeeded but
  product tag didn't" failure mode to reason about.
- `docs/dev/releasing.md` drops an entire deploy-key procedure and the
  "confirm the dist repo advanced" verification step collapses into
  "confirm master gained exactly one `release:` commit" against the same
  history contributors already work in.

Costs:

- Contributors cloning or fetching this repository now pull release
  binaries too (`bin/agent-bar` is committed, roughly 4 MB per release,
  replaced whole on every cut) — the working tree is heavier than a
  source-only checkout would be.
- Between a product merge landing on `master` and the release workflow's
  `release:` commit completing, the tip of `master` briefly pairs new
  source with the previous release's stamped binary and `bundle.json`
  (typically minutes). The post-merge update-path verification checklist
  in `docs/dev/releasing.md` exists precisely to catch a run that fails
  silently in that window, the same red-run risk ADR 0005 already called
  out for the prior pipeline.
- `omarchy plugin update`'s interactive diff, when a user inspects it
  before applying, now includes source changes (`src/**`, `docs/**`, and
  so on) alongside the artifacts that actually run, not just the assembled
  plugin files a two-repository split would have shown.

## Canonical detail

The amended bundle and release contract lives in
[docs/specs/v10/08-plugin-bundle-and-release.md](../specs/v10/08-plugin-bundle-and-release.md)
and [docs/dev/releasing.md](../dev/releasing.md).
