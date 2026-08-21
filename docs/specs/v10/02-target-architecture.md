# Target Architecture

Amended by the plugin-ID rename (2026-08-06):
`docs/superpowers/specs/2026-08-06-plugin-id-rename-design.md`. The plugin ID
is `othavi0.agent-bar`; it read `agent-bar.usage` when this document was
approved.

## System shape

```text
Provider CLIs
     |
     v
Private Rust provider adapters
     |
     v
Collection coordinator ---- Settings/cache/transaction modules
     |
     v
JSON schema v2 over short-lived processes
     |
     v
Quickshell Service.qml (one per shell)
     |
     +--------------------+
     |                    |
     v                    v
Monitor 1 BarWidget   Monitor 2 BarWidget
     \                    /
      \---- one logical popup state ----/
```

## Architectural requirements

- `ARCH-001`: The Rust helper is short-lived. v10 has no resident Rust daemon.
- `ARCH-002`: `Service.qml` is loaded once through the manifest `service`
  entry point.
- `ARCH-003`: Each monitor receives a lightweight `bar-widget` instance.
- `ARCH-004`: Only `Service.qml` owns polling, child-process scheduling,
  normalized provider state, settings synchronization, notification-evaluation
  requests, selected provider, and logical popup ownership.
- `ARCH-005`: Per-monitor widgets own rendering, local anchor geometry, and the
  visible popup instance only.
- `ARCH-006`: At most one agent-bar popup is visible across all monitors.
- `ARCH-007`: Provider adapters own executable discovery, official install
  URL, login argv, fetch, parsing, error classification, TTL, and timeout.
- `ARCH-008`: QML never parses raw provider output or infers state from a
  human message.
- `ARCH-009`: Settings are owned by `settings.json`; `shell.json` owns only
  plugin presence, placement, and Quattro layout.
- `ARCH-010`: Cache, settings, bundle installation, update, migration, and
  uninstall use shared locking and atomic-file primitives.
- `ARCH-011`: A single provider catalog owns ID, display name, catalog order,
  executable metadata, icon key, and official URL.
- `ARCH-012`: All production process execution uses argv arrays; no `sh -c`.
- `ARCH-013`: `Service.qml` is the only caller that requests notification
  evaluation. Rust owns the threshold, deduplication, dispatch, and persistence
  algorithm; ordinary human/recovery status commands skip it.
- `ARCH-014`: Collection discovery and login discovery are separate. A
  provider that can collect through HTTP or filesystem data does not become
  `cli_missing` merely because its interactive login executable is absent.

## Target Rust boundaries

The final names may change only through an approved checkpoint deviation. Each
module must remain a deep module with a small public surface.

```text
src/
├── main.rs
├── lib.rs
├── app_identity.rs
├── cli/
│   ├── mod.rs
│   ├── grammar.rs
│   ├── command.rs
│   └── exit.rs
├── status/
│   ├── mod.rs
│   ├── schema.rs
│   ├── collect.rs
│   ├── coordinator.rs
│   └── human.rs
├── providers/
│   ├── mod.rs
│   ├── catalog.rs
│   ├── process.rs
│   ├── claude.rs
│   ├── codex/
│   ├── amp.rs
│   └── grok.rs
├── settings/
│   ├── mod.rs
│   ├── schema.rs
│   ├── store.rs
│   └── migration.rs
├── cache/
│   ├── mod.rs
│   ├── schema.rs
│   ├── store.rs
│   └── coordinator.rs
├── notifications/
│   ├── mod.rs
│   └── state.rs
├── plugin/
│   ├── mod.rs
│   ├── paths.rs
│   ├── ownership.rs
│   ├── transaction.rs
│   ├── bundle.rs
│   ├── omarchy.rs
│   ├── doctor.rs
│   └── maintenance.rs
└── support/
    ├── atomic_file.rs
    ├── clock.rs
    ├── fs.rs
    └── redact.rs
```

Public module responsibilities:

- `cli`: parse strict word grammar and dispatch application operations.
- `status`: normalize provider results into schema v2 and coordinate cache
  generations.
- `providers`: isolate every provider-specific behavior behind one adapter
  interface.
- `settings`: validate and atomically store the canonical settings document.
- `cache`: persist normalized data and coordinate concurrent short-lived
  helper processes.
- `notifications`: decide threshold transitions and persist deduplication
  state; it does not render UI.
- `plugin`: own Omarchy paths, ownership evidence, migration, bundle
  transactions, update, doctor, and uninstall.
- `support`: narrow primitives shared by the modules above.

## Provider adapter interface

The implementation plan must define this interface before migrating provider
code:

