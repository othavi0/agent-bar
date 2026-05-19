# Update npm/Bun Support — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `agent-bar update` detect the installation type and update the correct install — the npm/Bun global package for npm installs, the git checkout for the legacy `~/.agent-bar` flow.

**Architecture:** Add a pure `detectInstallKind` function that classifies the running install into `managed-git`, `dev-git`, or `npm` by checking for `.git` directly in the repo root. Add `runNpmUpdate` (mirrors the injectable-hook style of the existing `runManagedUpdate`). The `main()` entry point detects the kind and routes: `npm` runs `bun add -g` + setup after confirmation, `dev-git` refuses with guidance, `managed-git` keeps the existing git flow untouched.

**Tech Stack:** Bun, TypeScript, `@clack/prompts` for terminal UI, `bun test` for tests.

**Spec:** `docs/superpowers/specs/2026-05-19-update-npm-support-design.md`

---

### Task 1: Install-kind detection

**Files:**
- Modify: `src/update.ts` (add `InstallKind` type + `detectInstallKind`, near `isManagedInstallRoot` at line 89)
- Test: `tests/update.test.ts`

- [ ] **Step 1: Write the failing test**

Add to `tests/update.test.ts`. Update the import at the top to include the new symbols:

```typescript
import { type CommandRunner, detectInstallKind, runManagedUpdate, runNpmUpdate } from '../src/update';
```

Add this `describe` block after the existing `describe('runManagedUpdate', ...)` block:

```typescript
describe('detectInstallKind', () => {
  it('classifies a git checkout at the managed root as managed-git', () => {
    const installRoot = tempInstallRoot();
    mkdirSync(join(installRoot, '.git'));

    expect(detectInstallKind(installRoot, installRoot)).toBe('managed-git');
  });

  it('classifies a git checkout outside the managed root as dev-git', () => {
    const repoRoot = tempInstallRoot();
    mkdirSync(join(repoRoot, '.git'));

    expect(detectInstallKind(repoRoot, '/home/test/.agent-bar')).toBe('dev-git');
  });

  it('classifies a directory without .git as npm', () => {
    const repoRoot = tempInstallRoot();

    expect(detectInstallKind(repoRoot, '/home/test/.agent-bar')).toBe('npm');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bun test tests/update.test.ts`
Expected: FAIL — `detectInstallKind` is not exported / not a function.

- [ ] **Step 3: Write minimal implementation**

In `src/update.ts`, add the `InstallKind` type next to the other exported types (after `ManagedUpdateStatus` at line 38):

```typescript
export type InstallKind = 'managed-git' | 'dev-git' | 'npm';
```

Add the `detectInstallKind` function right after `isManagedInstallRoot` (line 91):

```typescript
export function detectInstallKind(
  repoRoot: string,
  installRoot: string = join(homedir(), '.agent-bar'),
): InstallKind {
  if (!existsSync(join(repoRoot, '.git'))) {
    return 'npm';
  }
  return isManagedInstallRoot(repoRoot, installRoot) ? 'managed-git' : 'dev-git';
}
```

`existsSync` and `join` are already imported at the top of the file.

- [ ] **Step 4: Run test to verify it passes**

Run: `bun test tests/update.test.ts`
Expected: PASS — the 3 new `detectInstallKind` tests pass; the existing `runManagedUpdate` tests still pass.

Note: this step also requires `runNpmUpdate` to exist for the import to resolve. If the import fails, temporarily import only `detectInstallKind` and `runManagedUpdate`, then restore the full import in Task 2 Step 1.

- [ ] **Step 5: Commit**

```bash
git add src/update.ts tests/update.test.ts
git commit -m "feat: detecta tipo de instalação no update"
```

---

### Task 2: npm update flow

**Files:**
- Modify: `src/update.ts` (add npm types + `runNpmUpdate`, after `runManagedUpdate` at line 218)
- Test: `tests/update.test.ts`

- [ ] **Step 1: Write the failing test**

Ensure the import line in `tests/update.test.ts` includes `runNpmUpdate` (set in Task 1 Step 1). Add `writeFileSync` to the `node:fs` import:

```typescript
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
```

Add this helper after `fakeRunner`:

```typescript
function tempNpmRoot(name: string, version: string): string {
  const dir = tempInstallRoot();
  writeFileSync(join(dir, 'package.json'), JSON.stringify({ name, version }));
  return dir;
}
```

