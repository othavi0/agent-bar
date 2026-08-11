# Plugin Bundle and Release

Amended by git-plugin-distribution (2026-08-05):
`docs/superpowers/specs/2026-08-05-git-plugin-distribution-design.md`. The
manifest and receipt shapes below carry forward; the release-files,
update-transaction, and uninstall-transaction sections are replaced by the
distribution-repository model.

Amended by the plugin-ID rename (2026-08-06):
`docs/superpowers/specs/2026-08-06-plugin-id-rename-design.md`. The plugin ID
is `othavi0.agent-bar`; it read `agent-bar.usage` when this document was
approved.

## Product artifact

v10 builds one architecture-specific Omarchy plugin bundle. Since
git-plugin-distribution, the same assembled tree is also the exact
distribution-repository commit that `omarchy plugin add` clones, so it
carries its own `README.md`, `LICENSE`, and marketplace `preview.png`:

```text
othavi0.agent-bar/
├── manifest.json
├── bundle.json
├── README.md
├── LICENSE
├── preview.png
├── Service.qml
├── BarWidget.qml
├── Popup.qml
├── ProviderRail.qml
├── ProviderView.qml
├── SettingsView.qml
├── MaintenanceView.qml
├── components/
├── icons/
├── scripts/
│   └── agent-bar-open-terminal
└── bin/
    └── agent-bar
```

An installed plugin directory additionally contains a `.git/` at its root:
it is a git checkout of the distribution repository. Bundle tree validation
(`BundleValidator::validate_tree`, `doctor`) tolerates a real `.git`
directory sitting directly at the tree root and does not walk it; a `.git`
anywhere deeper, or one that is itself a symlink, is not special-cased and
still fails validation through the ordinary symlink/extra-file checks.

- `BUNDLE-001`: The product is the `othavi0.agent-bar` plugin directory.
- `BUNDLE-002`: `bin/agent-bar` is private and invoked by resolved absolute
  plugin path.
- `BUNDLE-003`: No global executable, package, application entry, ManagedGit
  checkout, or second asset installation is created.
- `BUNDLE-004`: QML and icons remain visible in the bundle for review.
- `BUNDLE-005`: The terminal helper remains Bash.
- `BUNDLE-006`: Manifest version and helper version must match exactly.
- `BUNDLE-007`: The initial official target is
  `x86_64-unknown-linux-gnu`.
- `BUNDLE-007A`: The installed plugin root is literal
  `$HOME/.config/omarchy/plugins/othavi0.agent-bar`; Quattro does not apply
  `XDG_CONFIG_HOME` to plugin discovery.

## Manifest

The final Quattro-validated manifest is:

```json
{
  "schemaVersion": 1,
  "id": "othavi0.agent-bar",
  "name": "Agent Bar",
  "version": "10.0.0",
  "author": "othavi0",
  "license": "MIT",
  "description": "LLM quota monitor for Claude, Codex, Amp, and Grok.",
  "kinds": ["service", "bar-widget"],
  "entryPoints": {
    "service": "Service.qml",
    "barWidget": "BarWidget.qml"
  },
  "barWidget": {
    "displayName": "Agent Bar",
    "description": "Shows normalized provider quota and reset information.",
    "category": "AI",
    "defaultSection": "right",
    "aliases": ["agent-bar"],
    "allowMultiple": false,
    "defaults": {},
    "schema": []
  }
}
```

It must:

- use schema version 1;
- retain ID `othavi0.agent-bar`;
- declare `service` and `bar-widget`;
- map the service to `Service.qml`;
- map the bar widget to `BarWidget.qml`;
- set `allowMultiple` to exactly `false`; Quattro replicates the single widget
  definition per monitor through its normal host mechanism;
- set `barWidget.defaultSection` to `right`, so `omarchy plugin add`'s
  interactive placement prompt (and its `--enable` flag) defaults there;
- contain no ignored v9 activation key;
- contain only supported schema keys;
- expose no inline Agent Bar settings schema.

`bundle.json` is the Agent Bar ownership and integrity receipt, and, since
git-plugin-distribution, the sole document `update check` reads to
discover the latest release. It records bundle schema, plugin ID, version,
Rust target, source commit, and the SHA-256, size, and mode of every other
bundle file, including `README.md`, `LICENSE`, and `preview.png`. Every
release embeds a freshly computed `sourceCommit`, so the distribution
repository never receives an empty commit. There is no separate archive or
checksum sidecar to cover `bundle.json` itself; the receipt does not attempt
a recursive self-digest.