```rust
pub trait ProviderAdapter: Send + Sync {
    fn descriptor(&self) -> &'static ProviderDescriptor;
    fn discover(&self, env: &ExecutionEnvironment) -> Discovery;
    fn login_command(&self, discovery: &Discovery) -> Result<ProcessSpec>;
    fn collect<'a>(
        &'a self,
        context: &'a CollectionContext,
        discovery: &'a Discovery,
    ) -> BoxFuture<'a, ProviderResult>;
}
```

`ProviderDescriptor` is the sole catalog entry and contains stable metadata.
`Discovery` separately reports collection availability and login-command
availability. A provider may collect without an installed login CLI; this must
not become `cli_missing`.
`CollectionContext` exposes narrow process, HTTP, filesystem, clock, and
redaction capabilities. This supports Claude HTTP collection, Grok HTTP billing
collection, Codex composite app-server/session-log collection, and Amp process
collection without forcing them into a fake command abstraction.
`ProviderResult` is a typed domain result, not serialized provider JSON.
`status::schema` is the only serialization boundary.

The descriptor shape is fixed:

```rust
pub struct ProviderDescriptor {
    pub id: ProviderId,
    pub display_name: &'static str,
    pub icon_key: &'static str,
    pub executable_name: &'static str,
    pub fallback_executable_paths: &'static [ExecutablePath],
    pub installation_url: &'static str,
    pub login_argv: &'static [&'static str],
    pub cache_ttl: Duration,
    pub timeout: Duration,
    pub max_output_bytes: usize,
    pub retry_policy: RetryPolicy,
}
```

`ExecutablePath` is a typed path template rooted in `HOME` or a
provider-specific home variable. It is not an arbitrary shell-expanded string.
Discovery checks `PATH` first, then the descriptor's fallback paths in order,
and accepts only a regular file with an executable bit. Discovery returns that
path exactly as found and never resolves symlinks: version managers such as
mise install their tools as shims that dispatch on `argv[0]`, so canonicalizing
a shim would execute the manager binary instead of the tool. The discovered
path replaces element zero of `login_argv`; every remaining element is passed
unchanged.

## Locked provider catalog

The v10 provider catalog is:

| ID | Name | Icon | Executable discovery | Login argv | Installation page |
| --- | --- | --- | --- | --- | --- |
| `claude` | Claude | `claude` | `PATH`, then `$HOME/.local/bin/claude` | `["claude", "auth", "login"]` | `https://code.claude.com/docs/en/getting-started` |
| `codex` | Codex | `codex` | `PATH`, then `$HOME/.local/bin/codex` | `["codex", "login"]` | `https://github.com/openai/codex` |
| `amp` | Amp | `amp` | `PATH`, then `$HOME/.local/bin/amp`, `$HOME/.amp/bin/amp`, `$HOME/.cache/.bun/bin/amp`, `$HOME/.bun/bin/amp` | `["amp", "login"]` | `https://ampcode.com/manual` |
| `grok` | Grok | `grok` | `PATH`, then `$GROK_HOME/bin/grok`, `$HOME/.grok/bin/grok`, `$HOME/.local/bin/grok` | `["grok", "login"]` | `https://x.ai/cli` |
| `antigravity` | Antigravity | `antigravity` | `PATH`, then `$HOME/.local/bin/agy`, `$HOME/.gemini/antigravity-cli/bin/agy` | none (login unavailable) | `https://antigravity.google` |

Locked collection policy:

| ID | Source order | Normalized window IDs | TTL | Timeout | Retry |
| --- | --- | --- | --- | --- | --- |
| `claude` | `$HOME/.claude/.credentials.json`, then authenticated `GET https://api.anthropic.com/api/oauth/usage` | `session`, `weekly`, then provider-scoped `weekly-model:<sanitized-id>` | 300 s | 10 s | one network/timeout retry |
| `codex` | resolved `codex app-server` JSON-RPC `rateLimits/read`, then newest valid rate-limit event below `$HOME/.codex/sessions` | `session`, `weekly`, then `other:<duration-minutes>:<ordinal>` | 90 s | 10 s | one app-server timeout retry before filesystem fallback |
| `amp` | resolved `amp usage` with `NO_COLOR=1`, `TERM=dumb` | `daily`, or no windows when the account exposes no percentage | 90 s | 10 s | one timeout/process-I/O retry |
| `grok` | `$GROK_HOME/auth.json`, then authenticated GET `https://cli-chat-proxy.grok.com/v1/billing?format=credits` (literal; headers Authorization Bearer + x-grok-client-mode) | `weekly` | 90 s | 10 s | one network/timeout retry |
| `antigravity` | resolved `agy --print /usage` with `NO_COLOR=1`, `TERM=dumb` | `gemini-weekly`, `gemini-5h` | 60 s | 10 s | one timeout/process-I/O retry |

