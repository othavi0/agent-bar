# Plugin Bundle and Release

## Product artifact

v10 builds one architecture-specific Omarchy plugin bundle:

```text
agent-bar.usage/
├── manifest.json
├── bundle.json
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

- `BUNDLE-001`: The product is the `agent-bar.usage` plugin directory.
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
  `$HOME/.config/omarchy/plugins/agent-bar.usage`; Quattro does not apply
  `XDG_CONFIG_HOME` to plugin discovery.

## Manifest

The final Quattro-validated manifest is:

```json
{
  "schemaVersion": 1,
  "id": "agent-bar.usage",
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
    "aliases": ["agent-bar"],
    "allowMultiple": false,
    "defaults": {},
    "schema": []
  }
}
```

It must:

- use schema version 1;
- retain ID `agent-bar.usage`;
- declare `service` and `bar-widget`;
- map the service to `Service.qml`;
- map the bar widget to `BarWidget.qml`;
- set `allowMultiple` to exactly `false`; Quattro replicates the single widget
  definition per monitor through its normal host mechanism;
- contain no ignored v9 activation key;
- contain only supported schema keys;
- expose no inline Agent Bar settings schema.

`bundle.json` is the Agent Bar ownership and integrity receipt. It records
bundle schema, plugin ID, version, Rust target, source commit, and the SHA-256,
size, and mode of every other bundle file. The release archive checksum covers
`bundle.json`; the receipt does not attempt a recursive self-digest.

The exact receipt shape is:

```json
{
  "schemaVersion": 1,
  "pluginId": "agent-bar.usage",
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

## Release files

```text
agent-bar.usage-10.0.0-x86_64-unknown-linux-gnu.tar.zst
agent-bar.usage-10.0.0-x86_64-unknown-linux-gnu.tar.zst.sha256
agent-bar.usage-10.0.0-x86_64-unknown-linux-gnu.metadata.json
LICENSE
```

- `BUNDLE-008`: The archive contains one top-level `agent-bar.usage` directory.
- `BUNDLE-009`: It contains no Rust source, tests, target directory, Git
  metadata, credentials, local config, or development fixtures.
- `BUNDLE-010`: File modes are deterministic; the Rust and terminal helpers
  are executable and other files are not.
- `BUNDLE-011`: Archive paths are relative, normalized, and free of symlinks,
  devices, and traversal.
- `BUNDLE-012`: Reproducible assembly produces the same inventory and content
  hashes from the same source commit.
- `BUNDLE-012A`: Icons retain their approved source formats:
  `claude.png`, `codex.png`, `amp.svg`, and `grok.svg`.
- `BUNDLE-012B`: Release metadata is a closed schema-v1 document containing
  plugin ID, version, target, Omarchy contract, minimum Quickshell version,
  source commit, archive filename/size/SHA-256, and release-notes URL. Values
  equal the receipt, archive, checksum sidecar, and GitHub release tag.

Exact metadata shape:

```json
{
  "schemaVersion": 1,
  "pluginId": "agent-bar.usage",
  "version": "10.0.0",
  "target": "x86_64-unknown-linux-gnu",
  "omarchyContract": 1,
  "minimumQuickshellVersion": "0.3.0",
  "sourceCommit": "0123456789abcdef0123456789abcdef01234567",
  "archive": {
    "fileName": "agent-bar.usage-10.0.0-x86_64-unknown-linux-gnu.tar.zst",
    "size": 1234567,
    "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
  },
  "releaseNotesUrl": "https://github.com/othavi0/agent-bar/releases/tag/v10.0.0"
}
```

The tracked English release-notes source is
`docs/releases/10.0.0.md`. The internal builder command is exactly:

```text
agent-bar-bundle assemble output <plugin-dir>
  source-commit <40-lowercase-hex>

agent-bar-bundle release bundle <plugin-dir> output <output-dir>
  source-commit <40-lowercase-hex> release-notes <path>
```

`assemble` creates and validates a bundle using the explicit source commit; it
does not claim the worktree matches that value. `release` requires a clean
worktree whose `HEAD` equals `source-commit`, validates the complete staged
bundle/receipt and release-notes file, and writes archive, checksum, closed
metadata JSON, and `LICENSE` to an initially empty output directory. It refuses
overwrite, missing notes, version/target/receipt mismatch, or a later source
change. The builder is an internal development binary, not installed in the
plugin.

## Installation

- `BUNDLE-013`: The existing `install.sh` is rewritten as a minimal
  plugin-scoped bootstrap. It downloads or accepts the exact
  release bundle, verifies SHA-256, stages, validates, and atomically installs
  it in the resolved Omarchy user plugin directory.
- `BUNDLE-014`: Installation records source release, source commit, target,
  checksum, plugin ID, and version in transaction state.
- `BUNDLE-015`: Existing v9 state follows the migration transaction before
  replacement.
- `BUNDLE-016`: Existing bar placement is preserved.
- `BUNDLE-017`: Fresh installation creates one entry through the supported
  Quattro path.
- `BUNDLE-018`: Rescan is always performed after a successful swap.
- `BUNDLE-019`: A failed install restores the previous complete plugin and
  shell entry.
- `BUNDLE-019A`: Fresh installation uses
  `omarchy plugin enable agent-bar.usage` once. Existing installation uses
  rescan only. Neither path runs `omarchy bar plugin add`.

## Update check

The private command surface includes:

```text
agent-bar update
agent-bar update check
agent-bar update apply <version>
```

- `BUNDLE-020`: Bare `update` is an interactive recovery flow: check, display
  target, confirm, apply.
- `BUNDLE-021`: `update check` returns a machine-readable document containing
  current version, latest compatible version, availability, release URL,
  release-notes URL, target, and checksum metadata.
- `BUNDLE-022`: `update apply <version>` accepts only the exact compatible
  version selected from a fresh official check.
- `BUNDLE-023`: The Settings UI performs check, confirmation, and apply as
  separate states.
- `BUNDLE-024`: Version checks use only the official Agent Bar release source.
- `BUNDLE-025`: No update downloads or executes a remote install script.

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
  "latestCompatible": {
    "version": "10.1.0",
    "omarchyContract": 1,
    "minimumQuickshellVersion": "0.3.0",
    "archiveUrl": "https://github.com/othavi0/agent-bar/releases/download/v10.1.0/agent-bar.usage-10.1.0-x86_64-unknown-linux-gnu.tar.zst",
    "checksumUrl": "https://github.com/othavi0/agent-bar/releases/download/v10.1.0/agent-bar.usage-10.1.0-x86_64-unknown-linux-gnu.tar.zst.sha256",
    "archiveSha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    "releaseNotesUrl": "https://github.com/othavi0/agent-bar/releases/tag/v10.1.0",
    "sourceCommit": "0123456789abcdef0123456789abcdef01234567"
  }
}
```

`latestCompatible` is `null` when no well-formed non-draft, non-prerelease
release satisfies the current target, supported Omarchy contract, and
installed Quickshell minimum. Well-formed but locally incompatible releases
are skipped. Malformed metadata or incomplete assets for a release that claims
the current target/contract are command errors, not silently skipped.
Otherwise `latestCompatible` describes the newest compatible release,
including the current version when no newer version exists. `available` is
true exactly when that version is strictly newer than `current.version`.
Unknown fields, redirects
outside the download policy below, non-HTTPS URLs, malformed hashes, target
mismatches, unsupported contract, insufficient Quickshell, and incomplete
release assets are contract failures. `update check` writes only this JSON
document plus newline to stdout; diagnostics go to stderr.

Release discovery begins only at
`https://api.github.com/repos/othavi0/agent-bar/releases`. Metadata URLs and
every initial release-asset request must remain under the exact
`https://github.com/othavi0/agent-bar/releases/` path. An asset download may
follow at most five HTTPS redirects to `github.com` or a hostname whose suffix
is `.githubusercontent.com`; userinfo, IP-literal hosts, non-default ports,
scheme downgrade, and every other host are rejected. No provider credential,
cookie, or authorization header is attached to release download redirects.

