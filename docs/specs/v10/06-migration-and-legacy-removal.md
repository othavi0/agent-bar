# Migration and Legacy Removal

## Transaction model

Since git-plugin-distribution (2026-08-05), plugin-directory mutation (fresh
install, update, and uninstall) is delegated to the Omarchy plugin manager,
which owns its own git-based fetch, fast-forward, validation, and rollback;
see "Update and uninstall transactions" below. The discipline described here
now covers only the two remaining in-process writers that still touch files
directly: v9-to-v10 settings migration and `doctor clean`'s legacy-artifact
removal. Both follow:

```text
preflight
  -> plan (ownership scan for doctor clean)
  -> backup
  -> write (atomic replacement for settings; backup-then-remove for doctor
     clean artifacts)
  -> manifest
```

- `MIG-001`: No affected path changes before preflight and backup succeed.
- `MIG-002A`: Durable backups live under XDG state, one directory per
  operation timestamp.
- `MIG-006`: Backups never live inside a directory being replaced.

`MIG-002` (same-filesystem staging), `MIG-003` (transaction journal),
`MIG-004` (staged-validation rollback), and `MIG-005` (verified rollback
report) described a stage, exchange, and journal pipeline that no live
command path ever used; that dead machinery was removed 2026-08-05 along
with the code that implemented it.

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
- `MIG-008`: Keep plugin ID `agent-bar.usage`. (Superseded 2026-08-06: the
  live ID is `othavi0.agent-bar`. The v9 migration matcher still recognizes
  the literal `agent-bar.usage` in legacy `shell.json` data on disk; see
  `docs/superpowers/specs/2026-08-06-plugin-id-rename-design.md`.)
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
- `MIG-019A`: Since git-plugin-distribution (2026-08-05), fresh installation
  is `omarchy plugin add <dist-repo-url>`, which clones, validates, moves
  the tree, and, after a separate confirmation or `--enable`, enables it
  and adds a missing bar entry. An already-installed but disabled plugin may
  still be enabled directly with `omarchy plugin enable agent-bar.usage`.
  Neither path ever follows with `omarchy bar plugin add`.
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

Replaced by git-plugin-distribution (2026-08-05):
`docs/superpowers/specs/2026-08-05-git-plugin-distribution-design.md`.
Update and uninstall no longer stage, exchange, or roll back the plugin
directory themselves; each hands its live mutation to the Omarchy CLI as a
detached transient `systemd-run --user` unit, so the helper process can
return as soon as the handoff is accepted without depending on the QML
service it may be running under.

- `MIG-020`: `update apply` takes no version argument and detaches
  unconditionally to `omarchy plugin update agent-bar.usage --yes`, which
  owns the git fetch, fast-forward, re-validation, and
  `git reset --hard ORIG_HEAD` rollback on a failed validation.
- `MIG-021`: `update check` reads the distribution repository's
  `bundle.json` receipt over HTTPS and reports `reinstallRequired: true`,
  forcing `available`/`latestCompatible` null, whenever the live plugin root
  has no `.git` directory, since a pre-conversion tree cannot be
  fast-forwarded and must be reinstalled through `omarchy plugin add`.
- `MIG-022`: `uninstall` purges only Agent Bar's own XDG state (with
  `purge`) under the exclusive maintenance lock, then detaches
  unconditionally to `omarchy plugin remove agent-bar.usage --yes`, which
  owns disabling the bar entry, deleting the plugin directory, and
  rescanning.
- `MIG-023`: Standard uninstall preserves settings, cache configuration, and
  migration backups; only the purge form removes them, and only before the
  detached handoff.
- `MIG-024`: Purge and the detached remove are ordered and disjoint: purge
  never touches the plugin directory, and `omarchy plugin remove` never
  touches `$XDG_CONFIG_HOME/agent-bar`, `$XDG_CACHE_HOME/agent-bar`, or
  `$XDG_STATE_HOME/agent-bar`.
- `MIG-025`: Both `update apply` and `uninstall` resolve `omarchy` and
  `systemd-run` to absolute executable paths before consuming the
  confirmation or purging any state, so a missing tool fails closed before
  anything destructive happens.
- `MIG-026`: A non-git plugin root removed by `omarchy plugin remove` is
  backed up by Omarchy to a timestamped sibling rather than deleted
  (verified Omarchy behavior), so the one-time migration path is safe.
  Agent Bar settings live outside the plugin directory and always survive
  it.

The pre-conversion stage/quarantine sibling and cross-filesystem-safe
quarantine paths (`PluginPaths::stage_dir`, `quarantine_dir`,
`settings_quarantine`, `cache_quarantine`, `backups_quarantine`) are no
longer produced by any live command path. Only `PluginPaths::backup_root`
survives, used by `setup`'s v9-to-v10 settings migration and `doctor
clean`, each under `$XDG_STATE_HOME/agent-bar/backups/<stamp>/`.
