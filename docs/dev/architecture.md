# Architecture

## System

```text
Provider CLIs, HTTP, and local provider data
                    |
                    v
        private Rust provider adapters
                    |
                    v
       collection/cache/settings modules
                    |
          JSON status schema v2
                    |
                    v
       Quickshell Service.qml (one)
                    |
       +------------+------------+
       |                         |
 Monitor-local widget      Monitor-local widget
       \                         /
        \--- one logical popup --/
```

## Quickshell ownership

The manifest declares `service` and `bar-widget`.

`Service.qml` exists once per shell and owns:

- automatic polling;
- status/config/maintenance child-process scheduling;
- immutable provider snapshots;
- selected provider and logical popup owner;
- settings load/save generations;
- forced-refresh coalescing;
- notification-evaluation requests.

Each `BarWidget.qml` exists per monitor and owns:

- provider chips;
- Quattro click-target registration;
- the monitor-local popup anchor and visible `Popup` / `KeyboardPanel`;
- optional foreign-monitor overlay when the popup is owned elsewhere: a
  click on that monitor's own chip strip is forwarded to the chip under the
  cursor, and any other press dismisses the popup (`dismissPopup()` clears
  ownership);
- rendering derived from the shared service.

The consolidated popup uses an icon rail (providers + Settings), a
content-fit card height, overflow-gated vertical scrolling, one lead
percentage window, and compact rows with a progress track. Widgets do not own
polling, provider state, settings persistence, or cache.

`Service.qml` stays declarative by delegating its logic to four JS modules
loaded beside it: `CoreService.js` (polling, generations, forced-refresh
coalescing), `CoreSettings.js` (draft and persisted settings flow),
`CoreMaintenance.js` (update and uninstall flow), and `CoreView.js`
(chip and popup presentation data, tooltips, severity cues).

## Rust boundaries

| Module | Responsibility |
| --- | --- |
| `cli` | Strict word grammar, dispatch, exit behavior |
| `status` | Schema v2, human status, collection coordination |
| `providers` | Catalog, discovery, fetch, parsing, normalization |
| `settings` | Strict settings schema, read-only show, atomic apply, migration |
| `cache` | Normalized cache, generation lock, singleflight |
| `notifications` | Threshold transitions, dispatch, persisted deduplication |
| `plugin` | Paths, ownership, transactions, Omarchy, doctor, maintenance |
| `support` | Atomic files, clock, filesystem, redaction |

Provider adapters receive a narrow context for process, HTTP, filesystem,
clock, and redaction. Claude may collect through HTTP, Grok through
authenticated billing HTTP, Codex through a composite app-server/session-log
flow, and Amp through its CLI. Adapters are not forced into a command-only
abstraction.

## Data contract

Providers produce typed domain results. Only `status::schema` serializes status
JSON. QML does not know provider response formats and does not parse human error
messages.

Operational provider failures are provider states inside a valid envelope.
Fatal syntax, settings, contract, serialization, or transaction errors are
process failures.

## Cache and concurrency

Provider results are cached only after normalization. The cache contains no raw
payload or credential.

Every request records its start time. Under the cross-process lock, it rechecks
per-provider generations:

- cache-use accepts a valid generation;
- cache-bypass accepts only a live generation started no earlier than its
  request;
- forced targets that reach the shared service during one active fetch are
  unioned, with `all` dominating, into one later helper generation;
- disjoint external helper calls serialize without treating different targets
  as equivalent;
- every successful live collection updates cache.

`Service.qml` keeps `pendingForcedTargets`, not a boolean, so provider-only
refreshes cannot become accidental all-provider refreshes or disappear.

A dedicated two-second private-helper `version` probe establishes health before
provider network collection. Independent QML process lanes prevent status,
settings, version, and maintenance requests from cancelling each other.

## Settings

`settings.json` is the only Agent Bar product configuration. `shell.json`
contains only plugin presence and placement. Settings show is pure; apply
validates a complete document before lock and atomic replacement.

The UI separates persisted snapshot, mutable draft, and in-flight immutable
payload. Generation IDs prevent stale callbacks.

## Plugin maintenance

`update apply` and `uninstall` delegate their live mutation to the Omarchy
CLI rather than staging, exchanging, or rolling back the plugin directory
themselves:

1. resolve `omarchy` and `systemd-run` to absolute executable paths
   (fails closed before anything destructive if either is missing);
2. `uninstall purge` removes Agent Bar's own XDG state here, before the
   handoff;
3. run `omarchy plugin update agent-bar.usage --yes` or
   `omarchy plugin remove agent-bar.usage --yes` as a detached transient
   `systemd-run --user` unit;
4. return once systemd has accepted and started the unit.

`omarchy plugin update` owns the git fetch, fast-forward, re-validation, and
`git reset --hard ORIG_HEAD` rollback on a failed validation. `omarchy
plugin remove` owns disabling the bar entry, deleting (or, for a non-git
directory, backing up) the plugin directory, and rescanning. Detaching the
unit lets the operation survive destruction of the initiating QML service
during rescan; there is no permanent daemon and no verified worker copy of
the helper.

`update check` fetches the distribution repository's `bundle.json` receipt
over HTTPS and reports `reinstallRequired: true` when the live plugin root
has no `.git` directory, so the UI can offer the one-time
remove-then-add migration instead of a false "up to date".

All status/config mutations, plus the purge/preflight/handoff step above,
hold the shared stable maintenance gate under XDG state. Maintenance holds
it exclusively while its own local work runs, preventing an external helper
from recreating cache or notification state mid-operation; it does not hold
the lock across the detached unit's own execution.

## Security boundaries

- argv process execution only;
- allowlisted HTTPS installation URLs;
- authenticated provider HTTP pinned to its exact HTTPS origin/path with
  redirects disabled and authorization values redacted;
- plain-text external strings;
- output size and timeout limits;
- archive traversal/link/device rejection;
- restrictive settings/cache/journal permissions;
- no credentials, raw provider data, account identifiers, or monetary data in
  UI, logs, cache, screenshots, or reports.

## Detailed contract

See [specs/v10/02-target-architecture.md](../specs/v10/02-target-architecture.md).
