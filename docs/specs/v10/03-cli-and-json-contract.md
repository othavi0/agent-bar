# Private CLI and JSON Contract

Amended by the plugin-ID rename (2026-08-06):
`docs/superpowers/specs/2026-08-06-plugin-id-rename-design.md`. The plugin ID
is `othavi0.agent-bar`; it read `agent-bar.usage` when this document was
approved.

The bundled helper is not the normal user interface. Its command contract is
still strict because QML, tests, recovery procedures, and migration depend on
it.

## Grammar

```text
agent-bar
agent-bar status
agent-bar status format human|json
agent-bar status provider <id>
agent-bar status cache use|bypass
agent-bar status notifications evaluate|skip

agent-bar login <provider>

agent-bar config show
agent-bar config apply stdin
agent-bar config apply file <path>
agent-bar config apply json <value>

agent-bar setup
agent-bar update
agent-bar update check
agent-bar update apply
agent-bar uninstall
agent-bar uninstall purge

agent-bar doctor scan
agent-bar doctor clean

agent-bar help
agent-bar help <command>
agent-bar version
```

- `CLI-001`: Bare `agent-bar` equals `status format human`.
- `CLI-002`: `status` clauses may appear in any order.
- `CLI-003`: A clause may appear at most once.
- `CLI-004`: Duplicate clauses, missing values, unknown words, unsupported
  providers, and unknown values are hard errors.
- `CLI-005`: `cache use` is the default.
- `CLI-005A`: `notifications skip` is the default. Only the shared Quickshell
  service uses `notifications evaluate`.
- `CLI-006`: `--help` and `--version` are the only accepted double-dash aliases.
- `CLI-007`: Every other legacy command, alias, and flag is rejected.
- `CLI-008`: `RUST_LOG` controls diagnostics; there is no verbose argument.
- `CLI-009`: Since git-plugin-distribution (2026-08-05), `setup` takes no
  arguments. It migrates settings to the current schema only; it does not
  create, enable, or move any plugin tree, since `omarchy plugin add` is
  the install now. Isolated testing and manual recovery use an injected
  `HOME` (and `XDG_STATE_HOME`), not a command argument. Production uses
  the literal Quattro plugin root under the real `HOME`.
- `CLI-010`: Public help describes the plugin-first product and labels the
  helper CLI as diagnostics/recovery.

## Output and exit behavior

- `CLI-011`: `status format json` writes exactly one status schema-v2 JSON
  object followed by a newline to stdout.
- `CLI-012`: Human mode writes terminal-safe English text.
- `CLI-013`: Diagnostics and logs go to stderr.
- `CLI-014`: Provider failures are data. A valid envelope exits `0` even when
  one or all providers are in typed failure states.
- `CLI-015`: Syntax, settings, contract, serialization, and transaction
  failures exit nonzero.
- `CLI-016`: Serialization failure never becomes blank stdout.
- `CLI-017`: Login returns the official provider process status when it can be
  represented safely; signal termination or invalid platform status maps to
  `1`.
- `CLI-017A`: `version` writes exactly the package semantic version plus
  newline to stdout, writes no success text to stderr, performs no discovery or
  I/O beyond output, and exits `0`.

Reserved helper exit codes:

| Code | Meaning |
| --- | --- |
| `0` | Request processed and output contract satisfied |
| `1` | Delegated login or generic operation failure |
| `2` | CLI grammar or unsupported value |
| `3` | Settings/input validation |
| `4` | Status/schema/serialization invariant |
| `5` | Plugin integration or transaction failure |
| `70` | Unexpected internal failure |

Provider operational states never use these codes in JSON status mode.

## Status schema v2

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

### Provider states

```text
loading
ready
stale
cli_missing
unauthenticated
rate_limited
network_error
provider_error
```

- `JSON-001`: `ready` means fresh usable data.
- `JSON-002`: `stale` retains last good data after a temporary refresh failure.
- `JSON-003`: `cli_missing` means an executable required by the selected
  collection source was not found in its documented search locations.
- `JSON-004`: `unauthenticated` means credentials or provider session are
  absent or rejected, independent of login-executable availability.
- `JSON-005`: `rate_limited`, `network_error`, and `provider_error` apply when
  no usable cached result exists.
- `JSON-006`: If usable cached data exists after a temporary failure, state is
  `stale` and `error.code` carries the precise cause.
