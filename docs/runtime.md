# Runtime

> Target v10 runtime and ownership model; not yet implemented on the
> specification branch.

## Owned paths

| Path | Purpose |
| --- | --- |
| `$HOME/.config/omarchy/plugins/agent-bar.usage/` | Complete plugin bundle |
| `$XDG_CONFIG_HOME/agent-bar/settings.json` | Canonical product settings |
| `$XDG_CACHE_HOME/agent-bar/status-v2.json` | Normalized provider cache |
| `$XDG_CACHE_HOME/agent-bar/status.lock` | Cross-process collection lock |
| `$XDG_CACHE_HOME/agent-bar/notification-state-v1.json` | Alert deduplication |
| `$XDG_CACHE_HOME/agent-bar/notification.lock` | Alert evaluation/dispatch lock |
| `$XDG_STATE_HOME/agent-bar/backups/` | Exact migration/maintenance backups |
| `$XDG_STATE_HOME/agent-bar/transactions/` | Journals and transient workers |
| `$XDG_STATE_HOME/agent-bar/reports/` | Durable sanitized maintenance reports |
| `$XDG_STATE_HOME/agent-bar/maintenance.lock` | Stable shared/exclusive mutation gate |

Default XDG paths are `~/.config`, `~/.cache`, and `~/.local/state`.

The plugin root and Omarchy `shell.json` always use `$HOME/.config/omarchy` in
production.

## Bundle

The plugin bundle contains manifest, `bundle.json`, QML, approved icons, the
terminal helper, and private Rust helper. `bundle.json` records ID, version,
target, Omarchy contract, minimum Quickshell version, source commit, and
hash/size/mode for every other file.

No global `agent-bar`, application entry, package, or managed checkout exists.

## Settings

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

Unknown keys and invalid/duplicate/missing providers are rejected. Reads never
rewrite. Applies validate before lock and atomic replacement. File mode is
`0600`.

## Cache

Cache contains normalized status only. It does not contain:

- credentials or tokens;
- raw provider output or headers;
- account identifiers;
- monetary values;
- local session history.

Corrupt cache is quarantined and rebuilt. Temporary provider failure retains
last good data as stale.

## Provider data sources

- Claude may use local credentials plus provider HTTP.
- Codex may use app-server with a bounded local fallback.
- Amp uses its official usage command.
- Grok may use provider-owned local auth/session data.

Collection discovery is separate from interactive login-CLI discovery.

## Privacy

Logs, screenshots, checkpoints, cache, and doctor reports redact tokens,
credentials, raw payloads, headers, and account identifiers. External display
strings are sanitized English plain text.

## Permissions

Settings, cache, journals, backups, and transient worker copies are restricted
to the user. Bundle executable files are `0755`; nonexecutables use
deterministic nonexecutable modes. Bundles contain no symlinks.
