# Product Contract

Amended by the plugin-ID rename (2026-08-06):
`docs/superpowers/specs/2026-08-06-plugin-id-rename-design.md`. The plugin ID
is `othavi0.agent-bar`; it read `agent-bar.usage` when this document was
approved.

## Purpose

Agent Bar shows normalized quota and reset information for Claude, Codex, Amp,
and Grok in Omarchy Quattro. It provides one compact bar chip per enabled
provider and one consolidated Quickshell popup for details, settings,
connection actions, update, and uninstall.

## Goals

- `PROD-001`: Make `othavi0.agent-bar` the only graphical Agent Bar surface.
- `PROD-002`: Use Omarchy Quattro and Quickshell native patterns.
- `PROD-003`: Keep provider-specific logic in a testable private Rust helper.
- `PROD-004`: Share provider state and polling across all monitors.
- `PROD-005`: Preserve valid v9 user settings and bar placement.
- `PROD-006`: Remove TUI, Waybar, history, local cost, BRL conversion, and
  chart complexity rather than hiding them.
- `PROD-007`: Give normal users complete configuration, update, and uninstall
  flows in the plugin UI. Update and uninstall remain UI journeys; since
  git-plugin-distribution (2026-08-05) they delegate their live mutation to
  the Omarchy CLI (`omarchy plugin update|remove othavi0.agent-bar`) instead of
  performing it in-process.
- `PROD-008`: Expose typed, safe, partial provider failures without parsing
  human error strings in QML.
- `PROD-009`: Treat keyboard navigation, scrolling, focus, themes, and absence
  of Agent Bar-authored motion as release gates.
- `PROD-010`: Ship the QML, icons, scripts, and matching Rust helper as one
  versioned plugin bundle. Since git-plugin-distribution (2026-08-05), "the
  bundle" is the distribution repository tree
  (`othavi0/omarchy-agent-bar`, one append-only commit per release): the same
  assembled tree is what CI pushes, what `omarchy plugin add` clones, and
  what an installed plugin directory is a git checkout of.

## Non-goals

- `PROD-011`: No TUI or terminal dashboard.
- `PROD-012`: No Waybar formatter, module, CSS, setup, or compatibility mode.
- `PROD-013`: No detailed session history or local usage scan.
- `PROD-014`: No local token-cost engine, currency conversion, or charts.
- `PROD-015`: No permanent Rust daemon.
- `PROD-016`: No credential form, credential proxy, or automatic provider CLI
  installation.
- `PROD-017`: No schema-v1 compatibility decoder or legacy feature flag.
- `PROD-018`: No standalone application, AUR package, cargo-binstall product,
  or global `agent-bar` executable.
- `PROD-019`: No custom theme editor or v10 internationalization layer.
- `PROD-019A`: No monetary values are displayed or serialized, including
  provider-reported spend, dollar balance, and credits.

## Supported providers and defaults

Fresh installations use:

```text
Provider order: Claude, Codex, Amp, Grok
Enabled providers: all
Display metric: remaining
Refresh interval: 60 seconds
Notifications: enabled
```

- `PROD-020`: The provider rail and bar follow the settings order.
- `PROD-021`: Enabled providers remain visible when their CLI is missing.
- `PROD-022`: Missing-CLI providers use a dimmed icon and an installation
  action; Agent Bar never installs the provider CLI.
- `PROD-023`: Disabled providers disappear from the bar and provider rail but
  remain available in Settings.
- `PROD-024`: Migration preserves valid user choices instead of applying fresh
  defaults.

## Primary user journeys

### Read quota

1. The bar shows one compact chip per enabled provider.
2. Left-clicking a chip opens that provider in the consolidated popup.
3. The popup shows plan, usage windows, reset times, and typed error/action
   state; connection state is implied structurally and the last-success age
   appears as a neutral caption while a reading is retained (`UX-028`).
4. Clicking the same chip closes the popup; clicking another chip switches
   provider without closing it.

### Refresh

1. The service polls automatically using provider cache policy.
2. The provider-header refresh action bypasses cache for that provider once.
3. Middle-clicking any bar chip bypasses cache for all enabled providers once.
4. Concurrent forced requests are coalesced without being discarded.

### Sign in

1. A detected but unauthenticated provider shows `Sign in` when login
   discovery succeeds; otherwise it shows `Install guide`.
2. The action opens the configured terminal through the bundled Bash helper.
3. The private Rust helper delegates to the provider's official login command.
4. Success preserves the provider command's meaningful status and forces a
   provider refresh.
5. Agent Bar never receives credentials.

### Configure

1. Right-clicking a chip opens Settings.
2. The user can enable and order providers, choose used/remaining, set the
   interval, and toggle notifications.
3. Edits remain a draft until `Save changes`.
4. `Restore defaults` resets only the draft; `Cancel` discards it.

### Maintain

1. Settings shows installed plugin version and `Check for updates`.
2. An available release requires explicit confirmation before applying it.
3. Confirming update delegates to `omarchy plugin update othavi0.agent-bar
   --yes`, which fetches, fast-forwards, re-validates, and rolls back
   automatically on a failed validation. A plugin directory without `.git`
   (a pre-conversion install) cannot be fast-forwarded; the check reports
   `reinstallRequired` and Settings shows the remove-then-add migration
   instruction instead of an update offer.
4. `Uninstall Agent Bar` requires confirmation, then delegates to
   `omarchy plugin remove othavi0.agent-bar --yes`.
5. Settings are preserved by default; deleting settings requires an additional
   explicit purge selection, applied before the delegated remove.

## Public state principles

- `PROD-025`: The complete last good result remains visible as stale after a
  temporary failure, including a valid ready result with zero windows.
- `PROD-026`: Missing CLI, unauthenticated, rate-limited, network, and provider
  failures are distinct states.
- `PROD-027`: State is never communicated by color alone.
- `PROD-028`: Raw provider output, HTML, secrets, and credentials never become
  UI copy.
- `PROD-029`: Partial provider failure does not hide successful providers.
- `PROD-030`: The plugin must never display an empty or silently malformed
  status result.
- `PROD-031`: A connected provider with no percentage window shows `—` in its
  chip and `This account is billed another way.` in its popup.