Add this `describe` block after the `detectInstallKind` block:

```typescript
describe('runNpmUpdate', () => {
  it('runs bun add -g and setup after the confirmation is accepted', async () => {
    const repoRoot = tempNpmRoot('@noctuacore/agent-bar', '4.0.1');
    let setupCount = 0;
    const { commands, run } = fakeRunner({});

    const result = await runNpmUpdate({
      repoRoot,
      runCommand: run,
      runSetup: async () => {
        setupCount += 1;
      },
      confirmNpm: async (summary) => {
        expect(summary.packageName).toBe('@noctuacore/agent-bar');
        expect(summary.currentVersion).toBe('4.0.1');
        return true;
      },
    });

    expect(result.status).toBe('updated');
    expect(setupCount).toBe(1);
    expect(commands).toEqual([['bun', ['add', '-g', '@noctuacore/agent-bar']]]);
  });

  it('does not run bun add or setup when the confirmation is declined', async () => {
    const repoRoot = tempNpmRoot('@noctuacore/agent-bar', '4.0.1');
    let setupCount = 0;
    const { commands, run } = fakeRunner({});

    const result = await runNpmUpdate({
      repoRoot,
      runCommand: run,
      runSetup: async () => {
        setupCount += 1;
      },
      confirmNpm: async () => false,
    });

    expect(result.status).toBe('cancelled');
    expect(setupCount).toBe(0);
    expect(commands).toEqual([]);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bun test tests/update.test.ts`
Expected: FAIL — `runNpmUpdate` is not exported / not a function.

- [ ] **Step 3: Write minimal implementation**

In `src/update.ts`, add these exported types after `ManagedUpdateResult` (line 46):

```typescript
export interface NpmUpdateSummary {
  packageName: string;
  currentVersion: string;
}

export type NpmUpdateStatus = 'cancelled' | 'updated';

export interface NpmUpdateResult {
  status: NpmUpdateStatus;
  summary: NpmUpdateSummary;
}

export interface NpmUpdateOptions {
  repoRoot?: string;
  runCommand?: CommandRunner;
  runSetup: () => Promise<void>;
  confirmNpm: (summary: NpmUpdateSummary) => Promise<boolean>;
  onEvent?: (event: UpdateEvent) => void;
}
```

Add this function right after `runManagedUpdate` (after line 218):

```typescript
async function readPackageInfo(repoRoot: string): Promise<NpmUpdateSummary> {
  const pkg = (await Bun.file(join(repoRoot, 'package.json')).json()) as {
    name?: string;
    version?: string;
  };
  if (!pkg.name || !pkg.version) {
    throw new Error('package.json is missing name or version');
  }
  return { packageName: pkg.name, currentVersion: pkg.version };
}

export async function runNpmUpdate(options: NpmUpdateOptions): Promise<NpmUpdateResult> {
  const repoRoot = options.repoRoot ?? REPO_ROOT;
  const runCommand = options.runCommand ?? runCmd;
  const onEvent = options.onEvent ?? (() => {});

  onEvent({ type: 'step', message: 'Reading package info...' });
  const summary = await readPackageInfo(repoRoot);

  const approved = await options.confirmNpm(summary);
  if (!approved) {
    return { status: 'cancelled', summary };
  }

  onEvent({ type: 'step', message: 'Updating package with Bun...' });
  await requireCommand(runCommand, repoRoot, 'Update package', 'bun', ['add', '-g', summary.packageName]);

  onEvent({ type: 'step', message: 'Re-applying Waybar integration...' });
  await options.runSetup();
  onEvent({ type: 'success', message: 'Package updated.' });

  return { status: 'updated', summary };
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `bun test tests/update.test.ts`
Expected: PASS — both `runNpmUpdate` tests pass; all earlier tests still pass.

- [ ] **Step 5: Commit**

```bash
git add src/update.ts tests/update.test.ts
git commit -m "feat: adiciona fluxo de update via npm/Bun"
```

---

### Task 3: Route `main()` by install kind

**Files:**
- Modify: `src/update.ts` (`main()` at lines 243-319)

- [ ] **Step 1: Add the dev-git and npm branches to `main()`**

In `src/update.ts`, replace the `printCommandHeader` line inside `main()` (line 245):

```typescript
  printCommandHeader('update', 'Updater for agent-bar');