Antigravity was removed once (see `CHANGELOG.md`) when it read credentials and
plan data out of a `~/.gemini` layout that no longer matches current installs.
This reintroduction is a different integration: it runs `agy`'s own `/usage`
text output through the same collection/normalization path as Amp, reads no
credential files, and reports no account or plan — a connected provider with
no plan is a valid, fully rendered state.

Collection concurrency is at most five adapters. Every process stdout, process
stderr, HTTP response, and individual provider file is capped at 1 MiB before
parsing. Codex filesystem fallback applies the same 1 MiB per-file limit, does
not follow links, descends at most eight levels, and visits at most 4096
directory entries. Codex candidate files sort by mtime descending then raw
path bytes ascending before taking 256; valid events sort by parsed UTC event
timestamp descending, then candidate path and line number.

`GROK_HOME`, when set, must be a nonempty absolute path; an invalid set value
is a typed provider configuration error. When unset, it resolves to
`$HOME/.grok`. Provider home resolution is injected and tested; it never
accepts a relative current-working-directory fallback.

One retry means one additional attempt after a 250 ms delay. It applies only
to the literal transient class in the table and only before any successful
result. Authentication, rate-limit, malformed payload, missing source, and
nonzero usage-command results are never retried. Adapter source fallback is
not counted as a retry.

Provider labels are fixed English copy: Claude `Session`/`Weekly`, Codex
`Session`/`Weekly`, Amp `Daily`, and Grok `Weekly`. Dynamic model labels are
sanitized plain text. A dynamic Claude model ID is lowercased, limited to
ASCII letters/digits/hyphens, and prefixed with `weekly-model:`; collisions
receive the deterministic source-order suffix `:2`, `:3`, and so on. Monetary
Amp lines, Claude extra usage, Codex credits, Grok session counts, raw account
email, and arbitrary provider extras are discarded before `ProviderResult`.

The table is product data, not an example:

- `ARCH-015`: Catalog order is Claude, Codex, Amp, Grok, Antigravity.
- `ARCH-016`: `view_installation` opens exactly the allowlisted page above;
  Agent Bar never opens or executes an installation script.
- `ARCH-017`: Login is always launched through the bundled terminal helper
  with the exact argv above.
- `ARCH-018`: Provider commands and URLs must be covered by literal equality
  tests so external CLI drift becomes an explicit code-review decision.
- `ARCH-019`: Collection mechanisms remain provider-specific and are not
  inferred from the login executable.
- `ARCH-020`: `Service.qml` owns one `IpcHandler` target named
  `othavi0.agent-bar` with only `health(expectedVersion)` and
  `refresh(providerId)` methods.
- `ARCH-021`: `health` returns `ok` only when the loaded manifest version and
  last verified helper version both equal `expectedVersion`. `refresh`
  validates the closed provider ID, queues one cache-bypass provider refresh,
  and returns `ok`; invalid IDs return `unknown`.
- `ARCH-022`: The locked source order, window IDs, limits, TTL, timeout, and
  retry policy above are literal contract tests.
- `ARCH-023`: `Service.qml` owns distinct process lanes for `status`,
  `versionProbe`, `settingsRead`, `settingsWrite`, `maintenanceCheck`, and
  `maintenanceHandoff`. No lane calls `exec()` while its process is running.
- `ARCH-024`: Status requests coalesce through the target-aware rules. Settings
  writes serialize. Maintenance handoff blocks new settings writes and polling;
  an already-running status or settings write drains before detached handoff,
  while read/check may finish without overwriting a newer generation.
  Update/uninstall cannot overlap each other.
- `ARCH-025`: Authenticated provider HTTP is restricted to the catalog's exact
  HTTPS origin and path, does not follow redirects, caps the streamed body
  before buffering, and never exposes authorization headers or credential
  values to diagnostics, cache, fixtures, or errors.
- `ARCH-026`: `$XDG_STATE_HOME/agent-bar/maintenance.lock` is a stable
  cross-process gate outside quarantined paths. Status and every non-maintenance
  mutation hold a shared lock; setup, migration, update, uninstall, and doctor
  clean hold the exclusive lock from final plan recheck through commit or
  verified rollback. The service stops new work and drains its mutable lanes
  before maintenance handoff.

## Target QML boundaries