- `JSON-007`: Missing CLI and authentication failures do not present obsolete
  usage as connected.
- `JSON-008`: `loading` belongs to the shared service model before a final
  helper response. A completed `status` envelope must not serialize `loading`.

### Field invariants

- `JSON-009`: `schemaVersion` is integer `2`.
- `JSON-009A`: `helperVersion` is the helper's strict semantic version and
  equals the plugin manifest version in a healthy bundle.
- `JSON-010`: Provider IDs are `claude`, `codex`, `amp`, or `grok`.
- `JSON-011`: `source` is `live`, `cache`, or `null`.
- `JSON-012`: Percentages are finite numbers in `0..=100`.
- `JSON-013`: Used plus remaining equals `100` within `0.01`.
- `JSON-014`: Reset timestamps are UTC RFC 3339 strings or `null`.
- `JSON-015`: No value uses `-1`, NaN, infinity, or a string sentinel.
- `JSON-016`: Status without `provider` returns enabled providers in settings
  order.
- `JSON-017`: Explicit `provider <id>` returns one supported provider even if
  disabled in normal settings.
- `JSON-018`: Plan and account are structured objects or `null`.
- `JSON-019`: Account labels are sanitized and must not contain credentials.
- `JSON-020`: Provider windows have stable IDs and English labels.
- `JSON-021`: `lastSuccessAt` records the data generation, not the current
  display time.
- `JSON-022`: A provider error does not invalidate successful siblings.
- `JSON-022A`: A connected provider may return an empty `windows` array when
  its account exposes no percentage quota.
- `JSON-022B`: The schema has no spend, balance, credits, currency, cost,
  arbitrary extras, or generic monetary facts.
- `JSON-022C`: `rateLimitResetsAvailable`, when present, is a non-negative
  integer count of provider-granted rate-limit resets. It is a quota-reset
  count, not a monetary fact: it never carries balance, price, or currency,
  and `JSON-022B` continues to ban those.

### Structural and semantic validation

The checked-in JSON Schema validates structure, closed enums, required fields,
unknown fields, primitive ranges, formats, and state-specific shapes. Rust
also runs a semantic validator before serialization. The semantic validator
owns invariants that JSON Schema cannot express safely:

- used plus remaining equals `100` within `0.01`;
- provider IDs are unique and follow request/settings order;
- window IDs are unique within a provider;
- an explicit-provider response contains exactly that provider;
- timestamps and source/state relationships are coherent;
- `helperVersion` equals the running package version;
- no completed envelope contains `loading`.

Both validators are mandatory. Passing the structural schema alone does not
make an envelope valid.

Completed provider state truth table:

| State | `source` | `windows` | `lastSuccessAt` | `error` | `action` |
| --- | --- | --- | --- | --- | --- |
| `ready` | `live` or `cache` | zero or more | required | `null` | `null` |
| `stale` | `cache` | zero or more retained | required | retryable cause | `retry` |
| `cli_missing` | `null` | empty | `null` | `cli_not_found` | `view_installation` |
| `unauthenticated` | `null` | empty | `null` | `authentication_required` | `login` or `view_installation` |
| `rate_limited` | `null` | empty | `null` | `rate_limited` | `retry` |
| `network_error` | `null` | empty | `null` | `network_error` | `retry` |
| `provider_error` | `null` | empty | `null` | `provider_error` | `retry` |

`stale` exists for a retryable refresh failure with a retained prior ready
result. A previously ready provider with no percentage window retains its
plan/account/last-success state, remains `stale`, and renders `—`; it never
fabricates a window.

### Discovery-to-state mapping

Collection availability and login availability are evaluated independently:

| Collection result | Login executable | Provider state/action |
| --- | --- | --- |
| Available | either | collect; login availability does not change state |
| Required collection executable missing | either | `cli_missing` / `view_installation` |
| Credentials/session absent or rejected | available | `unauthenticated` / `login` |
| Credentials/session absent or rejected | missing | `unauthenticated` / `view_installation` |
| Collection attempted and rate-limited | either | `rate_limited` / `retry` |
| Collection attempted and network failed | either | `network_error` / `retry` |
| Collection attempted and payload invalid | either | `provider_error` / `retry` |

Absence of a login-only executable never converts otherwise collectable data
to `cli_missing`.

### Error and action

