# Status JSON Schema v2

Consumed by the shared Quickshell service. The checked-in structural schema is
`schemas/status-v2.schema.json`.

Request:

```bash
PLUGIN="$HOME/.config/omarchy/plugins/othavi0.agent-bar/bin/agent-bar"
"$PLUGIN" status format json
```

Representative response:

```json
{
  "schemaVersion": 2,
  "helperVersion": "10.0.0",
  "generatedAt": "2026-07-26T18:42:00Z",
  "request": {
    "provider": null,
    "cache": "use"
  },
  "providers": [
    {
      "id": "claude",
      "name": "Claude",
      "state": "ready",
      "source": "live",
      "plan": {
        "id": "max",
        "label": "Max"
      },
      "account": {
        "label": "Personal"
      },
      "windows": [
        {
          "id": "session",
          "label": "Session (5h)",
          "usedPercent": 42.0,
          "remainingPercent": 58.0,
          "resetsAt": "2026-07-26T22:00:00Z"
        }
      ],
      "lastSuccessAt": "2026-07-26T18:42:00Z",
      "error": null,
      "action": null
    }
  ]
}
```

## States

| State | Meaning |
| --- | --- |
| `ready` | Fresh usable data |
| `stale` | Last good data retained after temporary failure |
| `cli_missing` | An executable required for collection is unavailable |
| `unauthenticated` | Credentials/session are absent or rejected |
| `rate_limited` | No usable cache and provider rate-limited |
| `network_error` | No usable cache and network failed |
| `provider_error` | No usable cache and provider response failed |

`loading` exists in the shared QML model before a completed helper response. A
completed status envelope never serializes `loading`.

## Invariants

- Percentages are finite and inside `0..=100`.
- Used plus remaining equals 100 within 0.01.
- Resets are UTC RFC 3339 or `null`.
- No `-1`, NaN, infinity, or string sentinel.
- A connected provider may have an empty windows array.
- Empty percentage data renders `—`; it does not fabricate a value.
- No spend, balance, credits, currency, cost, or arbitrary extras.
- One provider failure does not invalidate successful siblings.
- Provider order follows settings.
- Explicit provider requests work even when that provider is disabled.
- Helper version is strict semver and matches the loaded manifest in a healthy
  bundle.

## Errors and actions

Errors contain stable code, safe English message, and retryable boolean.
Actions are limited to:

```text
retry
login
view_installation
```

Installation targets are allowlisted official HTTPS URLs. Login actions contain
no command string. QML maps action kinds to typed service methods.

## Validation

The checked-in schema is `schemas/status-v2.schema.json`. It validates closed
structural and per-state shapes. A Rust semantic validator additionally checks
percentage sums, unique provider/window IDs, request ordering,
timestamp/state coherence, helper/package version equality, and the absence of
completed `loading` states.

Collection and login discovery are separate. Missing a login-only executable
does not hide otherwise collectable data. If credentials are absent or
rejected, the action is `login` when the login CLI exists and
`view_installation` when it does not.