```

Then, immediately after that line, before the `const spinner = p.spinner();` line (line 247), insert the routing block:

```typescript
  const installKind = detectInstallKind(REPO_ROOT);

  if (installKind === 'dev-git') {
    p.log.error(colorize('This is a development checkout, not a managed install.', semantic.danger));
    p.log.info(colorize('Update it with git directly, e.g. `git pull`.', semantic.subtitle));
    p.outro(colorize('Update aborted', semantic.muted));
    return;
  }

  if (installKind === 'npm') {
    await runNpmUpdateInteractive();
    return;
  }
```

- [ ] **Step 2: Add the `runNpmUpdateInteractive` helper**

In `src/update.ts`, add this function just before `export async function main()` (before line 243):

```typescript
async function runNpmUpdateInteractive(): Promise<void> {
  try {
    const result = await runNpmUpdate({
      runSetup: async () => {
        const { runSetup } = await import('./setup');
        await runSetup({ confirm: false, clearScreen: false });
      },
      confirmNpm: async (summary) => {
        printKeyValues('Package', [
          ['Name', summary.packageName],
          ['Installed', summary.currentVersion],
        ]);
        printWarning('npm update', [
          `This runs \`bun add -g ${summary.packageName}\` and re-applies setup.`,
        ]);

        const proceed = await p.confirm({
          message: 'Update the package with Bun and re-apply setup?',
          initialValue: true,
        });

        return !p.isCancel(proceed) && proceed;
      },
    });

    if (result.status === 'cancelled') {
      p.outro(colorize('Update cancelled', semantic.muted));
      return;
    }

    p.outro(colorize('Package updated. Restart Waybar if modules look stale.', semantic.good));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    p.log.error(colorize(message, semantic.danger));
    p.outro(colorize('Update failed', semantic.danger));
    process.exit(1);
  }
}
```

This reuses `printKeyValues`, `printWarning`, `colorize`, and `semantic`, all already imported at the top of `src/update.ts`.

- [ ] **Step 3: Run typecheck**

Run: `bun run typecheck`
Expected: PASS — no type errors. (`detectInstallKind`, `runNpmUpdate` are now used by `main()`.)

- [ ] **Step 4: Run the full test suite**

Run: `bun test`
Expected: PASS — all tests green, including the existing `runManagedUpdate` suite (unchanged).

- [ ] **Step 5: Smoke-test the dev-git branch**

Run: `./scripts/agent-bar update`
Expected: The command refuses with "This is a development checkout, not a managed install." and "Update it with git directly, e.g. `git pull`." — it must NOT run `bun add -g`, `git reset`, or any mutation. This repo is a development checkout, so this exercises the `dev-git` path safely.

- [ ] **Step 6: Run lint**

Run: `bun run lint`
Expected: PASS — `Checked N files`, no errors.

- [ ] **Step 7: Commit**

```bash
git add src/update.ts
git commit -m "feat: roteia update por tipo de instalação"
```

---

### Task 4: Documentation and help text

**Files:**
- Modify: `README.md` (lines 29-34, 51, 57-59)
- Modify: `docs/commands.md` (the `update` table row + "Update Behavior" section)
- Modify: `docs/runtime.md` ("Package Install" section)
- Modify: `docs/integration.md` ("Update" section, lines 24-29)
- Modify: `docs/troubleshooting.md` ("Update Refuses To Run", lines 40-50)
- Modify: `src/cli.ts` (line 73 help text)
- Modify: `AGENTS.md` (line 121 description)
- Modify: `CHANGELOG.md` (new entry)

- [ ] **Step 1: Update `README.md`**

Replace lines 29-34 (the "To update the npm package" block):

```markdown
To update later, run:

\```bash
agent-bar update
\```

For npm installs this runs `bun add -g @noctuacore/agent-bar` and re-applies
setup. For the legacy `~/.agent-bar` checkout it pulls upstream and re-applies
setup.
```