```json
{
  "state": "cli_missing",
  "source": null,
  "plan": null,
  "account": null,
  "windows": [],
  "lastSuccessAt": null,
  "error": {
    "code": "cli_not_found",
    "message": "Amp CLI was not found.",
    "retryable": false
  },
  "action": {
    "kind": "view_installation",
    "label": "Install guide",
    "target": "https://ampcode.com/manual"
  }
}
```

Allowed action kinds:

```text
retry
login
view_installation
```

- `JSON-023`: The real installation target must be an allowlisted official
  HTTPS URL from the Rust provider catalog.
- `JSON-024`: Login actions contain no shell command string.
- `JSON-025`: QML maps the closed action kind to a typed service method.
- `JSON-026`: Raw provider errors never become `message`.
- `JSON-027`: Messages are safe English plain text.
- `JSON-028`: HTML, ANSI, control characters, tokens, and provider payloads are
  rejected or sanitized at the Rust boundary.

## Settings command contract

- `CLI-018`: `config show` is read-only and returns canonical settings JSON.
- `CLI-019`: A missing settings file returns defaults without creating it.
- `CLI-020`: `config apply` accepts exactly one complete document.
- `CLI-021`: Validation completes before the write lock or filesystem mutation.
- `CLI-022`: Success returns the canonical stored document on stdout.
- `CLI-023`: Failure leaves the previous file byte-for-byte intact.
- `CLI-023A`: Successful `config show` and `config apply` write exactly one
  settings schema-v1 object plus newline to stdout. Diagnostics use stderr.

## Maintenance command contract

Amended by git-plugin-distribution (2026-08-05):
`docs/superpowers/specs/2026-08-05-git-plugin-distribution-design.md`.
`update apply` and `uninstall` no longer stage, exchange, or roll back the
plugin directory in-process; each resolves `omarchy` and `systemd-run` to
absolute paths, then detaches unconditionally to the Omarchy CLI as a
transient `systemd-run --user` unit and returns once the handoff is
accepted.

- `CLI-024`: `doctor scan` is read-only.
- `CLI-025`: `doctor clean` removes only confirmed owned legacy artifacts after
  creating a backup.
- `CLI-026`: `uninstall` preserves settings and migration backups.
- `CLI-027`: `uninstall purge` additionally deletes settings and owned backups
  only after an explicit UI or interactive confirmation, and only before the
  detached `omarchy plugin remove` handoff.
- `CLI-028`: QML passes structured intentions; it never concatenates command
  strings.
- `CLI-029`: `update apply` takes no version argument. It delegates
  unconditionally to `omarchy plugin update othavi0.agent-bar --yes`, which
  owns the git fetch, fast-forward, re-validation, and automatic
  `git reset --hard ORIG_HEAD` rollback on a failed validation.
- `CLI-030`: Setup, update, doctor, and uninstall never touch unrelated Omarchy
  plugins or layout entries.
- `CLI-031`: Notification dispatch failure is reported on stderr, does not
  invalidate an otherwise valid status envelope, and does not persist a false
  deduplication success.

Accepted help topics are exactly:

```text
status
login
config
setup
update
uninstall
doctor
help
version
```

`help <anything-else>` is a grammar error with exit `2`.

Both uninstall forms require confirmation:

- On a TTY, print `Type uninstall agent-bar to continue:` to stderr and accept
  only the exact line `uninstall agent-bar`.
- On non-TTY stdin, read exactly one JSON object followed by optional
  whitespace and EOF:

```json
{
  "schemaVersion": 1,
  "operation": "uninstall",
  "confirmed": true,
  "purgeSettingsAndBackups": false
}
```

Unknown fields, trailing non-whitespace, `confirmed: false`, wrong operation,
or a purge boolean that does not match the parsed command fails with exit `3`
before mutation. `uninstall` requires `false`; `uninstall purge` requires
`true`. QML always uses the structured non-TTY document. Standard uninstall
does not consume stdin until after its complete preflight has succeeded.

Since git-plugin-distribution (2026-08-05), bare `update` has no interactive
flow: `update apply` now applies unconditionally, so there is no longer a
specific fetched version for a TTY prompt to confirm. Bare `update` prints
usage to stderr pointing at `update check` and `update apply`, and exits `3`
without touching the network or the filesystem, on both TTY and non-TTY
stdin. The typed UI never invokes bare `update`.