```text
assets/omarchy/
├── manifest.json
├── Service.qml
├── BarWidget.qml
├── Popup.qml
├── ProviderRail.qml
├── ProviderView.qml
├── SettingsView.qml
├── MaintenanceView.qml
├── components/
│   ├── ProviderChip.qml
│   ├── ProviderHeader.qml
│   ├── UsageWindow.qml
│   ├── StateMessage.qml
│   ├── SettingsProviderRow.qml
│   ├── ConfirmDialog.qml
│   └── FocusController.qml
└── icons/
    ├── claude.png
    ├── codex.png
    ├── amp.svg
    └── grok.svg
```

- `Service.qml` owns state and commands; it contains no visual layout.
- `BarWidget.qml` implements the Quattro bar-widget protocol and click target
  registration.
- `Popup.qml` owns the monitor-local popup window and delegates content.
- Views compose focused components and contain no process execution.
- Components are presentational except for emitting typed user intentions.
- No QML file should exceed 500 lines without an approved explanation.

Quattro injection contract:

```qml
// Service.qml
property string omarchyPath: ""
property var shell: null
property var manifest: null
property var barWidgetRegistry: null
property var pluginRegistry: null

readonly property string pluginRoot:
    manifest && manifest.__sourceDir ? String(manifest.__sourceDir) : ""
```

`BarWidget.qml` receives `bar`, `moduleName`, and `settings`. It resolves the
single service with:

```qml
readonly property var agentService:
    bar && bar.shell ? bar.shell.serviceFor(moduleName) : null
```

No widget reconstructs the plugin path through `Qt.resolvedUrl()` string
replacement.

## Runtime paths

```text
Plugin bundle:
  $HOME/.config/omarchy/plugins/othavi0.agent-bar/

Settings:
  $XDG_CONFIG_HOME/agent-bar/settings.json

Cache:
  $XDG_CACHE_HOME/agent-bar/status-v2.json
  $XDG_CACHE_HOME/agent-bar/status.lock
  $XDG_CACHE_HOME/agent-bar/notification-state-v2.json

Backups:
  $XDG_STATE_HOME/agent-bar/backups/
```

Agent Bar settings/cache/state follow XDG defaults when the corresponding
variable is unset. The installed Quattro plugin root and `shell.json` use
literal `$HOME/.config/omarchy`; production must not redirect them through XDG.
Tests use an injected `HOME` (and `XDG_STATE_HOME`) and isolated XDG roots;
since git-plugin-distribution (2026-08-05), there is no `setup plugins-dir
<path>` argument to inject one instead.

## Data flow

1. `Service.qml` starts the bundled helper with
   `status format json notifications evaluate`.
2. The helper loads settings without writing them.
3. The coordinator checks cache and obtains the cross-process generation lock.
4. Required provider adapters execute concurrently with bounded concurrency.
5. Each result is normalized to typed domain data.
6. The coordinator persists a safe normalized cache and serializes schema v2.
7. `Service.qml` validates schema version and required shapes before replacing
   its immutable state snapshot.
8. Widgets receive the same state snapshot and render monitor-local views.

## Interactive login flow

Login has one exact argv-only path:

```text
Service.qml
  -> Quickshell.execDetached([
       pluginRoot + "/scripts/agent-bar-open-terminal",
       "login",
       providerId
     ])
  -> xdg-terminal-exec
       --app-id=org.omarchy.terminal
       --title=Agent Bar Login
       --
       <pluginRoot>/bin/agent-bar login <providerId>
  -> official provider login argv from the catalog
```

The Bash launcher validates exactly two arguments, derives `pluginRoot` from
`BASH_SOURCE[0]`, verifies that the private helper is a regular executable,
and `exec`s `xdg-terminal-exec` with the argv above. Omarchy's
`xdg-terminal-exec` owns terminal preference; Agent Bar has no emulator
fallback table and never invokes an intermediate shell.

On service load, the dedicated `versionProbe` lane runs the absolute private
helper as `version`, requires exact stdout `<semantic-version>\n`, empty
stderr, exit `0`, and a two-second deadline, then stores the verified version
before provider collection starts. Health depends on this probe and the
manifest, never on provider network completion.

After the official provider process exits `0`, the Rust login command performs
a best-effort argv call:

```text
omarchy-shell -q othavi0.agent-bar refresh <providerId>
```

It then returns the original provider exit status. A nonzero or signaled
provider process does not request refresh. Failure of the best-effort IPC never
changes the provider process result.

## Failure boundaries

- A provider failure becomes one provider state.
- A settings validation failure rejects the settings operation.
- A corrupt cache is isolated and rebuilt.
- A malformed helper envelope is a service-level error and never replaces the
  last valid snapshot.
- A failed plugin transaction rolls back the entire bundle and affected
  configuration.
- A QML view failure must not start additional provider processes.