The exact receipt shape is:

```json
{
  "schemaVersion": 1,
  "pluginId": "othavi0.agent-bar",
  "version": "10.0.0",
  "target": "x86_64-unknown-linux-gnu",
  "omarchyContract": 1,
  "minimumQuickshellVersion": "0.3.0",
  "sourceCommit": "0123456789abcdef0123456789abcdef01234567",
  "files": [
    {
      "path": "BarWidget.qml",
      "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
      "size": 1234,
      "mode": "0644"
    }
  ]
}
```

`files` contains every regular bundle file except `bundle.json`, sorted by raw
UTF-8 path bytes. Paths use `/`, are relative to the plugin root, and contain
no empty, `.` or `..` component. `sha256` is 64 lowercase hexadecimal
characters, `size` is the exact byte length, and `mode` is the four-digit
octal permission string after masking file-type bits. The receipt rejects
unknown fields, duplicate paths, directories, links, devices, sockets, and
files not present in both the receipt and staged bundle.

The exact manifest shape is copied from the locally installed Quattro registry
contract. Rust/JSON contract tests run in CI; `omarchy plugin validate`,
Quickshell imports, and QML behavior run in the isolated Quattro acceptance
environment because generic GitHub-hosted runners do not provide that runtime.

## Distribution repository

Replaces the archive-based "Release files" model. There is no `.tar.zst`
archive, checksum sidecar, or release-metadata JSON; the distribution
artifact is a plain git tree.

`othavi0/omarchy-agent-bar` holds exactly one root layout: the assembled
tree above (`manifest.json`, `bundle.json`, QML/icons/scripts/`bin/`,
`README.md`, `LICENSE`, `preview.png`) plus its own `.git/`. CI
(`.github/workflows/auto-release.yml`) is the sole writer: it clones the
repository, replaces every tracked path except `.git`, commits
`release: vX.Y.Z (agent-bar@<source-commit>)`, and pushes to `master` with a
dedicated SSH deploy key scoped to that repository only. The repository's
`master` branch is append-only: the workflow never force-pushes, and branch
protection denies it independently, because `omarchy plugin update` is a
fast-forward-only pull for every existing install. A rewritten history
breaks the update check for all of them with no remote-side recovery.

