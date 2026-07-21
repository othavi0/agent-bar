# JSON output (`--format json` + `--watch`)

For non-Waybar bars (Quickshell, Eww, Ironbar) that render natively and want raw
structured data instead of the Waybar Pango envelope.

## Modes

```bash
agent-bar --format json                    # one-shot snapshot, all registered providers
agent-bar --format json --provider claude  # one-shot, single provider
agent-bar --watch                          # stream: one JSON object per line (NDJSON), default 60s
agent-bar --watch --interval 30            # custom poll floor
```

- `--watch` implies `--format json`.
- `--interval` is a **floor**, not a strict period: each provider has a ~10s
  fetch timeout, so a slow tick can take longer than the interval. Use ≥ 30s.
- json/watch emit **all registered providers**, independent of the Waybar
  enabled-providers setting. `--provider X` emits a single-provider envelope.
- stdout is pure JSON/NDJSON; logs go to stderr.

## Envelope

```json
{
  "schemaVersion": 1,
  "fetchedAt": "2026-06-17T19:00:00.000Z",
  "providers": [
    {
      "provider": "claude",
      "displayName": "Claude",
      "available": true,
      "plan": "Max",
      "primary":   { "remaining": 30, "used": 70, "resetsAt": "2026-06-17T20:09:59Z", "windowMinutes": 300 },
      "secondary": { "remaining": 65, "resetsAt": "2026-06-19T22:59:59Z" },
      "models":    { "Sonnet": { "remaining": 89, "resetsAt": "2026-06-19T22:59:59Z" } },
      "extra":     { "weeklyModels": { "Sonnet": { "remaining": 89, "resetsAt": "2026-06-19T22:59:59Z" } } }
    },
    { "provider": "amp", "displayName": "Amp", "available": false, "error": "Not logged in." }
  ]
}
```

## Fields

| Field | Type | Notes |
| --- | --- | --- |
| `schemaVersion` | number | Contract version. See stability below. |
| `fetchedAt` | string (ISO) | When agent-bar produced the snapshot — **not** the network fetch time. On a cache hit the underlying data can be up to the cache TTL (~5min) older. |
| `providers[]` | array | One entry per provider. Order is registry order (currently includes `claude`, `codex`, `amp`, `grok`) and is **not** part of the contract — key on the `provider` field, never on array index. |
| `provider` | string | `claude` / `codex` / `amp` / `grok`. |
| `displayName` | string | Human label. |
| `available` | boolean | Authenticated and fetched OK. |
| `account` / `plan` / `planType` | string | Optional; omitted when absent. |
| `primary` / `secondary` | `Window` | Optional quota windows. |
| `models` | map of `Window` | Optional per-model/bucket windows. |
| `extra` | object | **Unstable** — see below. |
| `error` | string | Present only on failure (key omitted when OK — check `'error' in p`). |
| `staleReason` | string | Present only when the data was served from an expired cache after a transient fetch error (timeout, expired token). The quota fields are the last known good values; the string is the user-facing reason. Omitted for fresh data. |

`Window`: `{ remaining: number, used?: number|null, resetsAt: string|null, windowMinutes?: number|null, severity?: string }`.
`remaining`/`used` are percentages (0-100). `used` is only present when a provider
reports a distinct "used" metric that is not simply `100 - remaining` (it can exceed 100 with overage).
`severity` is optional (`Option<String>`, omitted when absent) and comes from the
provider's own API — today only Claude populates it, from `limits[].severity`.
Known values: `normal`/`ok`/`warning`/`elevated`/`high`/`critical`/`exceeded`/`blocked`.
Consumers should fall back to a local threshold on `remaining` (≥60/30/10) when
`severity` is absent or unrecognized — this mirrors `severity_color_api` in
`src/tui/widgets/severity.rs`.

## Stability

The top-level fields and `primary`/`secondary`/`models` (`Window`) are the
**stable contract** covered by `schemaVersion`. `extra` mirrors internal
provider-specific structures and is **best-effort/unstable** — it may change
without a `schemaVersion` bump. Don't depend on `extra` shapes long-term.

Bump policy: `schemaVersion` increments when a stable field is removed, renamed,
or changes type/meaning. Adding a new optional field does **not** bump.

Absence convention: optional fields are **omitted** when absent (never `null`,
never a serialized `undefined`). Provider/window array order is not guaranteed.

## Quickshell example

One-shot (poll with a Timer):

```qml
import Quickshell.Io

Process {
  id: proc
  command: ["agent-bar", "--format", "json", "--provider", "claude"]
  running: true
  stdout: StdioCollector {
    onStreamFinished: {
      const data = JSON.parse(this.text);
      const p = data.providers[0];
      label.text = p.error ?? (p.primary.remaining + "%");
    }
  }
}
```

Stream (one long-lived process, NDJSON via SplitParser):

```qml
import Quickshell.Io

Process {
  command: ["agent-bar", "--watch", "--interval", "60"]
  running: true
  stdout: SplitParser {           // splits on "\n" by default
    onRead: (line) => {
      const data = JSON.parse(line);
      const claude = data.providers.find((p) => p.provider === "claude");
      label.text = claude ? claude.primary.remaining + "%" : "?";
    }
  }
}
```

`Process.command` is an argv array (no shell) — keep each argument separate.

## Future

A native Omarchy Quickshell bar-widget plugin
(`~/.config/omarchy/plugins/agent-bar/`) is a future step once Omarchy ships its
Quickshell release (v4).
