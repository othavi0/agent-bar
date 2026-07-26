# Agent Bar Engineering Contract

> This branch specifies the v10 target. Until implementation completes, v9
> code may still contradict this file. The canonical transition contract is
> `docs/specs/v10/`.

`AGENTS.md` is the Codex adapter. This file is the repository's canonical agent
contract. Source and executable tests win over ordinary documentation; the
approved v10 specification wins when replacing v9 behavior.

## Hard rules

- Rust/Cargo and QML only. No Node, npm, Bun, pnpm, Yarn, ts-node, or Deno.
- Product artifact is only the Omarchy Quattro plugin `agent-bar.usage`.
- The Rust helper is private at plugin path `bin/agent-bar`.
- Do not create a global executable, standalone application, AUR package, or
  cargo-binstall product.
- Keep `scripts/agent-bar-open-terminal` as Bash. Rewrite it argv-safe; never
  use `sh -c`, `bash -lc`, `eval`, or `cmd="$*"`.
- No production `unwrap()` or `expect()`.
- Status JSON stdout is exactly one schema-v2 object plus newline. Settings and
  update commands use their separately documented JSON contracts. Logs use
  stderr.
- Provider operational failures are typed data, not process failures.
- QML never parses raw provider output or human error messages.
- Render external strings as plain text.
- Settings reads never write. Explicit apply/migration uses lock and atomic
  replacement.
- Do not install provider CLIs or handle credentials.
- Do not edit `/usr/share/omarchy`.
- Do not mutate live Omarchy/Hyprland/config paths outside the final authorized
  QA gate.
- Preserve unrelated worktree changes.
- Never bypass hooks, force-push, merge, tag, or publish without explicit
  authorization.

## Product boundaries

v10 includes:

- Claude, Codex, Amp, and Grok percentage quota windows.
- One shared Quickshell service and monitor-local bar widgets.
- Consolidated popup, Settings, login delegation, update, and uninstall.
- Typed status JSON, cache, notifications, migration, backup, and rollback.

v10 removes:

- TUI and terminal dashboard.
- Waybar and Pango output.
- Session history and charts.
- Local or provider-reported monetary data.
- Schema-v1 status compatibility.
- Permanent daemon and global installation.

Do not retain removed behavior behind features, aliases, stubs, or dormant
dependencies.

## Quattro contract

- Plugin root is literal
  `$HOME/.config/omarchy/plugins/agent-bar.usage`.
- Agent Bar settings/cache/state follow XDG.
- Manifest schema remains 1 with kinds `service` and `bar-widget`.
- `Service.qml` is the sole polling/process owner.
- `BarWidget.qml` resolves the service through
  `bar.shell.serviceFor(moduleName)`.
- Fresh setup uses `omarchy plugin enable agent-bar.usage`.
- Existing setup/update uses `omarchy plugin rescan`.
- Never run an unconditional `omarchy bar plugin add`.
- Update never edits `shell.json`.

## Provider rules

- One catalog owns ID, name, icon, order, official URL, TTL, and timeout.
- Collection availability and login availability are distinct.
- Providers normalize into typed domain results; `status::schema` alone
  serializes JSON.
- Single-provider and all-provider paths share timeout, retry, cache, and
  normalization.
- Raw output, credentials, tokens, account identifiers, and headers never enter
  logs, cache, screenshots, or UI.
- A connected provider without a percentage window is valid and renders `—`.
- Do not reintroduce spend, balance, credits, currency, or arbitrary extras.

## Verification

Focused checks are allowed while developing. Every checkpoint runs:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
git diff --check
```

QML/plugin changes also run:

```bash
find assets/omarchy -type f -name '*.qml' -exec \
  qmllint -I /usr/share/omarchy/shell {} +
omarchy plugin validate assets/omarchy
QT_QPA_PLATFORM=offscreen qmltestrunner \
  -input tests/qml \
  -import /usr/share/omarchy/shell \
  -import assets/omarchy \
  -o -,txt
```

Shell changes run ShellCheck. Bundle changes run the complete archive,
inventory, mode, architecture, version, traversal, and rollback matrix in the
v10 specification.

Tests use fake providers, fake clock/process/HTTP/filesystem seams, temporary
plugin roots, and isolated XDG directories. No live network or credentials.

## Workflow

1. Check `git status`.
2. Read this file and the relevant v10 spec.
3. Write a failing test.
4. Run it and confirm the intended failure.
5. Implement the smallest contract-complete change.
6. Run focused verification.
7. Review for secrets, shell construction, legacy leakage, and unrelated diff.
8. Commit with an English Conventional Commit subject of at most 50
   characters.
9. Stop at the mandatory Grok/Codex checkpoint.

The implementation branch is `feat/quickshell-native-v10`, created from the
exact `spec/quickshell-native-v10` commit. Grok may push and open the final
ready PR. Grok may not merge.

## Documentation

Active docs and public copy are English. Historical changelog entries, ADR
bodies 0001–0003, and `docs/superpowers/**` remain historical and are excluded
from active legacy/language scans.

## Pointers

- `docs/specs/v10/README.md` — canonical v10 reading order.
- `docs/specs/v10/09-implementation-plan.md` — executable plan.
- `docs/specs/v10/10-grok-execution-runbook.md` — permissions and checkpoints.
- `README.md` — product overview.
- `docs/architecture.md` — runtime data flow.
- `docs/commands.md` — private helper contract.
- `docs/runtime.md` — paths, settings, cache, and privacy.
- `docs/new-provider.md` — provider adapter checklist.
