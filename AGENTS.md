# agent-bar — Agent Instructions

The canonical agent instructions for this repository live in
[`CLAUDE.md`](CLAUDE.md).

Read and follow `CLAUDE.md` before editing files. This file is intentionally a
small compatibility shim so the instructions live in a single place and never
drift between files.

Quick bootstraps until `CLAUDE.md` is loaded:

- **Bun only** — no Node, npm, pnpm, yarn, ts-node, or Deno.
- Run the CLI as `./scripts/agent-bar` or `bun run start`; **never**
  `bun ./scripts/agent-bar` (it is a Bash shim).
- Do not run live-mutating commands (`agent-bar setup`, `update`, `uninstall`,
  `remove`) without explicit user approval.
- Do not hand-edit live `~/.config/waybar` or `~/.config/agent-bar` for
  verification — use temp directories and injected path flags.
