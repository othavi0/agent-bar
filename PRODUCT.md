# Product

## Users

Omarchy Quattro users who work with Claude, Codex, Amp, Grok, or Antigravity and need a
reliable glanceable answer to:

- How much percentage quota remains?
- When does the relevant window reset?
- Is the provider connected and fresh?
- What action is required when data is unavailable?

## Product purpose

Agent Bar is a Quickshell plugin, not a general terminal application. The bar
provides the glanceable state. The consolidated popup provides quota details,
provider actions, Settings, update, and uninstall without requiring the user to
learn a CLI: the update and uninstall buttons delegate to the Omarchy plugin
manager (`omarchy plugin update|remove`), which owns the actual mutation.

Success means:

- every visible value comes from normalized provider data;
- all monitors share one state and polling source;
- stale and partial failures remain understandable;
- Settings changes are recoverable; update and uninstall are explicit,
  confirmed actions delegated to the Omarchy plugin manager;
- pointer, keyboard, focus, scrolling, themes, and absence of Agent
  Bar-authored motion work as native Quattro behavior;
- the plugin never leaks credentials or raw provider output.

## Product personality

Quiet, compact, and native to Omarchy. Provider identity comes from official
icons. Severity is visible but not theatrical. The UI prefers clear labels
over decorative or ambiguous controls.

## Design principles

1. **Real data or an explicit unavailable state.**
2. **One source of truth per concept.**
3. **Typed states instead of human-message parsing.**
4. **Last good data remains visible when it is honestly stale.**
5. **No color-only meaning.**
6. **No custom theme system over Quattro.**
7. **Destructive maintenance is explicit and recoverable.**
8. **No credentials, monetary data, or local history.**

## Scope

Included:

- Claude, Codex, Amp, Grok, and Antigravity.
- Provider percentage windows and reset times.
- Plan and connection state.
- Typed missing/auth/network/rate/provider states.
- Provider login delegation through the official CLI.
- Provider enablement/order, used/remaining, interval, and notifications.
- Plugin update and uninstall delegated to the Omarchy plugin manager;
  settings migration with backup.

Removed:

- TUI.
- Waybar.
- Session history.
- Charts.
- Local costs and BRL conversion.
- Provider spend, balance, and credits.
- Standalone/AUR/cargo-binstall distribution.
- Permanent daemon.

## Canonical specification

[docs/specs/v10/README.md](docs/specs/v10/README.md) contains the approved
requirements and implementation plan.
