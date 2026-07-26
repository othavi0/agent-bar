# Agent Bar v10 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `superpowers:subagent-driven-development` (recommended) or
> `superpowers:executing-plans` to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Agent Bar v9 with one accessible Omarchy Quattro Quickshell
plugin backed by a private Rust helper, while deleting TUI, Waybar, history,
monetary data, and standalone distribution.

**Architecture:** Implement new contracts vertically beside v9, switch the
Quickshell consumer to schema v2, then delete the old consumers and producers.
One `Service.qml` owns runtime state; focused Rust modules own provider
collection, cache, settings, notifications, and plugin transactions.

**Tech Stack:** Rust, Tokio, Serde, Reqwest, Quickshell/QML, Omarchy Quattro,
Qt Quick Test, and narrow Bash scripts for the terminal helper, plugin
bootstrap, and verification orchestration.

## Global Constraints

- Product and active documentation language is English.
- Product ID is `agent-bar.usage`; target release is `10.0.0`.
- Product artifact is one x86_64 GNU/Linux plugin bundle.
- The Rust helper is private at `bin/agent-bar`; no global executable.
- All child processes use argv; no `sh -c`, `bash -lc`, or `eval`.
- `settings.json` is the only Agent Bar product settings source.
- Provider status uses JSON schema v2 only.
- Quattro manifest schema remains version 1.
- No TUI, Waybar, history, chart, monetary, AUR, cargo-binstall, or standalone
  compatibility layer survives.
- No provider CLI, credential, or system package is installed automatically.
- No production `unwrap()` or `expect()`.
- Every behavior change starts with a failing test.
- Run focused tests after each step and the full gate at each checkpoint.
- Do not mutate the live desktop before checkpoint 4 approval.
- Do not bypass hooks, force-push, merge, tag, or publish.

---

## Locked target file map

Create:

```text
schemas/status-v2.schema.json
schemas/settings-v1.schema.json
src/cli/{mod.rs,grammar.rs,command.rs,exit.rs}
src/status/{mod.rs,schema.rs,collect.rs,coordinator.rs,human.rs}
src/providers/{catalog.rs,process.rs}
src/settings/{mod.rs,schema.rs,store.rs,migration.rs}
src/cache/{mod.rs,schema.rs,store.rs,coordinator.rs}
src/notifications/{mod.rs,state.rs}
src/plugin/{mod.rs,paths.rs,ownership.rs,transaction.rs,bundle.rs,omarchy.rs,doctor.rs,maintenance.rs}
src/support/{mod.rs,atomic_file.rs,clock.rs,fs.rs,redact.rs}
src/bin/agent-bar-bundle.rs
assets/omarchy/{Service.qml,BarWidget.qml,Popup.qml,ProviderRail.qml,ProviderView.qml,SettingsView.qml,MaintenanceView.qml}
assets/omarchy/components/{ProviderChip.qml,ProviderHeader.qml,UsageWindow.qml,StateMessage.qml,SettingsProviderRow.qml,ConfirmDialog.qml,FocusController.qml}
tests/qml/**
tests/fixtures/status-v2/**
scripts/verify-v10-ui
```

Rewrite:

```text
src/{main.rs,lib.rs,app_identity.rs}
src/providers/{mod.rs,claude.rs,amp.rs,grok.rs}
src/providers/codex/**
assets/omarchy/manifest.json
scripts/agent-bar-open-terminal
install.sh
.github/workflows/publish.yml
```

Delete only after replacement consumers pass:

```text
src/action_right.rs
src/tui/**
src/usage/**
src/waybar/**
src/formatters/**
src/cache.rs
src/cli.rs
src/config.rs
src/watch.rs
src/theme.rs
src/platform.rs
src/runtime.rs
src/term_prompt.rs
src/config_cmd.rs
src/http.rs
src/install.rs
src/logger.rs
src/notify.rs
src/omarchy_integration.rs
src/settings.rs
src/test_support.rs
src/setup.rs
src/update.rs
src/uninstall.rs
src/doctor.rs
src/providers/amp_cli.rs
src/providers/base.rs
src/providers/error.rs
src/providers/extras.rs
src/providers/grok_cli.rs
src/providers/types.rs
assets/omarchy/Widget.qml
src/snapshots/agent_bar__omarchy_integration__tests__omarchy_manifest.snap
tests/golden.rs
docs/assets/agent-bar-banner.png
docs/waybar-contract.md
icons/amp-icon.svg
icons/claude-code-icon.png
icons/codex-icon.png
icons/grok-icon.svg
packaging/aur/.SRCINFO
packaging/aur/PKGBUILD
packaging/aur/agent-bar-bin.install
build.rs
```

Retain and rewrite, not delete:

```text
scripts/agent-bar-open-terminal
install.sh
scripts/check-version
tests/fixtures/amp/**
tests/fixtures/grok/**
```

## Checkpoint 1 — Backend contract

### Task 1: Freeze schemas and test seams

**Files:**

- Create: `schemas/status-v2.schema.json`
- Create: `schemas/settings-v1.schema.json`
- Create: `src/support/mod.rs`
- Create: `src/support/clock.rs`
- Create: `src/support/fs.rs`
- Modify: `src/lib.rs`
- Modify: `Cargo.toml`
- Regenerate: `Cargo.lock`
- Test: `tests/fixtures/status-v2/*.json`

**Interfaces:**

- Produces: `Clock`, `SystemClock`, `FileSystem`, `RealFileSystem`.
- Produces: checked-in structural JSON Schemas consumed by Rust and
  documentation tests. Cross-field semantic validation belongs to Task 3.

- [ ] **Step 1: Add failing schema fixture tests**

Create tests that load both schemas and assert:

```rust
assert!(validate("schemas/status-v2.schema.json", "tests/fixtures/status-v2/ready.json"));
assert!(!validate("schemas/status-v2.schema.json", "tests/fixtures/status-v2/percent-over-100.json"));
assert!(!validate("schemas/status-v2.schema.json", "tests/fixtures/status-v2/money-field.json"));
```

- [ ] **Step 2: Run the focused test and observe failure**

Run: `cargo test schema_contract -- --nocapture`

Expected: failure because schemas and fixtures do not exist.

- [ ] **Step 3: Add exact schemas and deterministic seams**

The status schema must encode every structural and state-shape `JSON-*`
invariant it can express. It must not pretend to prove arithmetic sums,
cross-array uniqueness/order, or runtime version equality; Task 3 implements
those checks in Rust. The settings schema must reject unknown keys and enforce
the provider item shape; its semantic validator proves exact once-only
membership. Add the Rust `jsonschema` crate as a dev dependency with default
features disabled, lock it in `Cargo.lock`, declare
`"$schema": "https://json-schema.org/draft/2020-12/schema"` in both
self-contained schemas, and compile them with `jsonschema::validator_for`.
This avoids HTTP/file reference resolution and a partial home-grown validator.
Add:

```rust
pub trait Clock: Send + Sync {
    fn now_utc(&self) -> time::OffsetDateTime;
}

pub trait FileSystem: Send + Sync {
    fn read(&self, path: &Path) -> io::Result<Vec<u8>>;
    fn metadata(&self, path: &Path) -> io::Result<FileMetadata>;
}
```

- [ ] **Step 4: Run the focused test**

