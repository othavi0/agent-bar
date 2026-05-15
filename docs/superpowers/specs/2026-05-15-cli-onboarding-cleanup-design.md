# CLI Onboarding Cleanup Design

## Context

The project already has a single public binary, `agent-bar-omarchy`, backed by
`scripts/agent-bar-omarchy`. Older helper scripts and the `apply-local` command
overlap with the newer `setup` flow and make onboarding harder to explain.

The current working tree partially removed those legacy surfaces, but it is
incomplete: `README.md` documents `bun run setup` while `package.json` does not
define that script, and `tests/cli.test.ts` still expects `apply-local`.

## Decision

Remove `apply-local` completely as a public command. Do not keep hidden aliases
for `apply-local` or `apply local`.

Keep `agent-bar-omarchy setup` as the public idempotent command for full install
and Waybar wiring. Interactive flows that only need to refresh generated Waybar
files should continue calling `applyWaybarIntegration()` internally instead of
asking users to run a legacy command.

Add `bun run setup` as the clone-and-install onboarding path. It should run
dependency installation first and then invoke the CLI setup flow.

## Scope

- Remove `apply-local` from CLI types, parser, help, dispatch, tests, docs, and
  agent instructions.
- Keep the deleted helper wrappers removed:
  - `scripts/agent-bar-omarchy-apply-local`
  - `scripts/agent-bar-omarchy-refresh`
  - `scripts/agent-bar-omarchy-remove`
  - `scripts/agent-bar-omarchy-setup`
  - `scripts/agent-bar-omarchy-uninstall`
- Keep `src/apply-local.ts` and `src/refresh.ts` removed if no live import needs
  them.
- Add or fix package scripts for onboarding.
- Leave historical references in `CHANGELOG.md`, `docs/plans/**`, and
  `docs/superpowers/**` unless they are part of current operational docs.

## Non-Goals

- Do not rename `setup` to `sync`.
- Do not add a new public `sync` command.
- Do not mutate the user's live Waybar setup during verification.
- Do not commit without explicit approval.

## Testing

Focused verification:

- `bun test tests/cli.test.ts`
- `bun run typecheck`
- `git diff --check`

Broaden to `bun run lint` if formatting or docs edits indicate style issues.