`update apply <version>` performs its own fresh check, writes the exact selected
object to the transaction journal, and proceeds only when `<version>` equals
`latestCompatible.version` and `available` is true. All downloaded bytes are
verified against `archiveSha256`; the checksum sidecar is corroborating release
evidence, not a substitute for the pinned journal hash.

Omarchy contract `1` means all of these are required:

- Quattro manifest service and bar-widget entry points;
- `manifest.__sourceDir` service injection;
- `bar.shell.serviceFor(moduleName)`;
- `KeyboardPanel`, `PanelKeyCatcher`, and `BarWidget`;
- `IpcHandler` reached through `omarchy-shell`;
- `omarchy plugin validate`, `plugin enable`, and asynchronous `plugin rescan`;
- `shell ping` and structured `shell listPlugins`.

Setup/update preflight requires regular readable Quattro QML components,
executable Omarchy commands, `quickshell --version >= 0.3.0`, successful shell
ping, valid `listPlugins` JSON, and validation of the staged bundle. Existing
update additionally requires the old Agent Bar health endpoint before any
download/swap. A release is `latestCompatible` only when its target and
contract metadata pass these probes; package version alone is insufficient.

## Update transaction

- `BUNDLE-026`: Download the complete target bundle to an isolated temporary
  path.
- `BUNDLE-027`: Verify checksum, architecture, archive inventory, manifest ID,
  schema, manifest version, and helper version before extraction can affect the
  live plugin.
