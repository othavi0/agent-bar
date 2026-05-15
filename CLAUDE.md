# agent-bar-omarchy — Claude Code

The canonical agent instructions for this repository live in [`AGENTS.md`](AGENTS.md).

Claude Code must read and follow `AGENTS.md` before editing files. This file is intentionally a small compatibility shim so Claude-specific instructions do not drift from the shared agent contract.

Quick bootstraps until `AGENTS.md` is loaded:

- Bun is the only supported runtime.
- Use `bun run start` or `./scripts/agent-bar-omarchy`; never run `bun ./scripts/agent-bar-omarchy`.
- Do not run live-mutating commands such as `agent-bar-omarchy setup`, `uninstall`, `remove`, or `update` unless the user explicitly asks.
- Do not manually edit live `~/.config/waybar` or `~/.config/agent-bar-omarchy` for verification; use temp paths and injected paths instead.
- `qbar` is legacy migration/removal compatibility only; do not add new user-facing `qbar` behavior.
