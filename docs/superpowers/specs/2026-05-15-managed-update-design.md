# Managed Update Design

## Context

`agent-bar-omarchy` is meant to be installed as a local Waybar tool, not managed
as an end-user development checkout. The README currently points users at a
personal `~/Projects/agent-bar-omarchy` path. That path is appropriate for
development, but not for a small installed CLI.

The existing `agent-bar-omarchy update` command fetches and pulls the current
checkout, installs dependencies unconditionally, and reloads Waybar. It does not
re-run setup, does not intentionally discard local edits, and has plain terminal
feedback.

## Decision

Use `~/.agent-bar` as the documented install directory.

Treat `~/.agent-bar` as a managed installation. `agent-bar-omarchy update` may
discard local changes there, pull the upstream state, update dependencies when
needed, and re-apply setup.

Do not make destructive update behavior apply to arbitrary development
checkouts. If the repo root is not `~/.agent-bar`, `update` must stop with a
clear message.

## User Experience

`agent-bar-omarchy update` should look and read like the interactive TUI:

- colored command header
- compact repo/branch/commit summary
- explicit destructive warning for `~/.agent-bar`
- visible step-by-step progress
- concise success/cancel/error outro

The update flow should ask for one confirmation before destructive actions. It
should not ask again when re-running setup.

## Update Flow

1. Resolve the repository root from the running checkout.
2. Confirm the root is exactly `~/.agent-bar`.
3. Verify the root is a git repository.
4. Fetch from origin with prune.
5. Resolve upstream from `@{u}`, with fallback to `origin/master`.
6. Detect pending upstream commits and local changes.
7. Detect whether dependency files change between `HEAD` and upstream:
   `package.json`, `bun.lock`, or `bun.lockb`.
8. If no upstream commits and no local changes exist, exit without mutation.
9. Before mutation, show commits/status and ask for confirmation.
10. Run `git reset --hard <upstream>` and `git clean -fd`.
11. Run `bun install` only if dependency files changed or `node_modules` is
    missing.
12. Re-run setup without a second prompt.

## Setup Refactor

Extract setup execution into a reusable function:

- `runSetup({ confirm, clearScreen })`
- `main()` calls `runSetup({ confirm: true, clearScreen: true })`
- `update` calls `runSetup({ confirm: false, clearScreen: false })`

The existing `agent-bar-omarchy setup` behavior remains interactive.

## Non-Goals

- Do not support destructive updates outside `~/.agent-bar`.
- Do not auto-stash local edits.
- Do not replace the TUI.
- Do not run live setup during tests.

## Verification

- Unit tests for update orchestration with a fake command runner.
- `bun test tests/update.test.ts`
- `bun test tests/cli.test.ts`
- `bun run typecheck`
- `bun run lint`
- `git diff --check`
