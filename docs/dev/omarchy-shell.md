# Omarchy Quattro Plugin Contract

## Installed path

Quattro discovers user plugins at the literal path:

```text
$HOME/.config/omarchy/plugins/othavi0.agent-bar/
```

It does not apply `XDG_CONFIG_HOME` to plugin discovery or `shell.json`.

## Manifest

```json
{
  "schemaVersion": 1,
  "id": "othavi0.agent-bar",
  "name": "Agent Bar",
  "version": "10.x.y",
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
    "defaultSection": "right",
    "aliases": ["agent-bar"],
    "allowMultiple": false,
    "defaults": {},
    "schema": []
  }
}
```

`version` (`10.x.y` above for illustration) always carries the exact
released version. `scripts/agent-bar-cut-release` bumps it in place at
release time, directly in the repository root's own `manifest.json` —
there is no build-time placeholder or separate assemble step to
substitute it (the repository root is the plugin tree; see
[ADR 0006](../adr/0006-single-repository-distribution.md)).
`barWidget.defaultSection` drives the section `omarchy plugin add`'s
interactive placement prompt preselects, and the section used when that
prompt is skipped.

Do not add `activation`, `keepLoaded`, or inline Agent Bar settings.

## Service injection

`Service.qml` declares:

```qml
property string omarchyPath: ""
property var shell: null
property var manifest: null
property var barWidgetRegistry: null
property var pluginRegistry: null
```

The absolute discovered plugin root is `manifest.__sourceDir`.

## Widget injection

`BarWidget.qml` receives `bar`, `moduleName`, and `settings`. It resolves the
singleton:

```qml
readonly property var agentService:
    bar && bar.shell ? bar.shell.serviceFor(moduleName) : null
```

`Service.qml` owns one `IpcHandler` target `othavi0.agent-bar`. Its closed
surface is `health(expectedVersion)` for maintenance and `refresh(providerId)`
for successful interactive login. The target architecture fixes the exact
return values and validation.

Service startup verifies the private helper with a dedicated two-second
`version` process before provider polling. Health therefore depends on
manifest/helper equality, not provider network latency.

Each chip registers with `bar.registerClickTarget()`, implements
`triggerPress(button)`, and unregisters on destruction.

## Popup

Use `KeyboardPanel` for reliable layer-shell keyboard focus. The shared service
tracks the popup owner across monitor-local bar instances because
`activePopout` is local to each bar.

`contentWidth` follows the viewport and `contentHeight` follows the content.
The fitted panel clips a vertical native `Flickable`, uses `StopAtBounds`, and
shows an as-needed scrollbar. Do not add custom wheel handling.

## Commands

Install (clones, validates, and moves the tree; prompts to enable):

```bash
omarchy plugin add https://github.com/othavi0/omarchy-agent-bar.git
```

Update (git fetch, fast-forward, re-validate; rolls back on a failed
validation):

```bash
omarchy plugin update othavi0.agent-bar
```

Remove (disables the bar entry, deletes or backs up the tree, rescans):

```bash
omarchy plugin remove othavi0.agent-bar
```

An already-installed but disabled plugin can be enabled directly:

```bash
omarchy plugin enable othavi0.agent-bar
```

Do not run `omarchy bar plugin add othavi0.agent-bar` over an existing entry;
it can reset placement.

## Interactive login

QML invokes the bundled Bash launcher with an argv array. The launcher resolves
the private helper from `manifest.__sourceDir`, delegates the configured
terminal choice to `xdg-terminal-exec`, and runs the helper by bundle path. It
does not assume a global executable, construct a shell string, or maintain its
own terminal-emulator fallback list.

## Validation

```bash
omarchy plugin validate /path/to/othavi0.agent-bar
# PATH qmllint is a stub; the Qt6 binary path is mandatory
find /path/to/othavi0.agent-bar -type f -name '*.qml' -exec \
  /usr/lib/qt6/bin/qmllint -I /usr/share/omarchy/shell {} +
```

Omarchy validation does not verify the Rust target, executable modes, bundle
hashes, version equality, or full inventory. Agent Bar's bundle validator owns
those checks.