Run: `cargo test schema_contract -- --nocapture`

Expected: all valid fixtures pass and every invalid fixture fails.

- [ ] **Step 5: Commit**

```bash
git add schemas tests/fixtures/status-v2 src/support src/lib.rs Cargo.toml Cargo.lock
git commit -m "test: define v10 schema contracts"
```

### Task 2: Implement the strict word-based CLI

**Files:**

- Create: `src/cli/mod.rs`
- Create: `src/cli/grammar.rs`
- Create: `src/cli/command.rs`
- Create: `src/cli/exit.rs`
- Modify: `src/main.rs`
- Rewrite: `tests/cli.rs`
- Modify: `Cargo.toml`
- Regenerate: `Cargo.lock`

**Interfaces:**

- Produces:

```rust
pub struct StatusOptions {
    pub format: StatusFormat,
    pub provider: Option<ProviderId>,
    pub cache: CacheMode,
    pub notifications: NotificationMode,
}

pub enum ConfigCommand {
    Show,
    Apply(ConfigInput),
}

pub enum ConfigInput {
    Stdin,
    File(PathBuf),
    Json(String),
}

pub enum SetupOptions {
    Production,
    PluginsDir(PathBuf),
}

pub struct ReleaseVersion(semver::Version);

pub enum UpdateCommand {
    Interactive,
    Check,
    Apply(ReleaseVersion),
}

pub enum DoctorCommand {
    Scan,
    Clean,
}

pub enum Command {
    Status(StatusOptions),
    Login(ProviderId),
    Config(ConfigCommand),
    Setup(SetupOptions),
    Update(UpdateCommand),
    Uninstall { purge: bool },
    Doctor(DoctorCommand),
    Help(Option<HelpTopic>),
    Version,
}
```

- [ ] **Step 1: Replace CLI tests with an exhaustive grammar table**

Generate all 24 permutations of `format`, `provider`, `cache`, and
`notifications`; add rejection cases for duplicates, missing values, v9
commands, and every unsupported `--flag`. Table-test every non-status form,
including all config input variants, production/test setup, update
interactive/check/apply, standard/purge uninstall, doctor scan/clean,
every accepted/rejected help topic, and both accepted double-dash aliases.
Assert `version` produces exact package semver plus newline with no discovery,
filesystem, provider, or stderr activity.
Cover setup parent-versus-plugin-root validation and bare update TTY,
no-update, exact phrase, EOF, rejection, and non-TTY behavior.

- [ ] **Step 2: Verify the old parser fails the new contract**

Run: `cargo test --test cli -- --nocapture`

Expected: new word grammar cases fail and legacy commands still pass.

- [ ] **Step 3: Implement a single-pass clause parser**

Track seen clause kinds in an enum set. Parse no implicit abbreviations. Map
grammar failures to exit `2`; never call provider or filesystem code during
parsing. Add `semver` as a direct dependency and reject versions that are not
strict semantic versions.

- [ ] **Step 4: Verify grammar and stdout/stderr separation**

Run:

```bash
cargo test --test cli -- --nocapture
cargo test cli:: -- --nocapture
```

