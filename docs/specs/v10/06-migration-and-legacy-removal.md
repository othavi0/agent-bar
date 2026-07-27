# Migration and Legacy Removal

## Transaction model

Every mutating operation follows:

```text
preflight
  -> ownership scan
  -> exact plan
  -> backup and manifest
  -> stage
  -> validate staged result
  -> atomic replacement
  -> Omarchy rescan
  -> health check
  -> commit journal or rollback
```

- `MIG-001`: No affected path changes before preflight and backup succeed.
- `MIG-002`: Staging occurs on the same filesystem as each replacement.
- `MIG-002A`: Journals, verified worker copies, reports, and durable backups
  live under XDG state. Exchange candidates and quarantine never do: each is a
  hidden sibling of the path it will replace or remove.
- `MIG-003`: The transaction journal records every completed step.
- `MIG-004`: Any failed validation or health check restores all affected
  components, not only the last file.
- `MIG-005`: Rollback is verified and included in the operation report.
- `MIG-006`: Backups never live inside a directory being replaced.

## Backup layout

```text
$XDG_STATE_HOME/agent-bar/backups/<timestamp>/
├── manifest.json
├── settings/
├── plugin/
├── shell/
└── legacy/
```

The manifest records:

- operation and transaction ID;
- source and restoration paths;
- ownership classification and evidence;
- before hash, size, type, and permissions;
- planned action;
- backup relative path;
- after hash when an operation succeeds.

## v9-to-v10 migration

- `MIG-007`: Migration is data migration only. No v9 behavior remains callable.
- `MIG-008`: Keep plugin ID `agent-bar.usage`.
- `MIG-009`: Preserve valid provider enablement, order, display metric, refresh
  interval, notification preference, bar section, index, and compatible inline
  layout.
- `MIG-010`: Move Agent Bar product settings into `settings.json`.
- `MIG-011`: Remove only Agent Bar-owned inline settings from `shell.json`.
- `MIG-012`: Never remove and re-add an existing bar entry.
- `MIG-013`: Never invoke an unconditional `bar plugin add` for an existing
  entry.
- `MIG-014`: Unknown legacy keys stay in the backup and report.
- `MIG-015`: Invalid recognized values abort before replacement.
- `MIG-016`: Re-running migration is idempotent.
- `MIG-017`: A fresh install uses approved defaults and adds one entry only when
  absent.
- `MIG-018`: Rescan reloads staged QML without altering placement.
- `MIG-019`: Shell restart is a last resort after a valid rescan fails.
- `MIG-019A`: Fresh setup uses `omarchy plugin enable agent-bar.usage`, which
  rescans and adds a missing bar entry. It never follows with
  `omarchy bar plugin add`.
- `MIG-019B`: Update does not edit `shell.json`.
- `MIG-019C`: Rollback restores the exact previous `shell.json` bytes.

## Ownership classification

```text
owned/current
owned/legacy
modified legacy
ambiguous
unrelated
```

- `CLEAN-001`: Automatic cleanup may remove only `owned/legacy`.
- `CLEAN-002`: Ownership requires an exact generated marker, recorded install
  manifest, known path plus matching content/hash, or another documented proof.
- `CLEAN-003`: Location or filename resemblance alone is not proof.
- `CLEAN-004`: Modified legacy and ambiguous artifacts remain untouched and are
  reported with paths and reason.
- `CLEAN-005`: Unrelated artifacts are neither listed nor opened beyond the
  minimum classification check.
- `CLEAN-006`: `doctor scan` is read-only.
- `CLEAN-007`: `doctor clean` creates a backup before removing confirmed legacy
  artifacts.

## Installed legacy removal

When ownership is proven, migration removes:

- generated Agent Bar Waybar module entries;
- generated Agent Bar Waybar CSS blocks;
- Waybar-specific installed scripts and menu routes;
- obsolete TUI-only installed helpers;
- `usage.redb` and Postcard history cache;
- obsolete notification/cache state;
- ManagedGit metadata and known old standalone-install artifacts;
- old Agent Bar inline shell settings after successful migration;
- QML files replaced by the complete staged v10 bundle.

No cleanup may:

