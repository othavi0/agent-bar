# Agent Bar Codex Adapter

The canonical repository instructions are in [CLAUDE.md](CLAUDE.md). Read that
file before editing code or documentation.

The approved v10 design and implementation plan are in
[docs/specs/v10/](docs/specs/v10/README.md). This branch implements that plan.

## Boot order

1. Read this file.
2. Read `CLAUDE.md`.
3. Read the relevant v10 specification files.
4. Check `git status --short`.
5. Preserve unrelated changes.

## Hard bootstrap

- Rust/Cargo and QML only; no Node runtime or test tooling.
- Product is only the Quickshell plugin `othavi0.agent-bar`.
- The Rust helper is private inside the plugin bundle.
- Keep the terminal helper as argv-safe Bash.
- Do not run live setup, update, uninstall, rescan, shell restart, or config
  mutation outside the approved final QA gate.
- Use temporary plugin roots and isolated XDG directories for tests.
- Do not edit `/usr/share/omarchy`.
- Do not commit or push unless authorized.
- Never bypass hooks, force-push, merge, tag, or publish without explicit
  authorization.

## Codex tool translation

| Intent | Codex |
| --- | --- |
| Read/search | `exec_command` with `rtk sed` / `rtk rg` |
| Edit | `apply_patch` |
| Progress | `update_plan` |
| Subagent | `spawn_agent` only when user/harness permits |
| Current external docs | `ctx7` or the applicable documentation skill |
| Browser QA | `agent-browser` |

Prefer `rtk` for shell commands. Use `rg`/`rg --files` for search.

## Verification

Documentation-only changes:

```bash
git diff --check
```

Shared contract changes:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

QML and bundle gates are defined in `CLAUDE.md` and the v10 acceptance spec.
