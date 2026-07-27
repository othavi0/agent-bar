# Agent Bar Domain

Canonical product vocabulary. Do not import removed v9 Waybar or TUI terms into
new code.

## Surfaces

**Bar chip**
A compact provider icon and percentage rendered by `BarWidget.qml`.

**Consolidated popup**
The single logical Agent Bar popup. It opens on one monitor and shows one
provider or Settings at a time.

**Provider rail**
The vertical icon-only selector on the popup's left edge.

**Provider view**
The selected provider's plan, state, percentage windows, resets, and actions.

**Settings**
The popup view that edits the canonical Agent Bar settings draft.

**Maintenance**
The Settings section for update and uninstall.

## Runtime

**Shared service**
The one `Service.qml` instance loaded by Quattro. It owns polling, helper
process scheduling, state snapshots, popup ownership, settings generations, and
notification-evaluation requests.

**Bar widget instance**
A lightweight monitor-local view. It does not own polling or provider state.

**Private helper**
The bundled Rust executable at `bin/agent-bar`. It is an implementation detail,
not a standalone product.

**Plugin bundle**
The complete version-matched `agent-bar.usage` directory: manifest, QML, icons,
scripts, receipt, and private helper.

## Provider model

**Provider descriptor**
The canonical Rust catalog entry containing provider ID, name, icon key,
official installation URL, discovery metadata, TTL, and timeout.

**Collection availability**
Whether Agent Bar can obtain provider quota. It is separate from login CLI
availability.

**Login availability**
Whether Agent Bar can delegate to the provider's official interactive login
command.

**Usage window**
A percentage quota with stable ID, English label, used/remaining values, and
optional UTC reset timestamp.

**Last good data**
The most recent valid normalized provider snapshot.

**Stale**
Last good data retained after a temporary refresh failure.

## Configuration and state

**Settings document**
The strict complete document in
`$XDG_CONFIG_HOME/agent-bar/settings.json`.

**Persisted snapshot**
The canonical settings returned by a successful `config show` or apply.

**Draft**
The mutable Settings UI copy. It is never persisted implicitly.

**Collection generation**
A completed live provider collection used for cross-process singleflight and
forced-refresh coalescing.

**Transaction**
A journaled setup/migration/update/uninstall operation with backup, staging,
validation, commit, and rollback.

**Ownership evidence**
The proof that permits Agent Bar to classify and remove one of its own files.

## Terms to avoid

- `module` for a bar chip.
- `menu` or `TUI` for the popup.
- `Waybar settings`.
- `extra usage` for arbitrary provider data.
- `standalone install`.
- `available` as a substitute for typed provider state.
- human error-message regex as control flow.
