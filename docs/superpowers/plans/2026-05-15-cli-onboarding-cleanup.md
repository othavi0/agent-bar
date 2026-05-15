# CLI Onboarding Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove legacy setup/apply surfaces completely and make `bun run setup` the documented clone-and-install onboarding path.

**Architecture:** The public command surface is centralized in `src/cli.ts` and dispatched in `src/index.ts`. Full live wiring remains owned by `src/setup.ts`; TUI flows keep using `applyWaybarIntegration()` internally for automatic sync. Operational docs should describe only current commands.

**Tech Stack:** Bun runtime and scripts, TypeScript ESM, `bun:test`, Biome formatting.

---

## File Structure

- Modify `package.json`: add the onboarding `setup` script without changing dependency versions.
- Modify `src/cli.ts`: remove `apply-local` remnants and clean whitespace.
- Modify `src/index.ts`: remove dead blank space left by deleted dispatch.
- Modify `tests/cli.test.ts`: remove `apply-local` parser expectations.
- Modify `README.md` and current `docs/*.md`: ensure public docs mention `setup`, not `apply-local`.
- Modify `AGENTS.md` and `CLAUDE.md`: keep agent safety instructions aligned with the current command surface.
- Keep deleted legacy files removed: old wrapper scripts plus `src/apply-local.ts` and `src/refresh.ts`.

## Task 1: Package Script

**Files:**
- Modify: `package.json`

- [ ] **Step 1: Add onboarding script**

Add this script entry next to `start`:

```json
"setup": "bun install && bun run start setup"
```

- [ ] **Step 2: Verify script is discoverable without running live setup**

Run:

```bash
bun pm pkg get scripts.setup
```

Expected output includes:

```json
"bun install && bun run start setup"
```

Do not run `bun run setup` during verification because it mutates live Waybar paths.

## Task 2: CLI Parser Cleanup

**Files:**
- Modify: `src/cli.ts`
- Modify: `src/index.ts`

- [ ] **Step 1: Remove dead parser surface**

Ensure `CliOptions.command` does not include `apply-local`, `showHelp()` does not print it, `KNOWN_COMMANDS` does not suggest it, and `parseArgs()` does not parse either `apply-local` or `apply local`.

- [ ] **Step 2: Clean dispatch leftovers**

Ensure `src/index.ts` has no `options.command === 'apply-local'` branch and no extra blank block where the branch was removed.

- [ ] **Step 3: Search current source for live references**

Run:

```bash
rg "apply-local|apply local|agent-bar-omarchy-apply-local" src tests README.md docs AGENTS.md CLAUDE.md
```

Expected: no results in current source, tests, README, current docs, or agent instructions. Historical docs under `docs/superpowers/**` are intentionally outside this command.

## Task 3: Test Contract Update

**Files:**
- Modify: `tests/cli.test.ts`

- [ ] **Step 1: Remove obsolete tests**

Delete the two tests that assert:

```ts
expect(parseArgs(['apply-local']).command).toBe('apply-local');
expect(parseArgs(['apply', 'local']).command).toBe('apply-local');
```

- [ ] **Step 2: Run focused CLI tests**

Run:

```bash
bun test tests/cli.test.ts
```

Expected: all tests in `tests/cli.test.ts` pass.

## Task 4: Documentation Alignment

**Files:**
- Modify: `README.md`
- Modify: `docs/README.md`
- Modify: `docs/commands.md`
- Modify: `docs/integration.md`
- Modify: `docs/runtime.md`
- Modify: `docs/troubleshooting.md`
- Modify: `docs/waybar-contract.md`
- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`
- Modify: `src/tui/configure-layout.ts`

- [ ] **Step 1: Keep current docs on current commands**

Ensure operational docs describe:

```bash
bun run setup
agent-bar-omarchy setup
agent-bar-omarchy update
agent-bar-omarchy uninstall
agent-bar-omarchy remove
```

Do not document removed wrapper scripts.

- [ ] **Step 2: Improve fallback wording**

In `src/tui/configure-layout.ts`, keep the fallback concise and accurate:

```ts
`Could not sync Waybar automatically. Run \`${APP_NAME} setup\` to reinstall the integration.`
```

and:

```ts
`If changes didn't take effect, run \`${APP_NAME} setup\` to reinstall the integration.`
```

- [ ] **Step 3: Search for stale current references**

Run:

```bash
rg "apply-local|agent-bar-omarchy-(setup|remove|uninstall|refresh)" README.md docs/README.md docs/commands.md docs/integration.md docs/runtime.md docs/troubleshooting.md docs/waybar-contract.md AGENTS.md CLAUDE.md src tests
```

Expected: no stale references except `scripts/agent-bar-omarchy-open-terminal` may mention `refresh` in comments only if still accurate for right-click behavior.

## Task 5: Final Verification

**Files:**
- All changed files

- [ ] **Step 1: Run focused verification**

Run:

```bash
bun test tests/cli.test.ts
bun run typecheck
git diff --check
```

Expected: all commands pass.

- [ ] **Step 2: Review diff for accidental dependency churn**

Run:

```bash
git diff -- package.json
git diff --stat
```

Expected: `package.json` only adds scripts needed for onboarding unless the user explicitly requested dependency updates.

- [ ] **Step 3: Report**

Report:

- what changed
- what was verified
- any intentionally unverified live-desktop behavior
- suggested follow-up improvements

## Self-Review

- Spec coverage: package script, complete command removal, docs, tests, and verification are covered.
- Placeholder scan: no placeholders remain.
- Type consistency: command names match existing `CliOptions.command` literals and existing script names.