(Replace the escaped ``` fences with real triple-backtick fences.)

Replace line 51 in the Commands block:

```markdown
agent-bar update      # Update the install (npm package or managed checkout)
```

Replace lines 57-59 (the paragraph starting "`update` is only for the legacy..."):

```markdown
`agent-bar update` detects the install type. For an npm/Bun global install it
updates the package; for the legacy managed `~/.agent-bar` checkout it pulls
upstream. In a development checkout it refuses and tells you to use git.
```

- [ ] **Step 2: Update `docs/commands.md`**

Replace the `agent-bar update` table row:

```markdown
| `agent-bar update` | Update the install: npm package via Bun, or the legacy `~/.agent-bar` checkout. | Global package or `~/.agent-bar`, managed Waybar files |
```

Replace the entire "Update Behavior" section with:

```markdown
## Update Behavior

`agent-bar update` detects the install type and updates accordingly:

- **npm/Bun install:** after confirmation, runs `bun add -g @noctuacore/agent-bar`
  and re-applies setup.
- **Legacy managed `~/.agent-bar` checkout:** must run from `~/.agent-bar`;
  fetches upstream, shows incoming commits and local changes, and after
  confirmation runs `git reset --hard <upstream>` + `git clean -fd`, installs
  dependencies when they changed, and re-applies setup.
- **Development checkout:** refuses and tells you to update with git directly.
```

- [ ] **Step 3: Update `docs/runtime.md`**

In the "Package Install" section, replace the text after the code block so it reads:

```markdown
Bun owns the global package location. `agent-bar setup` still creates
`~/.local/bin/agent-bar` as the stable command path used by generated Waybar
modules. After the initial install, `agent-bar update` updates the package and
re-applies setup.
```

- [ ] **Step 4: Update `docs/integration.md`**

Replace the "Update" section body (lines 24-29) with:

```markdown
## Update

`agent-bar update` detects the install type. For an npm/Bun install it runs
`bun add -g @noctuacore/agent-bar` and re-applies setup. For the legacy
managed `~/.agent-bar` checkout it pulls upstream and re-applies setup. In a
development checkout it refuses and points you to git.
```

- [ ] **Step 5: Update `docs/troubleshooting.md`**

Replace the "Update Refuses To Run" section (lines 40-50) with:

```markdown
## Update Refuses To Run

`agent-bar update` refuses when it runs from a development checkout (a git
checkout that is not `~/.agent-bar`). Update a development checkout with git
directly:

\```bash
git pull
\```

For npm installs and the legacy `~/.agent-bar` checkout, `agent-bar update`
works without extra steps.
```

(Replace the escaped fences with real triple-backtick fences.)

- [ ] **Step 6: Update `src/cli.ts`**

Replace line 73:

```typescript
  console.log(cmdLine('update', 'Update the install (npm or managed checkout)'));
```

- [ ] **Step 7: Update `AGENTS.md`**

Replace the `src/update.ts` bullet (lines 121-122):

```markdown
- `src/update.ts` — detects the install type and updates it: `bun add -g` for
  npm installs, git pull for the managed `~/.agent-bar` checkout, then re-runs
  setup.
```

- [ ] **Step 8: Update `CHANGELOG.md`**

`CHANGELOG.md` follows Keep a Changelog with `## [version] - date` headings and
`### Added/Changed/Fixed` subsections. There is no `## [Unreleased]` section yet.
Insert one between the header block (ends at line 6) and `## [4.0.1] - 2026-05-18`
(line 8). Add this after line 6, before the blank line preceding `## [4.0.1]`:

```markdown

## [Unreleased]

### Changed

- `agent-bar update` agora detecta instalações npm/Bun e atualiza o pacote
  global com `bun add -g`, em vez de tratar apenas o checkout legado
  `~/.agent-bar`.
```

- [ ] **Step 9: Verify docs build and lint**

Run: `bun run lint`
Expected: PASS — no errors.

Run: `bun test`
Expected: PASS — all tests green.

- [ ] **Step 10: Commit**

```bash
git add README.md docs/commands.md docs/runtime.md docs/integration.md docs/troubleshooting.md src/cli.ts AGENTS.md CHANGELOG.md
git commit -m "docs: atualiza update para refletir suporte a npm"
```

---

## Verification

After all tasks:

1. `bun test` — all green, including the new `detectInstallKind` and `runNpmUpdate` suites.
2. `bun run typecheck` — no errors.
3. `bun run lint` — no errors.
4. `./scripts/agent-bar update` from this checkout — refuses as `dev-git`, runs no mutation.
5. The npm flow is covered by `runNpmUpdate` unit tests (no live `bun add -g` run, since that mutates the global install).

## Notes

- Version bump to `4.1.0` is intentionally NOT part of this plan — it belongs to
  the release/finishing step, after `4.0.1` is published.
- Do not run `agent-bar setup`, `update`, or any live-mutating command beyond
  the safe `dev-git` smoke test without explicit user approval.
