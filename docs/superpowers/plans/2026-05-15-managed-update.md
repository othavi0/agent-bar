# Managed Update Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `~/.agent-bar` the documented install path and make `agent-bar-omarchy update` a guided managed-install update that discards local install edits, pulls upstream, updates dependencies when needed, and re-runs setup.

**Architecture:** Keep CLI parsing unchanged and refactor behavior behind existing `setup` and `update` commands. `setup` exposes a reusable `runSetup()` entry point so update can re-apply integration without a second prompt. `update` gets a testable orchestration function with injected command runner, confirmation, setup callback, install root, and progress events.

**Tech Stack:** Bun runtime, TypeScript ESM, `@clack/prompts`, `bun:test`, Biome.

---

## File Structure

- Modify `README.md`: use `~/.agent-bar` installation and document update behavior.
- Modify `docs/commands.md`: describe managed destructive update.
- Modify `src/setup.ts`: extract `runSetup({ confirm, clearScreen })`.
- Create `src/tui/terminal-ui.ts`: small shared helpers for command headers, key/value notes, and warnings.
- Modify `src/update.ts`: implement managed install update with dependency-change detection, local-change discard, setup re-run, and visual progress.
- Create `tests/update.test.ts`: fake command runner tests for update orchestration.

## Task 1: Update Tests First

**Files:**
- Create: `tests/update.test.ts`

- [ ] **Step 1: Add tests for managed root guard**

Test `runManagedUpdate()` with `repoRoot: '/tmp/dev/agent-bar-omarchy'` and `installRoot: '/home/test/.agent-bar'`.

Expected result:

```ts
expect(result.status).toBe('wrong-root');
expect(commands).toEqual([]);
```

- [ ] **Step 2: Add tests for destructive update sequence**

Test a managed root with upstream changes and local changes.

Expected command sequence:

```ts
[
  ['git', ['rev-parse', '--git-dir']],
  ['git', ['rev-parse', '--short', 'HEAD']],
  ['git', ['branch', '--show-current']],
  ['git', ['fetch', '--prune', 'origin']],
  ['git', ['rev-parse', '--abbrev-ref', '--symbolic-full-name', '@{u}']],
  ['git', ['log', '--oneline', 'HEAD..origin/master', '-10']],
  ['git', ['status', '--short']],
  ['git', ['diff', '--name-only', 'HEAD', 'origin/master', '--', 'package.json', 'bun.lock', 'bun.lockb']],
  ['git', ['reset', '--hard', 'origin/master']],
  ['git', ['clean', '-fd']],
  ['bun', ['install']],
]
```

Also assert the fake setup callback ran once.

- [ ] **Step 3: Add tests for dependency skip**

When the dependency diff is empty and `node_modules` exists, assert `bun install`
is not run but setup still runs after reset.

- [ ] **Step 4: Add tests for no-op**

When there are no upstream commits and no local changes, assert the result is
`up-to-date`, no reset happens, and setup is not called.

## Task 2: Refactor Setup

**Files:**
- Modify: `src/setup.ts`

- [ ] **Step 1: Add setup options**

Introduce:

```ts
export interface SetupOptions {
  confirm?: boolean;
  clearScreen?: boolean;
}
```

- [ ] **Step 2: Extract reusable setup function**

Move the current `main()` body into:

```ts
export async function runSetup(options: SetupOptions = {}): Promise<boolean>
```

Defaults:

```ts
const shouldConfirm = options.confirm ?? true;
const shouldClearScreen = options.clearScreen ?? true;
```

Return `false` when cancelled and `true` after successful setup.

- [ ] **Step 3: Keep CLI entry behavior**

Make `main()` call:

```ts
await runSetup({ confirm: true, clearScreen: true });
```

## Task 3: Terminal UI Helper

**Files:**
- Create: `src/tui/terminal-ui.ts`

- [ ] **Step 1: Add shared presentation helpers**

Create helpers that reuse existing colors:

```ts
import * as p from '@clack/prompts';
import { APP_NAME } from '../app-identity';
import { colorize, oneDark, semantic } from './colors';

export function printCommandHeader(command: string, subtitle?: string): void {
  p.intro(colorize(`${APP_NAME} ${command}`, oneDark.blue));
  if (subtitle) p.log.info(colorize(subtitle, semantic.subtitle));
}

export function formatKeyValue(key: string, value: string): string {
  return `${colorize(`${key}:`, semantic.subtitle)} ${colorize(value, oneDark.text)}`;
}

export function printKeyValues(title: string, rows: Array<[string, string]>): void {
  p.note(rows.map(([key, value]) => formatKeyValue(key, value)).join('\n'), colorize(title, semantic.title));
}

export function printWarning(title: string, lines: string[]): void {
  p.note(lines.map((line) => colorize(line, semantic.warning)).join('\n'), colorize(title, semantic.warning));
}
```

## Task 4: Managed Update Implementation

**Files:**
- Modify: `src/update.ts`

- [ ] **Step 1: Add exported types**

Define `CommandResult`, `CommandRunner`, `UpdateEvent`, `ManagedUpdateOptions`,
and `ManagedUpdateResult` in `src/update.ts`.

- [ ] **Step 2: Add root guard**

Implement:

```ts
export function isManagedInstallRoot(repoRoot: string, installRoot: string = join(homedir(), '.agent-bar')): boolean {
  return resolve(repoRoot) === resolve(installRoot);
}
```

- [ ] **Step 3: Implement `runManagedUpdate()`**

The function must:

- return `wrong-root` before running commands outside the install root
- verify git repo
- fetch
- resolve upstream with fallback to `origin/master`
- collect commits/status/dependency diff
- return `up-to-date` without mutation when clean and current
- call confirmation before reset
- reset hard and clean
- run `bun install` only when dependency files changed or `node_modules` is missing
- call setup callback exactly once after mutation

- [ ] **Step 4: Rewrite `main()` as visual wrapper**

Use `printCommandHeader`, `printKeyValues`, `printWarning`, `p.spinner()`, and
`p.confirm()` to show progress and warnings. Do not shell out to
`agent-bar-omarchy setup`; dynamically import `runSetup()` and call it with
`confirm: false, clearScreen: false`.

## Task 5: Docs

**Files:**
- Modify: `README.md`
- Modify: `docs/commands.md`

- [ ] **Step 1: Change install path**

Use:

```bash
git clone git@github.com:othavioquiliao/agent-bar-omarchy.git ~/.agent-bar
cd ~/.agent-bar
bun run setup
```

- [ ] **Step 2: Document update**

Say that `agent-bar-omarchy update` is for the managed `~/.agent-bar`
installation and discards local changes there before re-applying setup.

## Task 6: Verification

**Files:**
- All changed files

- [ ] **Step 1: Run targeted tests**

```bash
bun test tests/update.test.ts tests/cli.test.ts
```

- [ ] **Step 2: Run broad checks**

```bash
bun run typecheck
bun run lint
git diff --check
```

- [ ] **Step 3: Do not run live update/setup**

Do not run `agent-bar-omarchy update` or `agent-bar-omarchy setup` as
verification because they mutate the live Waybar installation.

## Self-Review

- Spec coverage: install path, destructive managed update, setup re-use, visual feedback, tests, and docs are covered.
- Placeholder scan: no placeholders remain.
- Type consistency: command names and option names match the proposed files.