Expected: all grammar, alias, exit-code, and help tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/cli src/main.rs tests/cli.rs Cargo.toml Cargo.lock
git commit -m "feat: add v10 CLI grammar"
```

### Task 3: Implement schema-v2 domain serialization

**Files:**

- Create: `src/status/mod.rs`
- Create: `src/status/schema.rs`
- Create: `src/status/collect.rs`
- Create: `src/status/human.rs`
- Modify: `src/lib.rs`
- Test: `src/status/schema.rs`
- Test: `tests/fixtures/status-v2/*.json`

**Interfaces:**

- Produces: `StatusEnvelope`, `ProviderStatus`, `ProviderState`,
  `UsageWindow`, `ProviderError`, `ProviderAction`.
- Produces: `StatusEnvelope::validate_semantics()` covering every invariant
  assigned to Rust in `03-cli-and-json-contract.md`.
- Consumes: `ProviderResult` introduced as a temporary test type in this task;
  Task 6 moves its owner to `providers`.

- [ ] **Step 1: Add failing invariant and serialization tests**

Cover the complete state truth table, partial failures, empty windows, stale
data, unknown action, invalid percent, non-finite number, percentage sum,
duplicate provider/window IDs, request ordering, UTC reset, helper/package
version equality, no money fields, and forced serializer failure. Assert
successful status JSON is exactly one object plus newline.

- [ ] **Step 2: Run and observe failure**

Run: `cargo test status::schema -- --nocapture`

Expected: failure because schema-v2 types do not exist.

- [ ] **Step 3: Implement closed enums and validated constructors**

Serialization must be possible only after constructors enforce:

```rust
0.0 <= used && used <= 100.0
(used + remaining - 100.0).abs() <= 0.01
resets_at.map(|value| value.offset() == time::UtcOffset::UTC)
```

Do not expose public struct fields that allow invalid percentages.

- [ ] **Step 4: Run schema and fixture tests**

Run: `cargo test status:: -- --nocapture`

Expected: all schema-v2 tests pass; serializer errors produce exit `4` and
nonempty stderr.

- [ ] **Step 5: Commit**

```bash
git add src/status src/lib.rs tests/fixtures/status-v2
git commit -m "feat: add status schema v2"
```

### Task 4: Build the canonical settings store

**Files:**

- Create: `src/settings/mod.rs`
- Create: `src/settings/schema.rs`
- Create: `src/settings/store.rs`
- Create: `src/support/atomic_file.rs`
- Create: `src/support/maintenance_gate.rs`
- Modify: `src/cli/command.rs`
- Modify: `Cargo.toml`
- Regenerate: `Cargo.lock`
- Test: `src/settings/store.rs`

**Interfaces:**

- Produces: `Settings`, `ProviderSetting`, `DisplayMetric`,
  `SettingsStore::show`, and `SettingsStore::apply`.
- Produces:

```rust
pub fn replace_atomically(
    target: &Path,
    bytes: &[u8],
    mode: u32,
) -> Result<()>;
```

- [ ] **Step 1: Add failing purity, validation, and atomicity tests**

Tests must assert missing-show does not create a file, existing-show preserves
bytes and mtime, unknown keys fail before lock, valid writes are mode `0600`,
and injected write/fsync/rename failures preserve previous bytes.
Also prove apply holds a shared `MaintenanceGate` and blocks behind an
exclusive maintenance holder without touching settings first.

- [ ] **Step 2: Run focused tests**

Run: `cargo test settings:: -- --nocapture`

Expected: failures against the mutating v9 settings loader.

- [ ] **Step 3: Implement strict settings and atomic replacement**

Use a complete-document apply. Do not deserialize through a map that discards
unknown keys. Keep read, validate, lock, and write as separate phases.

- [ ] **Step 4: Run settings and CLI config tests**

Run:

```bash
cargo test settings:: -- --nocapture
cargo test config_ -- --nocapture
```

Expected: all settings tests pass and apply returns canonical JSON.

- [ ] **Step 5: Commit**

```bash
git add src/settings src/support src/cli Cargo.toml Cargo.lock
git commit -m "feat: add canonical settings store"
```

### Task 5: Add safe process execution and provider catalog

**Files:**

- Create: `src/providers/catalog.rs`
- Create: `src/providers/process.rs`
- Create: `src/support/redact.rs`
- Modify: `src/providers/mod.rs`
- Test: `src/providers/catalog.rs`
- Test: `src/providers/process.rs`

**Interfaces:**

- Produces: `ProviderId`, `ProviderDescriptor`, `Discovery`,
  `CollectionAvailability`, `LoginAvailability`, `ProcessSpec`,
  `ProcessRunner`, and `ProcessOutput`.
- Consumes: Tokio process/time and injected environment.

- [ ] **Step 1: Add failing catalog and process-runner tests**

Assert the exact descriptor table in `02-target-architecture.md`: order,
display names, icon keys, executable names and fallback paths, official HTTPS
pages, and literal login argv. Also cover executable permission checks,
separate collection/login availability, argv preservation, timeout kill/reap,
output limits, ANSI/control redaction, and no shell process.

- [ ] **Step 2: Run focused tests**

Run:

```bash
cargo test providers::catalog -- --nocapture
cargo test providers::process -- --nocapture
```

Expected: failure because the canonical catalog and runner do not exist.

- [ ] **Step 3: Implement the catalog and runner**

The descriptor contains metadata only. Provider behavior remains in adapters.
Use the exact `ProviderDescriptor` and typed path-template contract from
`02-target-architecture.md`. Reject executable files without an execute bit.
Never join argv into a string. The `view_installation` action must expose only
the descriptor's allowlisted documentation page, never an installer URL.

- [ ] **Step 4: Run focused tests and Clippy**

Run:

```bash
cargo test providers::catalog -- --nocapture
cargo test providers::process -- --nocapture
cargo clippy --all-targets -- -D warnings
```

Expected: pass with no warning and no shell invocation.

- [ ] **Step 5: Commit**

```bash
git add src/providers src/support/redact.rs
git commit -m "feat: centralize providers and processes"
```

### Task 6: Migrate provider adapters to the v2 domain

**Files:**

- Modify: `src/providers/mod.rs`
- Modify: `src/providers/claude.rs`
- Modify: `src/providers/amp.rs`
- Modify: `src/providers/grok.rs`
- Modify: `src/providers/codex/**`
- Modify: `src/cli/**`
- Modify: `src/main.rs`
- Create: provider fixtures under `tests/fixtures/providers/**`
- Create: `tests/login.rs`
- Test: provider modules

**Interfaces:**

- Produces:

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

- Consumes: process, HTTP, filesystem, clock, and redaction capabilities from
  `CollectionContext`.
- Produces: typed `ProviderResult`; adapters never serialize schema v2.
- Produces: login exit-status forwarding and best-effort successful-login
  refresh through the exact Agent Bar IPC argv.

- [ ] **Step 1: Add complete provider fixture tests**

For each provider cover ready, empty percentage windows, missing collection
source, unauthenticated, rate limit, network, malformed output, and timeout.
Add the known Claude token-expired, unknown-limits fallback, and double-division
regressions. Assert no spend/credits are present in `ProviderResult`. Claude
HTTP fixtures also prove exact HTTPS origin/path, redirect refusal, body-limit
enforcement before buffering, and complete authorization-value redaction.
Codex/Grok filesystem fixtures prove absolute provider-home resolution,
no-link/depth/entry limits, mtime/path/event/line tie-breaking, and the
candidate cap.
In `tests/login.rs`, inject the provider and IPC runners and assert exact login
argv, exact successful refresh argv, no refresh after nonzero/signal
termination, and the exit-code mapping in `CLI-017`.

- [ ] **Step 2: Run provider tests against v9**

Run: `cargo test providers:: -- --nocapture`

Expected: failures for typed errors, order, empty windows, token cache, and
single/all policy.

- [ ] **Step 3: Migrate one provider at a time**

Order: Amp, Grok, Codex, Claude. Reuse parsers only after adapting their output
to v2 domain types. Delete `ExtraUsage`, `-1` sentinels, UI getters, and human
message classification as each consumer disappears.

Wire `login <provider>` through the closed provider catalog. After an official
login exits `0`, invoke the best-effort refresh exactly as specified in
`02-target-architecture.md`; refresh failure is a stderr diagnostic and does
not replace the successful provider status. Never request refresh after a
nonzero, signaled, or invalid platform status.

- [ ] **Step 4: Run each provider filter, then the provider suite**

Run:

```bash
cargo test providers::amp -- --nocapture
cargo test providers::grok -- --nocapture
cargo test providers::codex -- --nocapture
cargo test providers::claude -- --nocapture
cargo test providers:: -- --nocapture
cargo test --test login -- --nocapture
```

Expected: all fixtures pass with identical single/all normalization.

- [ ] **Step 5: Commit**

```bash
git add src/providers src/cli src/main.rs tests/login.rs tests/fixtures/providers tests/fixtures/amp tests/fixtures/grok
git commit -m "refactor: migrate providers to v2"
```

### Task 7: Implement cache coordination and notifications

**Files:**

- Create: `src/cache/mod.rs`
- Create: `src/cache/schema.rs`
- Create: `src/cache/store.rs`
- Create: `src/cache/coordinator.rs`
- Create: `src/status/coordinator.rs`
- Create: `src/notifications/mod.rs`
- Create: `src/notifications/state.rs`
- Modify: `src/support/maintenance_gate.rs`
- Modify: `src/main.rs`
- Test: cache, status coordinator, and notification modules

**Interfaces:**

- Produces: `StatusCoordinator::collect(StatusRequest)`.
- Produces: `NotificationEvaluator::evaluate(&StatusEnvelope, &Settings)`.
- Consumes: `CacheMode`, `NotificationMode`, provider adapters, clock, stores,
  process runner, and native notification runner.

- [ ] **Step 1: Add failing concurrency and transition tests**

Use barriers and a fake clock to prove cache expiry at `now >= expiresAt`,
cache-bypass writes live data, cache-use singleflight, force-during-active
requires a later-started generation, shared-service target union with `all`
dominance, disjoint external target serialization, sibling-preserving
single-provider cache merge, corrupt cache quarantine, partial stale,
notification warning/critical escalation, `notification.lock` serialization,
provider/window/nullable-reset ordering and rearm, exact `notify-send` argv,
five-second timeout, per-success atomic acknowledgement, crash-window replay,
corrupt-state quarantine, no false persistence or later dispatch after a
dispatch failure, shared-lock blocking behind an injected exclusive maintenance
holder, and default `notifications skip`.

- [ ] **Step 2: Run the focused suites**

Run:

```bash
cargo test cache:: -- --nocapture
cargo test status::coordinator -- --nocapture
cargo test notifications:: -- --nocapture
```

Expected: failures against v9 cache and Waybar-only notification wiring.

- [ ] **Step 3: Implement lock, generations, fan-out, and evaluation**

Use `fs2` for the cross-process lock. Record request and completion timestamps
with the injected clock. Only `notifications evaluate` dispatches; failure is a
stderr diagnostic and never invalidates valid status JSON. Implement the
literal at-least-once algorithm, state document, transition ordering, timeout,
and argv from `05-settings-cache-and-notifications.md`; do not claim
exactly-once delivery. Hold the stable shared maintenance gate across every
cache/notification mutation.

- [ ] **Step 4: Run checkpoint-1 gates**

Run:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Expected: all backend contracts pass. Legacy UI may still exist but must not be
the tested v10 consumer.

- [ ] **Step 5: Commit and stop for checkpoint 1**

```bash
git add src/cache src/status src/notifications src/support src/main.rs Cargo.toml Cargo.lock
git commit -m "feat: coordinate cache and notifications"
```

Write `/tmp/agent-bar-v10-checkpoint-01.md`, push the stable branch, and stop.

## Checkpoint 2 — Quickshell plugin

### Task 8: Assemble a testable service-plus-widget plugin skeleton

**Files:**

- Rewrite: `assets/omarchy/manifest.json`
- Create: `assets/omarchy/Service.qml`
- Create: `assets/omarchy/BarWidget.qml`
- Create: `tests/qml/tst_Service.qml`
- Create: `tests/qml/tst_BarWidget.qml`
- Create: `tests/qml/fixtures/fake-agent-bar`
- Modify: `.github/workflows/publish.yml`

**Interfaces:**

- Produces: Quattro service `agent-bar.usage`.
- Produces: `BarWidget.qml::agentService` resolved with
  `bar.shell.serviceFor(moduleName)`.
- Produces: one `agent-bar.usage` IPC target with the exact health/refresh
  contract.
- Consumes: fake helper path injection in the QML test harness only.

- [ ] **Step 1: Add failing manifest and singleton tests**

Assert exact manifest JSON, empty inline settings schema, one service instance,
two widget instances, both widgets resolving the same service object, one IPC
target, `helperVersion`/manifest/expected-version health equality, valid
provider refresh, invalid provider rejection, and a cold-start two-second
version probe that completes before deliberately slow provider collection.

- [ ] **Step 2: Run validation and QML tests**

Run:

```bash
omarchy plugin validate assets/omarchy
QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
```

Expected: validation/test failure because new entry points do not exist.

- [ ] **Step 3: Add the exact manifest and minimal components**

Declare injected service properties exactly:

```qml
property string omarchyPath: ""
property var shell: null
property var manifest: null
property var barWidgetRegistry: null
property var pluginRegistry: null
readonly property string pluginRoot:
    manifest && manifest.__sourceDir ? String(manifest.__sourceDir) : ""
```

Do not use `activation`, `keepLoaded`, inline product defaults, or schema.

- [ ] **Step 4: Run validation, QML tests, and lint**

Run:

```bash
omarchy plugin validate assets/omarchy
find assets/omarchy -type f -name '*.qml' -exec \
  qmllint -I /usr/share/omarchy/shell {} +
QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
```

Expected: manifest, singleton, and lint gates pass.

- [ ] **Step 5: Commit**

```bash
git add assets/omarchy tests/qml .github/workflows/publish.yml
git commit -m "feat: add Quickshell plugin skeleton"
```

### Task 9: Implement the shared Quickshell service

**Files:**

- Modify: `assets/omarchy/Service.qml`
- Test: `tests/qml/tst_Service.qml`
- Test: `tests/qml/tst_ServiceRaces.qml`

**Interfaces:**

- Produces service properties: `snapshot`, `refreshing`, `selectedProviderId`,
  `popupOwner`, `settingsState`, `settingsDraft`, `maintenanceState`, and
  `pendingForcedTargets`.
- Produces methods:

```qml
function refreshAll(force)
function refreshProvider(providerId, force)
function requestPopup(owner, providerId, view)
function closePopup(owner)
function openSettings(owner)
```

- [ ] **Step 1: Add failing process and ownership tests**

Assert one automatic timer, one active status process, immutable snapshot
replacement, malformed-envelope retention, target-aware pending forced union,
provider refresh, same-owner close, provider switch, cross-monitor owner
transfer, and overlapping status/settings-read/settings-write/update-check
without any same-lane `exec()` cancellation. Maintenance handoff stops polling,
rejects new writes, and waits for active mutable status/settings-write lanes to
drain before starting the detached worker.

- [ ] **Step 2: Run the service tests**

Run: `QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt`

Expected: service lifecycle and race tests fail.

- [ ] **Step 3: Implement one process owner and generation tracking**

Invoke:

```qml
[helperPath, "status", "format", "json", "cache", cacheMode,
 "notifications", "evaluate"]
```

Use a request generation and immutable local request object. Validate
`schemaVersion === 2`, `helperVersion`, provider array shape, IDs, percentages,
and action kinds before replacing the last snapshot.

Declare separate `Process` objects for `status`, `versionProbe`, `settingsRead`,
`settingsWrite`, `maintenanceCheck`, and `maintenanceHandoff`. Implement the
serialization, maintenance blocking, and stale-callback rules from
`ARCH-023`/`ARCH-024`. Never reuse a running `Process.exec()` lane as implicit
cancellation.

- [ ] **Step 4: Run service tests and QML lint**

Run:

```bash
QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
qmllint -I /usr/share/omarchy/shell assets/omarchy/Service.qml
```

Expected: one active status process, isolated service-owned process lanes,
coalescing, and popup ownership tests pass.

- [ ] **Step 5: Commit**

```bash
git add assets/omarchy/Service.qml tests/qml
git commit -m "feat: share shell state and polling"
```

### Task 10: Implement provider chips and Quattro click routing

**Files:**

- Modify: `assets/omarchy/BarWidget.qml`
- Create: `assets/omarchy/components/ProviderChip.qml`
- Copy: `icons/*` to the approved `assets/omarchy/icons/*` source locations
- Test: `tests/qml/tst_BarWidget.qml`

**Interfaces:**

- Consumes: shared service snapshot and popup methods.
- Produces: one click target per visible chip and `triggerPress(button)`.

- [ ] **Step 1: Add failing chip tests**

Cover configured order, enabled filtering, `—` for empty windows, used versus
remaining, ready/stale/error text cues, tooltip copy, left/middle/right
routing, registration, unregistration, and wheel no-op.

- [ ] **Step 2: Run the widget tests**

Run: `QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt`

Expected: chip and Quattro routing tests fail.

- [ ] **Step 3: Implement lightweight chips**

Views must not create `Process` or polling `Timer`. Register each chip with
`bar.registerClickTarget()` and unregister on destruction. Map mouse buttons to
typed service intentions; do not install a wheel handler.

- [ ] **Step 4: Run QML tests and source guard**

Run:

```bash
QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
! rg -n 'Process|Timer|bash -lc|sh -c' \
  assets/omarchy/BarWidget.qml assets/omarchy/components/ProviderChip.qml
```

Expected: tests pass and source guard finds no forbidden owner.

- [ ] **Step 5: Commit**

```bash
git add assets/omarchy/BarWidget.qml assets/omarchy/components/ProviderChip.qml assets/omarchy/icons tests/qml
git commit -m "feat: add native provider chips"
```

### Task 11: Build the popup, rail, header, and provider states

**Files:**

- Create: `assets/omarchy/Popup.qml`
- Create: `assets/omarchy/ProviderRail.qml`
- Create: `assets/omarchy/ProviderView.qml`
- Create: `assets/omarchy/components/ProviderHeader.qml`
- Create: `assets/omarchy/components/UsageWindow.qml`
- Create: `assets/omarchy/components/StateMessage.qml`
- Test: `tests/qml/tst_Popup.qml`
- Test: `tests/qml/tst_ProviderStates.qml`

**Interfaces:**

- Consumes: selected provider and service actions.
- Produces: monitor-local `KeyboardPanel` with one provider view.

- [ ] **Step 1: Add failing layout and state tests**

Assert icon-only rail, Settings at bottom, no duplicate header icon,
full-width separators, plan/connection/update labels, every provider state,
plain-text error rendering, allowlisted actions, stale retention, and no money
copy.

- [ ] **Step 2: Run popup tests**

Run: `QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt`

Expected: layout and state cases fail.

- [ ] **Step 3: Compose the popup from focused components**

Use `KeyboardPanel` fitting helpers. Provider header has no provider icon.
Action components emit `retry`, `login`, or `viewInstallation`; the service
maps them to argv or an allowlisted URL.

- [ ] **Step 4: Run QML tests and plain-text scan**

Run:

```bash
QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
! rg -n 'Text\\.RichText|RichText|innerHTML' assets/omarchy
```

Expected: popup/state tests pass and rich-text scan is empty.

- [ ] **Step 5: Commit**

```bash
git add assets/omarchy tests/qml
git commit -m "feat: add consolidated provider popup"
```

### Task 12: Implement race-safe Settings

**Files:**

- Create: `assets/omarchy/SettingsView.qml`
- Create: `assets/omarchy/components/SettingsProviderRow.qml`
- Modify: `assets/omarchy/Service.qml`
- Test: `tests/qml/tst_Settings.qml`
- Test: `tests/qml/tst_SettingsRaces.qml`

**Interfaces:**

- Consumes: `config show` and `config apply stdin`.
- Produces: immutable persisted snapshot, mutable draft, and request generation.

- [ ] **Step 1: Add failing Settings and race tests**

Cover loading lockout, provider toggles/order, used/remaining, numeric bounds,
notifications, restore-draft-only, cancel, invalid-save disabled, delayed show,
close during save, reopen during save, two saves, and stale callbacks.

- [ ] **Step 2: Run Settings tests**

Run: `QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt`

Expected: state-machine and race cases fail.

- [ ] **Step 3: Implement immutable request snapshots**

Capture payload and generation before starting a process. A callback updates
state only when its generation matches. Closing changes visibility, never
`busy`. Use native controls and English text buttons.

- [ ] **Step 4: Run Settings tests and lint**

Run:

```bash
QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
qmllint -I /usr/share/omarchy/shell assets/omarchy/SettingsView.qml
```

Expected: all Settings and race tests pass.

- [ ] **Step 5: Commit**

```bash
git add assets/omarchy/SettingsView.qml assets/omarchy/components/SettingsProviderRow.qml assets/omarchy/Service.qml tests/qml
git commit -m "feat: add transactional settings UI"
```

### Task 13: Add Maintenance UI and argv-safe login

**Files:**

- Create: `assets/omarchy/MaintenanceView.qml`
- Create: `assets/omarchy/components/ConfirmDialog.qml`
- Modify: `assets/omarchy/Service.qml`
- Rewrite: `scripts/agent-bar-open-terminal`
- Test: `tests/qml/tst_Maintenance.qml`
- Test: shell helper tests in `tests/terminal_helper.rs`

**Interfaces:**

- Consumes: `update check`, `update apply <version>`, `uninstall`, and
  `uninstall purge`.
- Produces: confirmed typed maintenance requests.

- [ ] **Step 1: Add failing update, uninstall, and helper tests**

Assert explicit check, compatible-version confirmation, release-notes target,
settings-preserved default, purge checkbox, second destructive click, argv
arrays, provider allowlist, plugin-root-derived absolute private-helper path,
exact `xdg-terminal-exec` argv, no emulator fallback table, successful-login
refresh IPC integration, nonzero-login suppression, exact uninstall
confirmation JSON, and preservation of provider exit status. Rust login
semantics and exact IPC argv are owned and exhaustively tested by Task 6; this
task verifies the rewritten shell helper and QML route into that contract.

- [ ] **Step 2: Run focused tests**

Run:

```bash
cargo test terminal_helper -- --nocapture
QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
```

Expected: old `cmd="$*"`/`bash -lc` helper and missing UI fail.

- [ ] **Step 3: Implement typed actions**

The helper accepts exactly `login <provider>`, derives plugin root from
`BASH_SOURCE[0]`, validates the private executable, and `exec`s the exact
`xdg-terminal-exec` argv in `02-target-architecture.md`. It has no terminal
emulator detection or shell-string fallback. QML uses
`Quickshell.execDetached([...])` only for login. Preserve Task 6's Rust
successful-login refresh behavior unchanged. Maintenance is delegated to the
transaction worker defined in checkpoint 3.

- [ ] **Step 4: Run helper, QML, and shell checks**

Run:

```bash
cargo test terminal_helper -- --nocapture
shellcheck scripts/agent-bar-open-terminal
! rg -n 'cmd=\"\\$\\*\"|sh -c|bash -lc|eval|command -v agent-bar|alacritty|kitty|foot|ghostty|wezterm' scripts/agent-bar-open-terminal
QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
```

Expected: all checks pass.

- [ ] **Step 5: Commit**

```bash
git add assets/omarchy scripts/agent-bar-open-terminal tests
git commit -m "feat: add safe maintenance and login"
```

### Task 14: Finish accessibility, scroll, theme, and screenshots

**Files:**

- Modify: all new QML components
- Create: `assets/omarchy/components/FocusController.qml`
- Test: `tests/qml/tst_Keyboard.qml`
- Test: `tests/qml/tst_Scroll.qml`
- Test: `tests/qml/tst_Accessibility.qml`
- Create: `tests/qml/tst_Screenshots.qml`
- Create: `scripts/verify-v10-ui`

**Interfaces:**

- Consumes: Quattro `KeyboardPanel`, `PanelKeyCatcher`, `Color`, `Style`, and
  native controls.
- Produces: the complete `UX-*` and `A11Y-*` contract.

- [ ] **Step 1: Add failing keyboard, scroll, and accessibility tests**

Cover every required key, editor suppression, visible focus, accessible roles,
focus scrolling, positive/negative wheel, vertical-only movement, stop bounds,
short-content clamp, scrollbar, absence of Agent Bar-authored animations,
light/dark theme, ordered focus-controller activation, and the required
screenshot states.

- [ ] **Step 2: Run the QML suite**

Run: `QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt`

Expected: accessibility and scrolling tests fail until behavior is complete.

- [ ] **Step 3: Implement native behavior and remove custom icon controls**

Use:

```qml
Flickable {
    id: flick
    contentWidth: width
    contentHeight: contentColumn.implicitHeight
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    flickableDirection: Flickable.VerticalFlick
    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
}
```

Use refresh `󰑐`, settings `󰒓`, native chevrons, and text labels for ambiguous
actions. Delete the v9 custom `IconButton`. Implement the exact
`PanelKeyCatcher`/`FocusController` and PageUp/PageDown/Home/End routing in
`04-quickshell-ux-and-accessibility.md`. Add no plugin-authored `Behavior`,
`Transition`, or animation.

- [ ] **Step 4: Run checkpoint-2 gates**

Run:

```bash
find assets/omarchy -type f -name '*.qml' -exec \
  qmllint -I /usr/share/omarchy/shell {} +
omarchy plugin validate assets/omarchy
QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
scripts/verify-v10-ui
! rg -n '\b(Behavior|Transition)\b|\b(?:[A-Z][A-Za-z]*)?(?:Animation|Animator)\b' \
  assets/omarchy --glob '*.qml'
cargo test
cargo clippy --all-targets -- -D warnings
```

`scripts/verify-v10-ui` recreates `target/v10-ui-evidence/`, runs only
`tst_Screenshots.qml` with deterministic fixture/theme inputs, requires every
PNG named in `07-testing-and-acceptance.md` to be nonempty, and writes sorted
`SHA256SUMS`. It exits nonzero for a missing/extra/empty image.

Expected: QML behavior, lint, Rust suite, animation-absence, and screenshot
capture pass.

- [ ] **Step 5: Commit and stop for checkpoint 2**

```bash
git add assets/omarchy tests/qml scripts/verify-v10-ui
git commit -m "feat: complete plugin accessibility"
```

Write `/tmp/agent-bar-v10-checkpoint-02.md`, push the stable branch, and stop.

## Checkpoint 3 — Migration, bundle, and legacy deletion

### Task 15: Implement paths, ownership, and transaction primitives

**Files:**

- Create: `src/plugin/mod.rs`
- Create: `src/plugin/paths.rs`
- Create: `src/plugin/ownership.rs`
- Create: `src/plugin/transaction.rs`
- Modify: `src/support/fs.rs`
- Modify: `Cargo.toml`
- Test: plugin path, ownership, and transaction modules

**Interfaces:**

- Produces: `PluginPaths`, `Ownership`, `OwnershipEvidence`,
  `TransactionPlan`, `TransactionJournal`, `Transaction`, and the exclusive
  side of `MaintenanceGate`.
- Produces same-filesystem exchange using `renameat2(RENAME_EXCHANGE)`.
- Consumes: atomic file, clock, and filesystem seams.

- [ ] **Step 1: Add failing path, ownership, and fault-injection tests**

Assert literal `$HOME/.config/omarchy/plugins`, injected test plugin root,
canonical-path/symlink rejection, all five ownership classes, before hashes,
mode capture, backup outside target, failure at every transaction step,
byte-for-byte rollback, destination-local hidden sibling paths across simulated
mount IDs, complete staged manifests ignored by the Quattro discovery glob,
unsupported exchange failure before mutation, and external status/config
writers unable to recreate quarantined state while maintenance is exclusive.

- [ ] **Step 2: Run focused tests**

Run:

```bash
cargo test plugin::paths -- --nocapture
cargo test plugin::ownership -- --nocapture
cargo test plugin::transaction -- --nocapture
```

Expected: failures because transaction primitives do not exist.

- [ ] **Step 3: Implement narrow primitives**

Use only these required direct crates for the transaction layer:

```text
fs2      cross-process advisory lock (introduced by Task 4)
rustix   renameat2, modes, and filesystem sync
sha2     bundle and ownership hashes
tar      inspected archive entries
zstd     .tar.zst decoder
```

Reject absolute paths, `..`, symlinks, hardlinks, devices, FIFOs, sockets, and
inventory mismatches before writing extraction output.
Acquire the stable exclusive maintenance gate before the final plan recheck
and retain it through fsynced commit or verified rollback.

- [ ] **Step 4: Run tests and dependency audit**

Run:

```bash
cargo test plugin:: -- --nocapture
cargo tree --depth 1
cargo clippy --all-targets -- -D warnings
```

Expected: transaction tests pass and every new direct dependency has one owner.

- [ ] **Step 5: Commit**

```bash
git add src/plugin src/support Cargo.toml Cargo.lock
git commit -m "feat: add plugin transactions"
```

### Task 16: Implement v9 migration, Omarchy integration, and doctor

**Files:**

- Create: `src/settings/migration.rs`
- Create: `src/plugin/omarchy.rs`
- Create: `src/plugin/doctor.rs`
- Create: `tests/fixtures/migration/v9/**`
- Test: migration, Omarchy, and doctor modules

**Interfaces:**

- Produces: `MigrationPlan::from_v9`, `OmarchyClient`, `DoctorReport`.
- Consumes: transaction, settings store, exact shell bytes, and injected
  command runner.

- [ ] **Step 1: Add failing migration and argv tests**

Fixtures cover every bar section/index, valid inline interval, invalid value,
duplicate entry, comments/formatting preservation where applicable, unknown
legacy keys, missing settings, repeated migration, rescan failure, and doctor
read-only/clean behavior. The fake Omarchy runner must record exact argv.
Fault injection must distinguish v10 health rollback, v9
`legacy-structural-rollback` (hashes/modes, exact shell bytes, enabled exact
listPlugins entry), and fresh-install absence rollback.

- [ ] **Step 2: Run focused tests**

Run:

```bash
cargo test settings::migration -- --nocapture
cargo test plugin::omarchy -- --nocapture
cargo test plugin::doctor -- --nocapture
```

Expected: v9 setup loses placement and doctor lacks the v10 ownership model.

- [ ] **Step 3: Implement migration and exact Quattro commands**

Fresh setup uses:

```text
omarchy plugin enable agent-bar.usage
```

Existing setup/update uses:

```text
omarchy plugin rescan
```

Never invoke `omarchy bar plugin add`. Update never writes `shell.json`.
Migration removes only the Agent Bar inline refresh key after its value is
validated and stored.

- [ ] **Step 4: Run migration and integration suites**

Run:

```bash
cargo test settings::migration -- --nocapture
cargo test plugin::omarchy -- --nocapture
cargo test plugin::doctor -- --nocapture
```

Expected: exact placement and unrelated shell bytes survive.

- [ ] **Step 5: Commit**

```bash
git add src/settings/migration.rs src/plugin tests/fixtures/migration
git commit -m "feat: migrate v9 without layout drift"
```

### Task 17: Build, install, and update one complete plugin bundle

**Files:**

- Create: `src/plugin/bundle.rs`
- Create: `src/plugin/maintenance.rs`
- Create: `src/bin/agent-bar-bundle.rs`
- Modify: `assets/omarchy/Service.qml`
- Rewrite: `install.sh`
- Rewrite: `scripts/check-version`
- Rewrite: `.github/workflows/publish.yml`
- Test: bundle and maintenance modules

**Interfaces:**

- Produces: `BundleReceipt`, `BundleBuilder`, `BundleValidator`,
  `ReleaseBuilder`, `UpdateCheck`, `MaintenanceWorker`.
- Produces: service health IPC for the expected version.
- Consumes: transaction journal and official GitHub release metadata.

- [ ] **Step 1: Add failing bundle and self-update tests**

Cover deterministic inventory, exact manifest, the literal `bundle.json` and
`update check` shapes from `08-plugin-bundle-and-release.md`, version mismatch,
wrong architecture, checksum, file mode, extra/missing file, traversal, link
types, modified local bundle, interrupted download, redirect outside the
closed GitHub download policy, redirect depth/scheme/host/userinfo/port
violations, incomplete release assets, release-metadata equality,
target/Omarchy-contract/Quickshell compatibility probes, draft/prerelease
rejection, absence of credentials on download redirects, downgrade, exchange
failure, rescan failure, service health mismatch, uninstall `listPlugins`
presence, malformed IPC JSON,
asynchronous-rescan polling/timeout, rollback health poll, user-manager/shell
preflight, v10-health/v9-structural/fresh-absence rollback verification, exact
transient-unit properties, monotonic worker deadlines and rollback reserve,
environment allowlist, failed handoff before mutation, rollback, and no global
executable.
Also test the exact internal release-builder grammar, dirty/wrong HEAD
rejection, empty-output requirement, release-notes path, and production of the
archive, checksum, metadata JSON, and LICENSE with cross-file equality.

- [ ] **Step 2: Run focused tests**

Run:

```bash
cargo test plugin::bundle -- --nocapture
cargo test plugin::maintenance -- --nocapture
```

Expected: failures because v9 builds a standalone binary/tarball.

- [ ] **Step 3: Implement bundle assembly and detached finalization**

Build one `agent-bar.usage/` tree, then generate `bundle.json` from every other
file. For update/uninstall:

1. copy and verify the current helper into the transaction directory as
   `agent-bar-maintenance-worker`;
2. create and sync the journal;
3. start a transient user systemd unit with worker path and journal ID as argv;
4. let worker exchange, rescan, perform the bounded health/absence poll,
   commit/rollback, report, garbage-collect, and notify.

After update commit, delete the exchanged old-bundle sibling as post-commit
garbage collection. Failure records the exact residual without rollback;
modified accepted bundles are first preserved in durable backup.

The copied executable name selects worker mode before public CLI parsing.
Use the exact unit name, properties, environment allowlist, destination-local
stage/quarantine paths, and rescan-poll algorithm in
`08-plugin-bundle-and-release.md`.

Rewrite `publish.yml` to build the authorized tag commit once for
`x86_64-unknown-linux-gnu`, run Rust/schema/bundle/release-metadata equality
gates, invoke the exact release builder, and upload archive, checksum, metadata,
and LICENSE without `--clobber`. Remove musl/zigbuild, standalone tarball, AUR,
and package publication. QML/Quattro gates consume the accepted checkpoint
evidence rather than pretending Ubuntu provides the Omarchy runtime.

- [ ] **Step 4: Assemble and validate a release-mode bundle**

Run:

```bash
cargo build --release
cargo run --bin agent-bar-bundle -- \
  assemble output target/release/agent-bar.usage \
  source-commit 0000000000000000000000000000000000000000
omarchy plugin validate target/release/agent-bar.usage
find target/release/agent-bar.usage -type f -name '*.qml' -exec \
  qmllint -I /usr/share/omarchy/shell {} +
shellcheck target/release/agent-bar.usage/scripts/agent-bar-open-terminal
target/release/agent-bar.usage/bin/agent-bar version
readelf -h target/release/agent-bar.usage/bin/agent-bar
```

Expected: ID/version/inventory/modes/architecture all match.

- [ ] **Step 5: Commit**

```bash
git add src/plugin src/bin assets/omarchy install.sh scripts/check-version .github/workflows/publish.yml Cargo.toml Cargo.lock
git commit -m "feat: package complete plugin bundle"
```

### Task 18: Implement transactional UI uninstall and purge

**Files:**

- Modify: `src/plugin/maintenance.rs`
- Modify: `src/plugin/transaction.rs`
- Modify: `assets/omarchy/MaintenanceView.qml`
- Modify: `assets/omarchy/Service.qml`
- Test: uninstall and QML Maintenance suites

**Interfaces:**

- Consumes: structured purge confirmation and transaction worker.
- Produces: quarantine, shell removal, absence health check, rollback, and final
  report.

- [ ] **Step 1: Add failing uninstall fault-matrix tests**

Inject failure before/after shell backup, exact-ID removal, quarantine rename,
rescan, absence check, commit-record fsync, settings purge quarantine, backup
purge quarantine, and post-commit quarantine deletion. Assert every pre-commit
failure rolls back and verifies the old service. Assert post-commit cleanup
failure never claims rollback, preserves a durable residual report, and leaves
settings/ambiguous legacy untouched for standard uninstall.
Also cover exact TTY phrase, EOF, malformed/extra JSON, false confirmation,
command/purge mismatch, and zero mutation on confirmation failure.
Add a barrier test with an external status helper already holding or awaiting
the shared maintenance gate: handoff drains service-owned mutable lanes,
exclusive maintenance completes, and no cache or notification path is
recreated.

- [ ] **Step 2: Run Rust and QML tests**

Run:

```bash
cargo test uninstall -- --nocapture
QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
```

Expected: missing transaction behavior fails.

- [ ] **Step 3: Implement quarantine-first removal**

Remove only exact `agent-bar.usage` layout entries, quarantine the bundle on the
same filesystem, rescan from the transient worker, verify absence, then fsync
the irreversible commit record and report. On pre-commit failure restore every
quarantine and exact shell bytes and verify the old service. Purge uses a
destination-local quarantine for each selected path. Post-commit deletion is
garbage collection; failures leave reported residual paths and do not claim
rollback.

- [ ] **Step 4: Run uninstall suites**

Run:

```bash
cargo test uninstall -- --nocapture
cargo test plugin::transaction -- --nocapture
QT_QPA_PLATFORM=offscreen qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
```

Expected: every injected pre-commit failure rolls back; post-commit cleanup
failures produce durable residual reports without claiming rollback; both
confirmation paths pass.

- [ ] **Step 5: Commit**

```bash
git add src/plugin assets/omarchy tests
git commit -m "feat: add transactional uninstall"
```

### Task 19: Delete all v9 consumers, producers, and dependencies

**Files:**

- Delete: every path in the locked deletion inventory
- Delete after migration: legacy root modules superseded by new directories
- Modify: `src/main.rs`
- Modify: `src/lib.rs`
- Modify: `src/app_identity.rs`
- Modify: `Cargo.toml`
- Regenerate: `Cargo.lock`
- Test: `tests/cli.rs` and complete Rust suite

**Interfaces:**

- Consumes: all v10 replacements from Tasks 1–18.
- Produces: a v10-only source graph.

- [ ] **Step 1: Add the failing active-legacy scan**

Create a semantic test/script using the exact scope, behavioral patterns,
legitimate v1 contracts, migration fixture allowances, and historical cuts in
`07-testing-and-acceptance.md`. It must detect TUI, Waybar, old quota/config
shapes, history/cost/money producers, Redb/Postcard, AUR/binstall, global
install paths, hidden TTY fallback, legacy commands, and orphan dependencies
without rejecting negative removal documentation or valid `session`,
`session_log`, `amp usage`, and plugin bootstrap references.

- [ ] **Step 2: Run scan and record expected v9 matches**

Run: `cargo test active_legacy_scan -- --nocapture`

Expected: failure listing current v9 source and packaging.

- [ ] **Step 3: Delete consumers before producers**

Deletion order:

1. v9 QML monolith and old CLI consumers;
2. `action_right` and TUI;
3. local usage/history/cost;
4. Waybar output/integration and Pango formatters;
5. old settings/config/cache/update/setup modules;
6. provider helper modules explicitly replaced by catalog/adapters;
7. standalone/AUR/binstall packaging and workflow branches;
8. dead tests, snapshots, `build.rs`, constants, dependencies, root icon
   duplicates, and the legacy documentation banner.

Delete every literal path in the locked inventory; no “superseded root module”
may remain by omission. Retain the rewritten login helper, plugin-scoped
`install.sh`, rewritten `scripts/check-version`, and migrated provider fixtures.

- [ ] **Step 4: Regenerate dependencies and run full Rust gates**

Run:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo tree --prefix none
```

Expected: Cargo regenerates `Cargo.lock` only as required by the edited
`Cargo.toml`; no unrelated dependency is upgraded, no removed dependency or
module remains, and all v10 tests pass.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: remove all v9 legacy"
```

### Task 20: Align active documentation and release workflow

**Files:**

- Modify/audit: `README.md`
- Modify/audit: `PRODUCT.md`
- Modify/audit: `CONTEXT.md`
- Modify/audit: `CONTRIBUTING.md`
- Modify/audit: `CLAUDE.md`
- Modify/audit: `AGENTS.md`
- Modify/audit: `docs/README.md`
- Modify/audit: `docs/architecture.md`
- Modify/audit: `docs/commands.md`
- Modify/audit: `docs/integration.md`
- Modify/audit: `docs/json-output.md`
- Modify/audit: `docs/new-provider.md`
- Modify/audit: `docs/omarchy-shell.md`
- Modify/audit: `docs/releasing.md`
- Modify/audit: `docs/runtime.md`
- Modify/audit: `docs/troubleshooting.md`
- Modify/audit: `docs/agents/domain.md`
- Audit: `docs/agents/issue-tracker.md`
- Audit: `docs/agents/triage-labels.md`
- Delete: `docs/waybar-contract.md`
- Create: `docs/adr/0004-quickshell-only-v10.md`
- Create: `docs/releases/10.0.0.md`
- Modify: `docs/adr/README.md`
- Modify: `CHANGELOG.md`
- Audit: `.github/workflows/publish.yml`

**Interfaces:**

- Consumes: completed v10 behavior and checked-in schemas.
- Produces: English active docs with executable examples.

- [ ] **Step 1: Run documentation contract tests**

Run the legacy scan, command-example parser test, JSON-example schema test, and
link checker. Confirm the transition warning still exists until this task.

- [ ] **Step 2: Observe documentation failures**

Run:

```bash
cargo test active_docs -- --nocapture
git diff --check
```

Expected: any implementation-induced drift or transition warning fails.

- [ ] **Step 3: Align docs without rewriting history**

Update exact paths, commands, defaults, maintenance, migration, bundle, and
tests. Mark ADRs 0001–0003 superseded in the index, add ADR 0004, retain their
bodies, retain changelog release sections 9.0.0 and older, and retain
`docs/superpowers/**`. Keep Unreleased, the ADR index, and ADR 0004 inside the
active gates. Remove the target-only warning after all v10 behavior exists.

- [ ] **Step 4: Run checkpoint-3 gates**

Run:

```bash
cargo test active_docs -- --nocapture
cargo test active_legacy_scan -- --nocapture
git diff --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Expected: active docs and source are v10-only.

- [ ] **Step 5: Commit and stop for checkpoint 3**

```bash
git add -A
git commit -m "docs: align active contracts with v10"
```

Write `/tmp/agent-bar-v10-checkpoint-03.md`, push the stable branch, and stop.

## Checkpoint 4 — Release candidate and live QA

### Task 21: Produce the isolated release candidate

**Files:**

- Modify only files required by blocking checkpoint-3 review findings
- Create ignored artifacts under `target/release-candidate/`
- Modify only if required: `docs/releases/10.0.0.md`

**Interfaces:**

- Produces: one verified `10.0.0` plugin archive, checksum, screenshots, test
  logs, metadata JSON, release notes, and requirement coverage report.

- [ ] **Step 1: Apply only approved checkpoint-3 corrections**

Each correction begins with a reproducing test and its own focused commit.
Commit the final tracked release notes and corrections, then require a clean
worktree. Resolve that immutable `HEAD` as `SOURCE_COMMIT`; no tracked source
may change after this point.

- [ ] **Step 2: Run the complete isolated matrix**

Run every command in `07-testing-and-acceptance.md`, including
`scripts/verify-v10-ui`, Rust, QML, manifest, bundle, archive, docs, legacy,
light/dark theme, absence of plugin-authored motion, and all deterministic
screenshots.

- [ ] **Step 3: Assemble the release candidate twice**

Build from the same clean `SOURCE_COMMIT` twice into distinct empty directories
using the exact `agent-bar-bundle release` command. Compare bundle inventory,
modes, manifest/helper version, archive contents, checksum, metadata, and file
hashes. Explain any intentionally nondeterministic archive container metadata;
content inventory must match.

```bash
cargo run --bin agent-bar-bundle -- assemble \
  output target/release-candidate/bundle-1 \
  source-commit "$SOURCE_COMMIT"
cargo run --bin agent-bar-bundle -- release \
  bundle target/release-candidate/bundle-1 \
  output target/release-candidate/build-1 \
  source-commit "$SOURCE_COMMIT" \
  release-notes docs/releases/10.0.0.md
```

Repeat with fresh `bundle-2` and `build-2` paths.

- [ ] **Step 4: Self-review the full feature range**

Compare every requirement ID to `REQUIREMENTS_MATRIX.md`. Record zero silent
deviations. Audit the remote branch, commits, and worktree status.

- [ ] **Step 5: Commit and stop before live QA**

Confirm there are no uncommitted source/docs changes, push the already
committed `SOURCE_COMMIT`, write
`/tmp/agent-bar-v10-checkpoint-04.md`, and stop. Do not install live.

### Task 22: Run authorized live Omarchy QA after Codex approval

**Files:**

- Do not edit repository source unless a live failure is first reproduced in
  an isolated test and checkpoint 4 is reopened.
- Store evidence in temporary or ignored QA paths.

**Interfaces:**

- Consumes: Codex approval of checkpoint 4.
- Produces: live QA report, screenshots, sanitized logs, and verified rollback.

- [ ] **Step 1: Capture exact live backup and baseline**

Back up the current plugin, `settings.json`, exact `shell.json` bytes, installed
versions, monitor list, and relevant logs. Hash every backup.

- [ ] **Step 2: Install the release candidate transactionally**

Use the candidate's plugin-scoped installer. Do not alter unrelated plugins,
bar layout, Hyprland, themes, terminal settings, or system packages.

- [ ] **Step 3: Execute the live acceptance script**

Test both monitors, popup transfer, every pointer and keyboard action, scroll,
focus, dark/light theme, absence of plugin-authored motion, provider states,
refresh races, settings races, notifications, update check/no-update state,
and standard/purge uninstall in the approved safe sequence. Do not fabricate a
production update source for the unpublished 10.0.0 candidate; real
self-update apply is a separately authorized post-release smoke test.

- [ ] **Step 4: Verify rollback**

Restore the exact baseline bundle, settings, shell bytes, and runtime state.
Rescan, verify both monitors, compare hashes, and confirm no residual plugin,
worker, journal, quarantine, or global executable.

- [ ] **Step 5: Write final checkpoint and open the PR**

Write `/tmp/agent-bar-v10-live-qa.md`. After all evidence passes, push the final
branch and open a ready PR. Do not merge, tag, publish, or distribute.
