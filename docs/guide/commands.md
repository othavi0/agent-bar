# Private Helper Commands

The helper is bundled inside the plugin and is not the normal user interface.
Users interact through the Quickshell UI; these commands support diagnostics,
recovery, and the shared service.

Resolve it with:

```bash
PLUGIN="$HOME/.config/omarchy/plugins/othavi0.agent-bar/bin/agent-bar"
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

Status arguments may appear in any order and at most once. Defaults:

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
"$PLUGIN" config apply json '{"schemaVersion":1,"providers":[{"id":"claude","enabled":true},{"id":"codex","enabled":true},{"id":"amp","enabled":true},{"id":"grok","enabled":true}],"display":{"metric":"remaining"},"refreshIntervalSeconds":60,"notifications":{"enabled":true}}'
```

`show` is read-only. `apply` requires one complete valid settings document and
returns the canonical stored document.

## Plugin integration

```bash
"$PLUGIN" setup
"$PLUGIN" doctor scan
"$PLUGIN" doctor clean
```

`setup` takes no arguments; it migrates settings to the current schema.
Install and update go through `omarchy plugin add|update othavi0.agent-bar`.
`doctor scan` never writes. `doctor clean` backs up and removes only
confirmed owned legacy artifacts.

## Update

```bash
"$PLUGIN" update
"$PLUGIN" update check
"$PLUGIN" update apply
```

- Bare `update` has no interactive flow; it prints usage pointing at `update
  check` and `update apply`.
- `update check` returns machine-readable compatibility metadata read from
  this repository's own `bundle.json` git receipt (the repository root is
  the plugin tree; see [ADR 0006](../adr/0006-single-repository-distribution.md)).
- `update apply` takes no argument. It delegates unconditionally to
  `omarchy plugin update othavi0.agent-bar --yes` as a detached transient unit
  and returns as soon as the handoff is accepted; that command owns the
  fast-forward, re-validation, and automatic rollback on a failed
  validation.

Normal users use the Maintenance UI.

## Uninstall

```bash
"$PLUGIN" uninstall
"$PLUGIN" uninstall purge
```

Both forms require confirmation before any mutation:

- On a TTY, type the exact phrase `uninstall agent-bar` at the prompt.
- On non-TTY stdin, provide a strict JSON confirmation document:

  ```json
  {
    "schemaVersion": 1,
    "operation": "uninstall",
    "confirmed": true,
    "purgeSettingsAndBackups": false
  }
  ```

  `purgeSettingsAndBackups` must match the invoked form (`true` only for
  `uninstall purge`), `confirmed` must be `true`, and trailing bytes after
  the JSON object are rejected.

After confirmation, `uninstall` purges only Agent Bar's own XDG state, then
delegates unconditionally to `omarchy plugin remove othavi0.agent-bar --yes`
as a detached transient unit — that command owns the plugin tree and the
`shell.json` entry now. Standard uninstall preserves settings, cache, and
migration backups. Purge additionally removes `$XDG_CONFIG_HOME/agent-bar`,
`$XDG_CACHE_HOME/agent-bar`, and `$XDG_STATE_HOME/agent-bar` before the
handoff. Both forms print one stdout JSON line once the handoff is accepted:

```json
{
  "schemaVersion": 1,
  "operation": "uninstall",
  "purged": false,
  "delegated": true,
  "unit": "agent-bar-remove-<txid>.service"
}
```

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
| `1` | Generic operation failure, including login pre-flight failures |
| `2` | CLI grammar or unsupported value |
| `3` | Settings/input validation surfaced by `config` commands |
| `4` | Status/schema/serialization invariant; `status` also exits 4 when settings fail to load |
| `5` | Plugin integration or delegation failure |
| `70` | Unexpected internal failure |

`login <provider>` passes the delegated provider CLI's own exit code
through verbatim when the login command runs and fails; the reserved codes
above apply to the helper's own failures.

Set `RUST_LOG` for diagnostics. There is no verbose command option.
