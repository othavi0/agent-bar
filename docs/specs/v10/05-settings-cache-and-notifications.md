# Settings, Cache, and Notifications

## Canonical settings

```json
{
  "schemaVersion": 1,
  "providers": [
    { "id": "claude", "enabled": true },
    { "id": "codex", "enabled": true },
    { "id": "amp", "enabled": true },
    { "id": "grok", "enabled": true }
  ],
  "display": {
    "metric": "remaining"
  },
  "refreshIntervalSeconds": 60,
  "notifications": {
    "enabled": true
  }
}
```

- `SET-001`: `settings.json` is the only product settings source.
- `SET-002`: Every supported provider appears exactly once.
- `SET-003`: Array order is provider display order.
- `SET-004`: `display.metric` is `used` or `remaining`.
- `SET-005`: `refreshIntervalSeconds` is an integer in `30..=3600`.
- `SET-006`: Unknown keys, duplicate providers, unknown providers, missing
  providers, and invalid values are rejected.
- `SET-007`: Reads never rewrite, migrate, normalize, or delete keys.
- `SET-008`: Missing settings return defaults without creating a file.
- `SET-009`: Explicit apply, setup, or migration are the only writers.
- `SET-010`: Writes validate first, lock, preserve the previous file, write a
  same-filesystem temporary file, sync, rename, and sync the parent directory.
- `SET-011`: The final file is user-readable and user-writable only.
- `SET-012`: Success returns the canonical stored document.
- `SET-013`: `shell.json` contains no Agent Bar refresh or product settings.

## QML draft state machine

```text
closed
  -> loading
  -> clean
  -> dirty
  -> saving
  -> clean     on matching success
  -> dirty     on failure
```

- `SET-014`: Opening Settings captures an immutable persisted snapshot.
- `SET-015`: Controls remain unavailable until the matching load completes.
- `SET-016`: Every load and save receives a monotonically increasing generation
  ID.
- `SET-017`: A callback may mutate UI state only when its generation still
  matches the active request.
- `SET-018`: Save captures an immutable payload; later draft edits cannot alter
  the in-flight request.
- `SET-019`: Closing the popup never fabricates process completion.
- `SET-020`: Reopening during a save reflects actual busy state.
- `SET-021`: A successful save adopts only the canonical returned document.
- `SET-022`: Restore defaults mutates the draft only.
- `SET-023`: Service startup reads and applies the persisted settings once the
  helper is proven alive: provider visibility and order, display metric, and
  refresh interval govern the bar without the Settings popup ever opening. A
  failed or invalid startup read keeps in-memory defaults, and a load or save
  completed through the popup is newer than the startup read and wins. The
  startup read never touches the popup snapshot state above.

## Cache files

```text
$XDG_CACHE_HOME/agent-bar/status-v2.json
$XDG_CACHE_HOME/agent-bar/status.lock
$XDG_CACHE_HOME/agent-bar/notification-state-v1.json
$XDG_CACHE_HOME/agent-bar/notification.lock
$XDG_STATE_HOME/agent-bar/maintenance.lock
```

- `CACHE-001`: Cache contains normalized provider data only.
- `CACHE-002`: Cache contains no token, credential, raw provider response, raw
  headers, or raw stderr.
- `CACHE-003`: Cache writes use the shared atomic-file primitive and restrictive
  permissions.
- `CACHE-004`: Provider TTL is internal Rust catalog policy.
- `CACHE-005`: Only `Service.qml` performs automatic polling.
- `CACHE-006`: Automatic status uses `cache use`.
- `CACHE-007`: Manual status uses `cache bypass`.
- `CACHE-008`: Single-provider and all-provider collection use identical
  timeout, retry, normalization, and cache paths.

## Collection generations

Each request records `requestedAt` before lock acquisition. Each live provider
generation records `startedAt`, `completedAt`, and an increasing revision.

```text
request
  -> inspect valid cache
  -> acquire generation lock
  -> recheck cache/generation
  -> collect only if still required
  -> atomically publish result and generation
```

- `CACHE-009`: A cache-use caller accepts a valid generation after rechecking
  under the lock.
- `CACHE-010`: A cache-bypass caller accepts a live provider generation only
  when that generation's `startedAt` is equal to or later than its own
  `requestedAt`. A generation already active when force was requested never
  satisfies that request merely because it completed later.
- `CACHE-011`: A force request during an active service collection is retained
  with its exact provider target.
- `CACHE-012`: `Service.qml` owns `pendingForcedTargets`, represented as a set
  of provider IDs or `all`. Requests that arrive during one active helper are
  unioned; `all` dominates. Completion starts exactly one follow-up helper for
  that union, then clears only the targets captured by that follow-up.
- `CACHE-013`: An identical cross-process bypass accepts a qualifying
  generation after lock recheck. Disjoint cross-process provider targets may
  serialize as separate collections; they are never incorrectly treated as
  equivalent or overwritten.
- `CACHE-014`: Provider collection is concurrent with a bounded worker count.
- `CACHE-015`: Child stdout and stderr have size limits.
- `CACHE-016`: Timeout terminates and reaps the process.
- `CACHE-017`: One bounded retry is allowed only for classified transient,
  idempotent failures.
