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
- temporary plugin roots;
- `setup plugins-dir <path>`;
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
find assets/omarchy -type f -name '*.qml' -exec \
  qmllint -I /usr/share/omarchy/shell {} +

omarchy plugin validate assets/omarchy

QT_QPA_PLATFORM=offscreen qmltestrunner \
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
Changelog release sections 9.0.0 and older, ADR bodies 0001–0003, and
`docs/superpowers/**` remain historical. Unreleased, the ADR index, and ADR
0004 are active.

## Release

Implementation may prepare a release candidate and open a ready PR. Merge,
tag, GitHub Release publication, and distribution require separate explicit
authorization. See [docs/dev/releasing.md](docs/dev/releasing.md).
