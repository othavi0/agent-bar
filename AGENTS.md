# agent-bar — Codex Adapter

This repository is **Claude Code first**. The canonical agent instructions live
in [`CLAUDE.md`](CLAUDE.md), and Codex must read and follow that file before
editing anything here.

This file is only the Codex compatibility adapter. Keep it small enough to avoid
drift, but explicit enough that a fresh Codex session can bootstrap safely.

## Boot Order

1. Read this file.
2. Read [`CLAUDE.md`](CLAUDE.md) in full — it now fits comfortably under 200
   lines after the 4.0.3 prune.
3. Let `CLAUDE.md` define the repo contract. The code in `src/` still wins over
   docs when behavior and docs disagree.
4. Translate Claude Code-specific tools and workflows to Codex equivalents using
   the table below.

## Hard Bootstraps

Use these before `CLAUDE.md` is fully loaded:

- **Bun only** — no Node, npm, pnpm, yarn, ts-node, or Deno for runtime or test
  workflows.
- Run the CLI as `./scripts/agent-bar` or `bun run start`; **never**
  `bun ./scripts/agent-bar` because it is a Bash shim.
- Never convert the Bash shims in `scripts/` to TypeScript.
- Do not run live-mutating commands (`agent-bar setup`, `update`, `uninstall`,
  `remove`) without explicit user approval.
- Do not hand-edit live `~/.config/waybar` or `~/.config/agent-bar` for
  verification. Use temp directories, injected path flags, and `XDG_*`
  overrides.
- Keep stdout clean for Waybar JSON. Diagnostics belong on stderr unless the
  command is intentionally terminal/TUI output.
- Preserve unrelated user changes in the worktree.

## Claude Code To Codex

| Claude Code concept | Codex equivalent in this repo |
| --- | --- |
| `CLAUDE.md` project memory | Canonical repo instructions. Read it directly. |
| `AskUserQuestion` | `request_user_input` when available; otherwise ask one concise question only when needed. |
| `Bash` | `exec_command`; prefer `rtk` prefixes where possible. |
| `Read` | `exec_command` with `rtk sed`, `rtk nl`, `rtk head`, or `rtk tail`. |
| `Grep` | `exec_command` with `rtk rg`. |
| `Glob` | `exec_command` with `rtk rg --files` or plain `find` when predicates are needed. |
| `Write` / `Edit` / `MultiEdit` | `apply_patch`. |
| `TodoWrite` | `update_plan` for visible progress. |
| `Task` / subagents | `spawn_agent` only when the user or harness permits delegation. |
| `Skill` | Open the relevant `SKILL.md` and follow it. |
| `WebSearch` / `WebFetch` | `web.run`; for library docs prefer `ctx7` when the global instructions require it. |
| Plan mode | Controlled by the Codex harness; do not fake it with file edits. |

## Codex Workflow In This Repo

- Start with `rtk git status --short`.
- Read the smallest relevant slice of [`CLAUDE.md`](CLAUDE.md) and source files.
- For docs or agent-instruction-only edits, focused verification is
  `git diff --check`.
- For code changes, use the verification matrix in `CLAUDE.md`; broaden to
  `bun test && bun run typecheck && bun run lint` when shared contracts move.
- Do not commit or push unless the user explicitly asks.

## Repo Pointers

- [`CLAUDE.md`](CLAUDE.md) — canonical agent contract.
- [`README.md`](README.md) — install and command surface.
- [`docs/README.md`](docs/README.md) — operational docs index.
- [`docs/commands.md`](docs/commands.md) — CLI reference.
- [`docs/runtime.md`](docs/runtime.md) — settings, cache, credentials, owned paths.
- [`docs/integration.md`](docs/integration.md) — setup/update/remove ownership model.
- [`docs/waybar-contract.md`](docs/waybar-contract.md) — generated Waybar module/CSS contract.
- [`docs/new-provider.md`](docs/new-provider.md) — provider extension checklist.
