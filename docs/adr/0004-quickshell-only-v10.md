# ADR 0004: Quickshell-only v10 plugin

Status: Accepted
Date: 2026-07-26

## Context

Agent Bar v9 combined a Rust provider backend, Waybar output, a large terminal
UI, local history/cost systems, standalone distribution, and an early
Quickshell widget. Omarchy Quattro makes Quickshell plugins the native future.
The v9 split created duplicate UI, duplicated polling across monitors, settings
ownership conflicts, lifecycle races, broken generic icons, and a large legacy
surface unrelated to the desired product.

Omarchy third-party plugins are unsandboxed user plugin directories. The native
plugin installer does not compile Rust or run install hooks, while Agent Bar's
provider normalization and transactional behavior remain better suited to a
testable Rust helper than QML.

## Decision

Agent Bar v10 is only the Omarchy Quattro plugin `agent-bar.usage`.

- One Quickshell service owns runtime state.
- Monitor-local widgets render shared state.
- A private Rust helper is bundled inside the plugin.
- The helper is not installed globally or distributed as a separate product.
- The complete plugin bundle is versioned, validated, updated, and rolled back
  as one unit.
- `settings.json` owns Agent Bar settings; `shell.json` owns only presence and
  placement.
- TUI, Waybar, history, charts, monetary data, standalone distribution, and
  schema-v1 status compatibility are deleted.
- Login delegates to official provider CLIs through an argv-safe terminal
  helper.
- Update and uninstall are available in Settings and use journaled
  transactions.

## Consequences

Positive:

- one graphical product and one polling owner;
- smaller active domain and dependency graph;
- provider behavior remains testable in Rust;
- native Quattro focus, scrolling, themes, and accessibility;
- migration and maintenance have explicit ownership and rollback.

Costs:

- the plugin release contains an architecture-specific private helper;
- the project maintains a plugin-scoped bootstrap because native plugin add
  does not install compiled helpers;
- update/uninstall require a transient maintenance worker to survive shell
  reload;
- v10 is a breaking release with data migration but no runtime compatibility.

## Canonical detail

The approved requirements and implementation plan live in
`docs/specs/v10/`.