- `CACHE-018`: Corrupt cache is moved aside, reported by doctor, and rebuilt.
- `CACHE-019`: Cache failure never replaces last good QML state with an empty
  model.
- `CACHE-019A`: Every successful live collection, including cache bypass,
  atomically updates the normalized cache.
- `CACHE-019B`: Status holds the shared maintenance gate across cache and
  notification mutation. It cannot recreate cache/runtime state while an
  exclusive maintenance transaction is active.

The normalized cache has one closed document:

```json
{
  "schemaVersion": 2,
  "revision": 42,
  "providers": {
    "claude": {
      "startedAt": "2026-07-26T18:42:00Z",
      "completedAt": "2026-07-26T18:42:01Z",
      "expiresAt": "2026-07-26T18:47:01Z",
      "status": {}
    }
  }
}
```

`status` is one validated provider schema-v2 object without request-envelope
fields. Provider keys are closed IDs. A single-provider write merges only that
provider under the exclusive lock, increments `revision`, and preserves all
siblings byte-for-byte at the semantic value level. Cache expiry is
`now >= expiresAt`. Unknown fields, version mismatch, invalid timestamps,
invalid provider status, duplicate semantic IDs, or a partial write quarantine
the entire cache document.

## Stale behavior

- `CACHE-020`: Initial collection without data uses local `loading`.
- `CACHE-021`: Later collections keep the last snapshot visible.
- `CACHE-022`: Temporary failure with any prior ready result produces `stale`,
  including a retained result with zero percentage windows.
- `CACHE-023`: Auth and missing-CLI transitions clear misleading connected
  state.
- `CACHE-024`: `lastSuccessAt` remains the timestamp of the retained data.
- `CACHE-025`: A provider refresh affects only that provider; an all-provider
  refresh preserves sibling results independently.

## Notifications

Thresholds use the normalized used percentage regardless of display mode:

```text
normal:   used < 90
warning:  used >= 90
critical: used >= 95
```

- `NOTIFY-001`: Notification key is provider ID, window ID, and the nullable
  reset timestamp. Unknown reset is the single canonical `null` key.
- `NOTIFY-002`: Notifications only escalate normal -> warning -> critical.
- `NOTIFY-003`: During uninterrupted execution, the same level is emitted once
  per key.
- `NOTIFY-004`: Runtime notification state persists across shell restarts.
- `NOTIFY-005`: Recovery below 90 or a changed reset key, including a change
  between `null` and a timestamp, silently rearms.
- `NOTIFY-006`: Stale data and provider failures do not trigger usage alerts.
- `NOTIFY-007`: Disabling notifications produces no message and deletes no
  provider/cache data.
- `NOTIFY-008`: The single settings toggle controls all Agent Bar usage alerts.
- `NOTIFY-009`: Notification copy is safe English. The title names the
  provider and the window; the body states the percentage in the metric
  selected in Settings and, when the reset is known, the humanised time until
  it. Trigger thresholds stay on `usedPercent` regardless of the displayed
  metric.
- `NOTIFY-010`: Notification state is persisted only after successful dispatch
  confirmed by process exit `0`.
- `NOTIFY-011`: `Service.qml` requests evaluation with
  `status notifications evaluate`; Rust performs the transition, dispatch, and
  persistence algorithm.
- `NOTIFY-012`: Delivery is at-least-once. A process or machine crash after the
  desktop notification is accepted but before state fsync may repeat that one
  notification on recovery; the product does not claim impossible exactly-once
  desktop delivery.

Notification evaluation acquires `notification.lock`, reloads state, evaluates,
dispatches, and persists before releasing the lock. Entries are processed in
provider settings order and then window order. Each successful dispatch is
persisted atomically before the next entry, so a later failure does not lose
earlier acknowledgements. Recovery/rearm transitions that require no message
are persisted under the same lock.

The state document is closed and sorted by provider/window/reset:

```json
{
  "schemaVersion": 1,
  "entries": [
    {
      "providerId": "claude",
      "windowId": "session",
      "resetAt": "2026-07-26T22:00:00Z",
      "level": "warning"
    }
  ]
}
```

`resetAt` is either a UTC RFC 3339 string or `null`. Allowed persisted levels
are `warning` and `critical`; normal/rearmed keys are removed. Duplicate
composite keys (including duplicate null-reset keys), unknown providers,
unknown fields, or invalid timestamps quarantine the state and start a safe
empty evaluation.

The backend is the resolved executable `notify-send`. Rust waits at most five
seconds for:

```text
notify-send
  --app-name=Agent Bar
  --urgency=normal|critical
  <title>
  <body>
```

Warning title is `<Provider> <Window> is running low`; critical title is
`<Provider> <Window> is almost out`. Body is `<value>% <unit>. Resets in
<countdown>.` when the reset is known and still ahead, `<value>% <unit>.
Resets now.` once it has passed, and `<value>% <unit>.` when the window
carries no reset timestamp. `<unit>` is `left` or `used`, following the
Settings display metric; `<countdown>` is the same humanised form the popup
renders, shared with QML through one pinned table. Values pass the normal
plain-text sanitizer. Spawn failure, timeout, signal, or nonzero exit is a
dispatch failure: report it on stderr, leave that key unadvanced, continue no
later notifications in that evaluation, and still return the valid status
envelope.
