# Plugin Integration and Ownership

> Target v10 integration contract; not yet implemented on the specification
> branch.

## Installation

The plugin-scoped `install.sh`:

1. obtains the exact release bundle and checksum;
2. verifies checksum and archive safety;
3. stages the complete `agent-bar.usage` directory;
4. validates manifest, bundle receipt, versions, target, modes, and files;
5. runs v9 migration when required;
6. installs with same-filesystem transaction semantics;
7. enables a missing plugin entry or rescans an existing entry;
8. verifies the loaded version;
9. commits or rolls back.

Fresh installation uses:

```text
omarchy plugin enable agent-bar.usage
```

Existing setup/update uses:

```text
omarchy plugin rescan
```

Do not follow either command with `omarchy bar plugin add`. That command can
remove and recreate the entry, losing its section, index, and inline fields.

## Placement

`shell.json` owns plugin presence and placement. Agent Bar:

- creates one `{ "id": "agent-bar.usage" }` entry only when absent;
- preserves the exact section and index during migration;
- removes only its old inline refresh key after successful settings migration;
- does not edit `shell.json` during update;
- restores exact previous bytes on rollback;
- removes only exact Agent Bar entries during uninstall.

## Settings migration

Valid v9 provider enablement/order, used/remaining mode, refresh interval, and
notification preference migrate to the strict v10 settings document.

Invalid recognized values abort before replacement. Unknown fields remain in
the backup/report and do not enter v10. Repeated migration is idempotent.

There is no v9 runtime compatibility layer after migration.

## Transactions

Every mutation uses preflight, ownership scan, backup, journal, destination-
local staging/quarantine, validation, exchange, asynchronous rescan, bounded
health polling, and commit/rollback.

Update and uninstall continue in a transient user systemd unit from a verified
helper copy under transaction state. This survives the Quickshell service being
destroyed during rescan without creating a permanent daemon.

Status/config mutation holds the stable shared XDG-state maintenance lock.
Maintenance drains service-owned writers, takes the exclusive lock, rechecks
its plan, and retains exclusivity through commit or verified rollback.

The plugin candidate/quarantine is a hidden sibling under the Omarchy plugins
directory. Settings, cache, and backup purge use hidden siblings on their own
filesystems. XDG state stores journals, workers, reports, and durable backups;
it is not assumed to share a filesystem with any live target.

## Ownership

Artifacts are classified as:

- owned/current;
- owned/legacy;
- modified legacy;
- ambiguous;
- unrelated.

Only owned/legacy may be removed automatically. A known-looking path is not
enough; ownership requires a receipt, marker, expected content/hash, or another
documented proof.

`doctor scan` is read-only. `doctor clean` backs up before removing confirmed
legacy. Modified and ambiguous paths remain untouched.

## Uninstall

Standard UI uninstall:

- removes exact shell entries;
- quarantines then removes the plugin bundle;
- removes cache, notification state, and confirmed legacy;
- preserves settings and migration backups.

Purge additionally removes settings and owned backups after a separate explicit
selection. Every pre-commit failure restores bundle, exact shell bytes, and
purge quarantine. Post-commit garbage-collection failure keeps the successful
uninstall committed and records exact residual paths in the durable report; it
does not claim rollback.

## Live safety

Tests use injected plugin roots and isolated XDG paths. Live
`$HOME/.config/omarchy` mutation is limited to the final approved QA gate with
exact backup and verified rollback.