- `BUNDLE-008`: The pushed tree contains exactly one `othavi0.agent-bar`
  worth of content at the distribution repository root (plus that
  repository's own `.git/`).
- `BUNDLE-009`: It contains no Rust source, tests, target directory,
  credentials, local config, or development fixtures from the product
  repository.
- `BUNDLE-010`: File modes are deterministic; the Rust and terminal helpers
  are executable and other files are not. Git preserves the executable bit
  across the clone/fast-forward.
- `BUNDLE-011`: Tree paths are relative, normalized, and free of symlinks,
  devices, and traversal, both at assemble time and as pushed.
- `BUNDLE-012`: Reproducible assembly produces the same inventory and content
  hashes from the same source commit.
- `BUNDLE-012A`: Icons retain their approved source formats:
  `claude.png`, `codex.png`, `amp.svg`, and `grok.svg`.
- `BUNDLE-012B`: **Retired.** There is no separate release-metadata document;
  `bundle.json` (above) is the only discovery document, fetched directly
  from the distribution repository over HTTPS.

The tracked English release-notes source is
`docs/releases/10.0.0.md`. The internal builder command is exactly:

```text
agent-bar-bundle assemble output <plugin-dir>
  source-commit <40-lowercase-hex>
```

`assemble` creates and validates a bundle using the explicit source commit;
it does not claim the worktree matches that value. It requires
`assets/dist/README.md`, the repository-root `LICENSE`, and
`docs/media/demo.png` to exist, and copies them into the tree as
`README.md`, `LICENSE`, and `preview.png`. The `agent-bar-bundle release
bundle` subcommand (archive/checksum/metadata builder) is removed; the
builder is an internal development binary, not installed in the plugin.

## Installation

Replaces the `install.sh` bootstrap model entirely. Agent Bar ships no
installer; installation is the native Omarchy plugin flow end to end.

- `BUNDLE-013`: **Retired.** `install.sh` is deleted. Installation is
  `omarchy plugin add <dist-repo-url>`: it clones the URL, validates the
  clone with `omarchy-plugin-validate`, and moves it to
  `$HOME/.config/omarchy/plugins/othavi0.agent-bar`. No Agent Bar-authored
  bootstrap, checksum verification, or staging step runs.
- `BUNDLE-014`: **Retired.** `omarchy plugin add` owns its own install
  bookkeeping; Agent Bar's transaction state records nothing about
  installation.
- `BUNDLE-015`: A pre-conversion (non-git) install is migrated by removing
  and re-adding rather than an in-place transaction; see `MIG-026`.
- `BUNDLE-016`: `omarchy plugin add`'s interactive placement prompt, or its
  `--enable` flag, places the bar widget; `manifest.json`'s
  `barWidget.defaultSection: "right"` (above) is the default when the
  prompt is skipped.
- `BUNDLE-017`: **Retired**, folded into `BUNDLE-016`: placement is entirely
  the Omarchy CLI's enable flow.
- `BUNDLE-018`: **Retired.** `omarchy plugin add` performs its own
  validate/move/rescan; Agent Bar issues no separate rescan call for
  install.
- `BUNDLE-019`: **Retired.** A failed `omarchy plugin add` leaves no partial
  Agent Bar install to restore; failure is the Omarchy CLI's own, entirely
  before any move into the plugins directory.
- `BUNDLE-019A`: **Retired**, superseded by `MIG-019A`.

## Update check

The private command surface includes:

```text
agent-bar update
agent-bar update check
agent-bar update apply
```

- `BUNDLE-020`: **Retired.** Bare `update` has no interactive flow: since
  `update apply` applies unconditionally (below), there is no specific
  fetched version left for a TTY prompt to confirm. Bare `update` prints
  usage pointing at `update check`/`update apply` and exits `3`.
- `BUNDLE-021`: `update check` returns a machine-readable document
  containing current version, latest compatible version, availability,
  release-notes URL, target, and `reinstallRequired`. It carries no
  archive/checksum/source-commit fields.
- `BUNDLE-022`: `update apply` takes no version argument; it delegates
  unconditionally to `omarchy plugin update othavi0.agent-bar --yes`.
- `BUNDLE-023`: The Settings UI performs check and confirmation as separate
  states before triggering apply.
- `BUNDLE-024`: `update check` reads only the distribution repository's own
  `bundle.json`, fetched directly from
  `https://raw.githubusercontent.com/othavi0/omarchy-agent-bar/master/bundle.json`.
- `BUNDLE-025`: `update apply` never downloads, extracts, or executes
  anything itself; the git fast-forward and validation are entirely the
  Omarchy CLI's.

The exact successful `update check` response is:

```json
{
  "schemaVersion": 1,
  "checkedAt": "2026-07-26T18:42:00Z",
  "current": {
    "version": "10.0.0",
    "target": "x86_64-unknown-linux-gnu",
    "omarchyContract": 1,
    "quickshellVersion": "0.3.0"
  },
  "available": true,
  "reinstallRequired": false,
  "latestCompatible": {
    "version": "10.1.0",
    "omarchyContract": 1,
    "minimumQuickshellVersion": "0.3.0",
    "releaseNotesUrl": "https://github.com/othavi0/agent-bar/releases/tag/v10.1.0"
  }
}
```

`latestCompatible` is `null` when the receipt's target/contract are
incompatible with the local install, or when `reinstallRequired` is `true`.
Otherwise it describes the receipt's version, including the current version
when no newer version exists. `available` is true exactly when that version
is strictly newer than `current.version`, and is always `false` when
`reinstallRequired` is `true`. A `reinstallRequired` document must have
`available: false` and `latestCompatible: null`; it may never also claim an
offer. `update check` writes only this JSON document plus newline to
stdout; diagnostics go to stderr.

`reinstallRequired` is `true` whenever the live plugin root has no `.git`
directory: `omarchy plugin update` can only fast-forward a git-managed
install, so a pre-conversion (tarball-installed) tree must be reinstalled
through `omarchy plugin add` instead of updated in place. The receipt is
still fetched and validated for its own sake in that case, so a malformed
or unreachable distribution repository is still a command error.

`update apply` performs no version check of its own. The omarchy CLI it
delegates to always fast-forwards to whatever the distribution repository's
`master` currently is.

Omarchy contract `1` means all of these are required:

- Quattro manifest service and bar-widget entry points;
- `manifest.__sourceDir` service injection;
- `bar.shell.serviceFor(moduleName)`;
- `KeyboardPanel`, `PanelKeyCatcher`, and `BarWidget`;
- `IpcHandler` reached through `omarchy-shell`;
- `omarchy plugin add`, `plugin update`, `plugin remove`, and
  `plugin validate`;
- `shell ping` and structured `shell listPlugins`.

Setup preflight (for the settings-migration path only) requires regular
readable Quattro QML components and executable Omarchy commands.
`update apply` and `uninstall` preflight requires resolvable absolute paths
for `omarchy` and `systemd-run` before consuming any confirmation or
purging any state.

## Update transaction

Retired as a block. `BUNDLE-026`–`BUNDLE-032K` described the staged-download
worker chain: temporary-path download, archive/checksum/inventory
validation, `renameat2(RENAME_EXCHANGE)` swap, a self-copied
`agent-bar-maintenance-worker` running in a transient systemd unit, health
IPC polling, and post-commit garbage collection. None of that exists after
git-plugin-distribution: there is no archive to download or verify, no
directory exchange, and no copied worker binary.

What replaces it:

- `BUNDLE-026`: `update apply` builds no download/stage plan; the git fetch
  and fast-forward are entirely `omarchy plugin update`'s.
- `BUNDLE-027`: Validation (manifest ID, schema, entry points, no symlinks,
  `barWidget.defaultSection`) runs as `omarchy-plugin-validate` inside
  `omarchy plugin update`, after the fast-forward and before it is kept.
- `BUNDLE-028`–`BUNDLE-029`: **Retired.** There is no separate
  downgrade/modified-directory policy in Agent Bar: `omarchy plugin update`
  always fast-forwards to the distribution repository's current `master`,
  and refuses a plugin directory with local modifications outright (a
  non-fast-forward `git merge`) rather than negotiating around them.
- `BUNDLE-030`: `omarchy plugin update` performs fetch, fast-forward,
  re-validate, and (only on a failed validation) rollback as one operation
  from the caller's perspective; Agent Bar's own part is limited to holding
  the maintenance lock across the handoff and reporting it.
- `BUNDLE-031`: A failed validation restores the previous complete bundle
  via `git reset --hard ORIG_HEAD`, run by `omarchy plugin update` itself.
- `BUNDLE-032`: **Retired.** There is no directory exchange; the update is a
  git working-tree fast-forward in place.
- `BUNDLE-032A`–`BUNDLE-032K`: **Retired.** The copied-worker, transient-unit
  argv0 dispatch, health-IPC polling, `listPlugins` absence verification,
  monotonic deadline budget, and post-commit garbage collection they
  described are gone with the worker chain. What survives of the
  "detached transient unit" idea is simpler: `update apply` and `uninstall`
  each start one `systemd-run --user --collect
  --unit=agent-bar-<update|remove>-<32-lowercase-hex-txid>.service -- <omarchy>
  plugin <update|remove> othavi0.agent-bar --yes` and return once systemd has
  accepted it, so the operation survives the initiating QML service being
  torn down by the rescan it triggers. `MIG-020`–`MIG-026` are the current
  contract.

## UI uninstall

Retired as a block. `BUNDLE-033`–`BUNDLE-038C` described uninstall's own
quarantine/rescan/health/garbage-collection transaction, matching the
update worker chain above. `MIG-020`–`MIG-026` are the current contract:
`uninstall` purges only Agent Bar's own XDG state (with `purge`), then
delegates unconditionally to `omarchy plugin remove othavi0.agent-bar --yes`,
which owns disabling the bar entry, deleting (or, for a non-git directory,
backing up) the plugin directory, and rescanning. The structured stdin
confirmation document (`BUNDLE-036`'s schema) is unchanged and lives in
`03-cli-and-json-contract.md`.

## Release boundary

- `BUNDLE-039`: The implementation prepares version `10.0.0`, changelog,
  migration guide, release-notes draft, archive, and checksum.
- `BUNDLE-040`: Grok may commit, push the feature branch, and open a ready PR.
- `BUNDLE-041`: Grok may not merge, tag, publish a GitHub Release, distribute
  the archive, or change the live desktop before the authorized QA gate.
- `BUNDLE-042`: Publishing requires final Codex review, passing live QA, user
  merge, and separate explicit release authorization.