- `BUNDLE-028`: Refuse downgrade unless a separately approved recovery path is
  used.
- `BUNDLE-029`: Refuse to replace a modified/ambiguous plugin directory without
  preserving and reporting it.
- `BUNDLE-030`: Back up, swap, rescan, and health-check as one transaction.
- `BUNDLE-031`: Restore the previous complete bundle on failure.
- `BUNDLE-032`: Directory replacement uses Linux
  `renameat2(RENAME_EXCHANGE)`. If the filesystem cannot provide exchange
  semantics, update fails before replacement instead of exposing a missing or
  half-installed plugin directory.
- `BUNDLE-032A`: Before self-update or uninstall, the helper copies and verifies
  itself inside the transaction directory under the executable name
  `agent-bar-maintenance-worker`.
- `BUNDLE-032B`: The helper launches that copy in a transient user systemd unit
  using argv. There is no permanent daemon.
- `BUNDLE-032C`: Worker mode is selected by the copied executable filename and
  a validated transaction journal, not by a public hidden flag or shell string.
- `BUNDLE-032D`: The worker performs exchange/removal, rescan, health IPC,
  rollback, final journal state, and desktop notification after the initiating
  QML object can be destroyed.
- `BUNDLE-032E`: `Service.qml` exposes one plugin-scoped health IPC endpoint
  with the contract in `02-target-architecture.md`. The maintenance worker
  requires `omarchy-shell agent-bar.usage health <expectedVersion>` to return
  `ok` after update.
- `BUNDLE-032F`: After uninstall rescan, the worker calls
  `omarchy-shell shell listPlugins`, parses the returned JSON, and requires that
  no entry has exact ID `agent-bar.usage`. Quiet/best-effort IPC is forbidden
  for maintenance health gates.
- `BUNDLE-032G`: Rescan is asynchronous. After the rescan command returns, the
  worker polls a monotonic 15-second deadline with delays
  `100, 200, 400, 500, 500...` milliseconds. Update requires health stdout
  exactly `ok\n` and exit `0`. Uninstall requires parsed `listPlugins` absence
  and failure/absence of the old service health endpoint. Timeout, malformed
  output, or mismatch triggers rollback. A restored v10 bundle must pass the
  same health poll for its previous version. A restored v9 bundle has no health
  IPC: rollback instead verifies every restored file against the pre-transaction
  backup manifest, restores exact `shell.json` bytes, rescans, and requires
  parsed `listPlugins` to contain the exact enabled `agent-bar.usage` entry.
  Fresh-install rollback verifies exact shell bytes and exact plugin absence.
