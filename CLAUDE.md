# Agent Bar Engineering Contract

`AGENTS.md` is the Codex adapter. This file is the repository's canonical agent
contract for Agent Bar v10. Source and executable tests win over ordinary
documentation; the approved specification in `docs/specs/v10/` is the product
contract when documentation and behavior disagree.

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
- Consolidated popup, Settings, login delegation, update, and uninstall
  delegated to the Omarchy plugin manager.
- Typed status JSON, cache, notifications, settings migration, and backup.

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
- Fresh installs use `omarchy plugin add <dist-repo-url>`, which clones,
  validates, and moves the tree; enabling it is a separate confirmation.
- Existing installs update with `omarchy plugin update agent-bar.usage`.
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
# PATH qmllint is a stub reporting version 1.0 that stays SILENT even on an
# undefined type — the Qt6 binary path is mandatory here too
find assets/omarchy -type f -name '*.qml' -exec \
  /usr/lib/qt6/bin/qmllint -I /usr/share/omarchy/shell {} +
omarchy plugin validate assets/omarchy
# PATH qmltestrunner is Qt5 and fails SILENTLY (errors only in journald) —
# the Qt6 binary path and both env vars below are mandatory
QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
  /usr/lib/qt6/bin/qmltestrunner \
  -input tests/qml \
  -import /usr/share/omarchy/shell \
  -import assets/omarchy \
  -o -,txt
```

`qmllint` catches syntax and structure only. It cannot resolve the `qs.*`
module namespace, so every plugin QML file — including files a change never
touched — emits unresolved-import and unqualified-access warnings. Read its
output for what a change introduced, never as a type check, and never treat
its exit code as a verdict: it exits 0 while printing warnings. Type errors,
dangling references, and readonly-property assignments in plugin QML reach
only `qmltestrunner`, `omarchy plugin validate`, and live QA.

Shell changes run ShellCheck. Bundle changes run `cargo test --test
dist_tree_validate`, which mirrors `omarchy-plugin-validate` and the complete
inventory/mode/architecture/version matrix; see
[docs/specs/v10/08-plugin-bundle-and-release.md](docs/specs/v10/08-plugin-bundle-and-release.md)
for the amended contract.

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

Active docs and public copy are English, enforced by
`tests/active_language.rs`. Only `docs/superpowers/**` is excluded: it holds
past session plans and specs, which are a build record rather than
documentation. New files written there are English.

## Pointers

- `docs/specs/v10/README.md` — canonical v10 reading order.
- `docs/specs/v10/09-implementation-plan.md` — executable plan.
- `docs/specs/v10/10-grok-execution-runbook.md` — permissions and checkpoints.
- `README.md` — product overview.
- `docs/dev/architecture.md` — runtime data flow.
- `docs/guide/commands.md` — private helper contract.
- `docs/guide/runtime.md` — paths, settings, cache, and privacy.
- `docs/dev/new-provider.md` — provider adapter checklist.