- rewrite unrelated Waybar modules or CSS formatting;
- remove another Omarchy plugin;
- alter general bar position, layout, theme, Hyprland, or terminal settings;
- follow symlinks outside an owned root;
- recursively delete an unresolved or broad path.

## Source removal

The implementation deletes, rather than disables:

- `src/tui/**` and all TUI snapshots;
- `src/action_right.rs`;
- `src/usage/**`;
- `src/waybar/**`;
- Waybar, terminal-dashboard, Pango, chart, history, local-cost, and currency
  formatters no longer needed by human status;
- provider-reported spend, credit balance, and monetary extra fields;
- v9 QML monolith after replacement components exist;
- legacy CLI variants, hidden TTY fallback, watch/NDJSON behavior, and
  compatibility aliases;
- legacy schemas, fixtures, integration tests, and snapshots;
- unused feature flags and dependencies.

Expected dependency removals include:

```text
ratatui
crossterm
tui-input
throbber-widgets-tui
tachyonfx
redb
postcard
async-trait
serial_test
temp-env
insta
```

The implementation must prove each dependency is unused before editing
`Cargo.toml`. It must also reassess Waybar/Pango-only and history-only
dependencies from the actual post-refactor graph.

## Doctor report

`doctor scan` reports:

- plugin ID, path, manifest validity, and helper/manifest version match;
- settings validity and permissions;
- cache validity and permissions;
- shell entry count, section, index, and forbidden inline settings;
- current, confirmed legacy, modified legacy, and ambiguous artifacts;
- executable discovery for enabled providers;
- installed Omarchy and Quickshell compatibility;
- stale/incomplete transaction journals;
- maintenance-gate path, permissions, and active-lock state;
- exact actions `doctor clean` would take.

The report never prints account labels, provider payloads, credentials, or
tokens.

## Terminal login helper

The Bash helper is retained only for interactive provider login and rewritten:

- accept exactly two arguments: `login <provider>`;
- allow only `claude`, `codex`, `amp`, and `grok`;
- resolve a physical absolute plugin root from the directory containing
  `BASH_SOURCE[0]`;
- verify `<absolute-plugin-root>/bin/agent-bar` is a regular executable;
- `exec xdg-terminal-exec --app-id=org.omarchy.terminal
  --title=Agent Bar Login -- <absolute-plugin-root>/bin/agent-bar login
  <provider>` through argv;
- let `xdg-terminal-exec` honor the user's configured Omarchy terminal;
- preserve `"$@"` and provider exit status;
- never use an emulator fallback table, `command -v agent-bar`, `cmd="$*"`,
  `eval`, `sh -c`, or `bash -lc`.

## Update and uninstall transactions

- `MIG-020`: Update backs up and replaces the complete plugin directory.
- `MIG-021`: Update validates plugin ID, manifest schema, target architecture,
  version equality, archive inventory, checksum, and safe paths before swap.
- `MIG-022`: Uninstall removes the shell entry before final plugin removal but
  the initiating UI is expected to unload. The detached worker continues
  rescan, absence verification, commit/rollback, report, and notification.
- `MIG-023`: Standard uninstall preserves settings and backups.
- `MIG-024`: Purge deletes settings and owned backups only after explicit
  confirmation.
- `MIG-025`: Cache and notification runtime state are removed by both forms.
- `MIG-026`: Ambiguous legacy files remain after uninstall and appear in the
  completion report.

Exact same-filesystem locations use a validated 32-lowercase-hex transaction
ID:

```text
$HOME/.config/omarchy/plugins/.agent-bar.usage.stage-<txid>/
$HOME/.config/omarchy/plugins/.agent-bar.usage.quarantine-<txid>/
<settings-parent>/.settings.json.agent-bar-quarantine-<txid>
<cache-parent>/.agent-bar-cache-quarantine-<txid>/
<backup-parent>/.agent-bar-backups-quarantine-<txid>/
```

Hidden plugin siblings must not match a valid plugin ID. They intentionally
retain the complete candidate/quarantined bundle, including its root manifest,
so staged validation and rollback are possible; Quattro's registry must ignore
their dot-prefixed directory names. Cross-filesystem rename is never used.
Restoration from a durable backup first copies into a verified destination
sibling and then performs the destination-local atomic operation.