- `BUNDLE-032H`: Maintenance preflight requires a reachable user systemd
  manager, shell ping, executable absolute command paths, a synced journal,
  and a verified worker copy. It starts unique unit
  `agent-bar-maintenance-<32-lowercase-hex-txid>.service` using
  `systemd-run --user --collect`, `Type=exec`, `UMask=0077`, and
  `TimeoutStartSec=120` and `RuntimeMaxSec=600`. The worker uses injected
  monotonic deadlines: preflight/download/stage must finish by 420 seconds,
  live mutation by 510, and rollback by 570, leaving 30 seconds for durable
  failure reporting before systemd's hard bound. It does not begin live
  mutation without the reserved rollback window. The only forwarded
  environment names are `HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`,
  `XDG_STATE_HOME`, `XDG_RUNTIME_DIR`, `DBUS_SESSION_BUS_ADDRESS`,
  `WAYLAND_DISPLAY`, and `OMARCHY_PATH`. Missing optional values are omitted.
  Worker-internal executables use absolute paths recorded during preflight.
- `BUNDLE-032I`: The caller reports successful handoff only after systemd has
  accepted and executed the worker. Failure to satisfy preflight or start the
  unit occurs before live mutation.
- `BUNDLE-032J`: Bundle stage/quarantine and purge quarantine use the
  destination-local sibling locations defined by `MIG-002A`. XDG state stores
  journal/worker/report/backups but is never assumed to share a filesystem
  with the plugin or settings.
- `BUNDLE-032K`: After a successful update health check and fsynced commit, the
  old bundle left in the exchange sibling is post-commit garbage collection.
  Cleanup failure records its exact residual path in the durable report and
  never claims rollback. A locally modified bundle accepted through an
  approved recovery path is copied to durable backup before this cleanup.

## UI uninstall

- `BUNDLE-033`: Standard uninstall quarantines then removes the shell entry,
  bundle, cache, notification state, transaction runtime, and confirmed owned
  legacy files.
- `BUNDLE-034`: Standard uninstall preserves settings and migration backups.
- `BUNDLE-035`: Purge requires the explicit UI selection or an interactive
  terminal confirmation.
- `BUNDLE-036`: Purge confirmation from QML is a structured stdin document,
  not a shell token, using the exact schema in
  `03-cli-and-json-contract.md`.
- `BUNDLE-037`: The reversible phase backs up exact shell bytes; moves bundle,
  cache, confirmed legacy, and selected purge paths to destination-local
  quarantine; removes exact shell entries; rescans; and verifies absence. Only
  then does the worker fsync a commit record and sanitized report under
  `$XDG_STATE_HOME/agent-bar/reports/<txid>.json`.
- `BUNDLE-038`: A desktop notification reports successful uninstall because
  the plugin UI no longer exists.
- `BUNDLE-038A`: Uninstall first quarantines the bundle by same-filesystem
  rename. It deletes quarantine only after shell absence is verified; rollback
  restores the bundle and exact previous `shell.json` bytes.
- `BUNDLE-038B`: The fsynced commit record is the irreversible boundary.
  Failures before it roll back every quarantine and verify the restored
  service. After it, quarantine deletion and transaction-runtime removal are
  garbage collection: failure records exact residual paths in the durable
  report and notification and never claims rollback.
- `BUNDLE-038C`: Successful cleanup removes the worker copy and transaction
  journal as its last filesystem action. Failed or incomplete rollback keeps
  the journal and worker evidence for `doctor`; the durable report is never
  stored inside a path standard uninstall deletes.

## Release boundary

- `BUNDLE-039`: The implementation prepares version `10.0.0`, changelog,
  migration guide, release-notes draft, archive, and checksum.
- `BUNDLE-040`: Grok may commit, push the feature branch, and open a ready PR.
- `BUNDLE-041`: Grok may not merge, tag, publish a GitHub Release, distribute
  the archive, or change the live desktop before the authorized QA gate.
- `BUNDLE-042`: Publishing requires final Codex review, passing live QA, user
  merge, and separate explicit release authorization.
