# Private Helper Commands

> Target v10 command contract. The helper is bundled inside the plugin and is
> not the normal user interface.

Resolve it with:

```bash
PLUGIN="$HOME/.config/omarchy/plugins/agent-bar.usage/bin/agent-bar"
```

## Status

```text
agent-bar
agent-bar status
agent-bar status format human|json
agent-bar status provider <id>
agent-bar status cache use|bypass
agent-bar status notifications evaluate|skip
```

Status clauses may appear in any order and at most once. Defaults:

```text
format human
provider all enabled
cache use
notifications skip
```

Examples:

```bash
"$PLUGIN" status
"$PLUGIN" status format json provider claude
"$PLUGIN" status provider codex cache bypass format json
```

Only the shared Quickshell service uses `notifications evaluate`.

## Login

```bash
"$PLUGIN" login claude
"$PLUGIN" login codex
"$PLUGIN" login amp
"$PLUGIN" login grok
```

Login delegates to the official provider CLI. Agent Bar never receives
credentials and preserves the meaningful provider exit status.

## Settings

```bash
"$PLUGIN" config show
"$PLUGIN" config apply stdin
"$PLUGIN" config apply file /path/to/settings.json
"$PLUGIN" config apply json '{"schemaVersion":1,...}'
```

`show` is read-only. `apply` requires one complete valid settings document and
returns the canonical stored document.

## Plugin integration

```bash
"$PLUGIN" setup
"$PLUGIN" setup plugins-dir /temporary/plugins
"$PLUGIN" doctor scan
"$PLUGIN" doctor clean
```

The `plugins-dir` argument is an existing writable absolute parent that
contains or receives the `agent-bar.usage` child. `doctor scan` never writes.
`doctor clean` backs up and removes only confirmed owned legacy artifacts.

## Update

```bash
"$PLUGIN" update
"$PLUGIN" update check
"$PLUGIN" update apply 10.1.0
```

- `update` is the interactive recovery flow.
- `update check` returns machine-readable compatibility metadata.
- `update apply <version>` accepts only the exact version selected by a fresh
  official check.

Bare `update` requires TTY stdin and the exact confirmation line
`update agent-bar`; non-TTY automation uses `update check`/`update apply`.

Normal users use the Maintenance UI.

## Uninstall

```bash
"$PLUGIN" uninstall
"$PLUGIN" uninstall purge
```

Standard uninstall preserves settings and migration backups. Purge requires an
explicit UI selection or interactive terminal confirmation.

## Help and version

```bash
"$PLUGIN" help
"$PLUGIN" help status
"$PLUGIN" version
"$PLUGIN" --help
"$PLUGIN" --version
```

`--help` and `--version` are the only supported double-dash aliases. Every
other flag or v9 command is rejected.

## Output and exit codes

JSON mode writes one object plus newline to stdout. Logs and diagnostics use
stderr.

| Code | Meaning |
| --- | --- |
| `0` | Request processed; provider failures may still be typed data |
| `1` | Delegated login or generic operation failure |
| `2` | CLI grammar or unsupported value |
| `3` | Settings/input validation |
| `4` | Status/schema/serialization invariant |
| `5` | Plugin integration or transaction failure |
| `70` | Unexpected internal failure |

Set `RUST_LOG` for diagnostics. There is no verbose command option.
