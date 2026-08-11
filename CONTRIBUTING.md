# Contributing

> v10 implementation follows the canonical plan in
> [docs/specs/v10/09-implementation-plan.md](docs/specs/v10/09-implementation-plan.md).

## Prerequisites

- Current stable Rust toolchain through rustup.
- Git.
- Omarchy Quattro and Quickshell development files for QML validation.
- Qt Quick Test tools (`qmltestrunner`, `qmllint`).
- ShellCheck for the retained Bash helper.

Rust/Cargo is the application toolchain. Do not add Node, npm, Bun, pnpm, Yarn,
Deno, or JavaScript build tooling.

## Safe development

Do not install a work-in-progress build into the live desktop. Use:

- isolated Git worktrees;
- temporary plugin roots reached through an isolated `HOME`;
- isolated `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, and `XDG_STATE_HOME`;
- fake provider executables and fixture data;
- offscreen QML tests.

The live `$HOME/.config/omarchy/plugins` and `shell.json` are reserved for the
explicit final QA gate.

## Standard verification

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

QML/plugin verification:

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

Shell helper:

```bash
shellcheck scripts/agent-bar-open-terminal
```

## Test rules

- Write the failing test before behavior.
- Never use live provider credentials or provider network in tests.
- Inject clock, filesystem, process runner, HTTP, and XDG roots.
- Test single-provider and all-provider paths through the same policy.
- Treat QML behavior, accessibility, scrolling, and screenshots as release
  gates.
- Do not update snapshots merely to silence an unexpected change.

## Provider changes

Read [docs/dev/new-provider.md](docs/dev/new-provider.md). Provider-specific behavior
stays behind the adapter. QML receives schema-v2 normalized data only.

## Commits and checkpoints

- Use English Conventional Commit subjects of at most 50 characters.
- Keep one reviewable behavior per commit.
- Do not bypass hooks or signatures.
- Stop at the mandatory checkpoints in the implementation plan.
- Record exact commands, results, screenshots, and deviations.

## Documentation

Active documentation is English and must match executable contracts.
Every versioned changelog release section, `docs/releases/**`,
`docs/history/**`, ADR bodies 0001–0003, and `docs/superpowers/**` remain
historical: they record how things were and are never rewritten. The
`[Unreleased]` changelog section, the ADR index, ADR 0004, and everything
under `docs/specs/v10/**` and `docs/guide/**` are active and must match the
shipped behavior.

## Release

Merging to `master` triggers an automatic patch release: version bump, Rust
gates, a tagged bump commit, and a push of the assembled plugin tree to the
distribution repository (`othavi0/omarchy-agent-bar`), followed by the
tagged GitHub Release on this repository. There are no release assets to
attach; the distribution repository's tree is the release. Implementation
may prepare a release candidate and open a ready PR, but merge itself is
the release decision and requires separate explicit authorization. See
[docs/dev/releasing.md](docs/dev/releasing.md).
