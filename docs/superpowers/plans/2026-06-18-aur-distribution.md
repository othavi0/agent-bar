# AUR Distribution (`agent-bar-bin`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship an AUR `-bin` package that installs a `bun build --compile` standalone binary from the GitHub Release, with the supporting agent-bar code changes so a system install works correctly.

**Architecture:** A foundational `isCompiledBinary()` (via the `/$bunfs` VFS marker) gates three behaviors in a compiled binary: asset resolution from `/usr/share/agent-bar`, a PATH-based `appBin`, and a `system` install kind for `update`. CI gains a step (after npm publish) that compiles + attaches the binary tarball. Packaging files live in `packaging/aur/`.

**Tech Stack:** Bun + TypeScript, `bun:test`, Biome, GitHub Actions, Arch `makepkg`/PKGBUILD.

## Global Constraints

- **Bun only.** No Node/npm in runtime or tests.
- **stdout limpo** — logs to stderr.
- **Never `!` non-null assertion** — narrow with guards.
- **Identity constants** from `src/app-identity.ts` (`APP_NAME`).
- **No build in the PKGBUILD** — download release artifact + verify sha256 only.
- **`runner` ≠ `target`**: CI builds on `ubuntu-latest`; the package targets Arch only (`arch=('x86_64')`).
- **Additive**: `isCompiledBinary()` is false in managed/npm/dev → those paths keep current behavior.
- Empirically validated: compiled binary runs without Bun (`--version`=5.2.0), `import.meta.dir`=`/$bunfs/root`, size ~91 MB.
- Spec: `docs/superpowers/specs/2026-06-18-aur-distribution-design.md`.

---

### Task 1: `isCompiledBinary()` foundational helper

**Files:**
- Create: `src/runtime.ts`
- Test: `tests/runtime.test.ts`

**Interfaces:**
- Produces: `isCompiledBinary(): boolean` — true inside a `bun --compile` binary, or when `AGENT_BAR_FORCE_COMPILED=1` (test seam).

- [ ] **Step 1: Write the failing test**

```ts
import { afterEach, describe, expect, it } from 'bun:test';
import { isCompiledBinary } from '../src/runtime';

describe('isCompiledBinary', () => {
  afterEach(() => {
    delete process.env.AGENT_BAR_FORCE_COMPILED;
  });

  it('is false when running via bun (not a compiled binary)', () => {
    expect(isCompiledBinary()).toBe(false);
  });

  it('is true when the compiled-binary override is set', () => {
    process.env.AGENT_BAR_FORCE_COMPILED = '1';
    expect(isCompiledBinary()).toBe(true);
  });
});
```

- [ ] **Step 2: Run, expect fail** — `bun test tests/runtime.test.ts` → FAIL (module not found).

- [ ] **Step 3: Implement** `src/runtime.ts`:

```ts
/**
 * True when running inside a `bun build --compile` standalone binary.
 * Detected via the `$bunfs` virtual-filesystem prefix that `import.meta.dir`
 * carries in a compiled binary (empirically `/$bunfs/root`) — immune to the
 * binary being renamed, symlinked, or found via PATH. `AGENT_BAR_FORCE_COMPILED=1`
 * is a test seam.
 */
export function isCompiledBinary(): boolean {
  if (process.env.AGENT_BAR_FORCE_COMPILED === '1') return true;
  return import.meta.dir.startsWith('/$bunfs');
}
```

- [ ] **Step 4: Run, expect pass** — `bun test tests/runtime.test.ts` → PASS.

- [ ] **Step 5: Commit**
```bash
git add src/runtime.ts tests/runtime.test.ts
git commit -m "feat: isCompiledBinary() via marcador \$bunfs"
```

---

### Task 2: `resolveAssetSourceRoot()` — assets fora do repo

**Files:**
- Modify: `src/waybar-contract.ts` (add `resolveAssetSourceRoot`; default `installWaybarAssets` repoRoot to it)
- Test: `tests/waybar-contract.test.ts`

**Interfaces:**
- Consumes: `isCompiledBinary` (Task 1).
- Produces: `resolveAssetSourceRoot(): string` — dir containing `icons/` and `scripts/`. Throws a clear error under a compiled binary with no assets.

- [ ] **Step 1: Write the failing test** (uses a temp dir with `icons/` to stand in for `/usr/share/agent-bar` via the env override)

```ts
import { mkdtempSync, mkdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { resolveAssetSourceRoot } from '../src/waybar-contract';

it('honors an absolute AGENT_BAR_ASSET_DIR that contains icons/', () => {
  const dir = mkdtempSync(join(tmpdir(), 'ab-assets-'));
  mkdirSync(join(dir, 'icons'), { recursive: true });
  process.env.AGENT_BAR_ASSET_DIR = dir;
  try {
    expect(resolveAssetSourceRoot()).toBe(dir);
  } finally {
    delete process.env.AGENT_BAR_ASSET_DIR;
    rmSync(dir, { recursive: true, force: true });
  }
});

it('throws a clear error under a compiled binary with no assets', () => {
  process.env.AGENT_BAR_FORCE_COMPILED = '1';
  process.env.AGENT_BAR_ASSET_DIR = '/nonexistent-xyz';
  try {
    expect(() => resolveAssetSourceRoot()).toThrow(/Asset directory not found/);
  } finally {
    delete process.env.AGENT_BAR_FORCE_COMPILED;
    delete process.env.AGENT_BAR_ASSET_DIR;
  }
});
```

- [ ] **Step 2: Run, expect fail** — FAIL (function not exported).

- [ ] **Step 3: Implement** in `src/waybar-contract.ts` (import `isCompiledBinary` from `./runtime`; `isAbsolute`/`existsSync` from node):

```ts
const SYSTEM_ASSET_DIR = '/usr/share/agent-bar';

export function resolveAssetSourceRoot(): string {
  const hasIcons = (dir: string) => existsSync(join(dir, 'icons'));

  const envDir = process.env.AGENT_BAR_ASSET_DIR;
  if (envDir && isAbsolute(envDir) && hasIcons(envDir)) return envDir;

  if (hasIcons(SYSTEM_ASSET_DIR)) return SYSTEM_ASSET_DIR;

  if (!isCompiledBinary() && hasIcons(DEFAULT_REPO_ROOT)) return DEFAULT_REPO_ROOT;

  throw new Error(
    'Asset directory not found. Run `agent-bar setup` after installing, or set AGENT_BAR_ASSET_DIR to an absolute path containing icons/.',
  );
}
```
Then change `installWaybarAssets` default: `const repoRoot = options.repoRoot ?? resolveAssetSourceRoot();` (instead of `?? DEFAULT_REPO_ROOT`).

- [ ] **Step 4: Run, expect pass** — `bun test tests/waybar-contract.test.ts` → PASS. Confirm existing asset-install tests still pass (they pass an explicit `repoRoot`, so unaffected).

- [ ] **Step 5: Commit**
```bash
git add src/waybar-contract.ts tests/waybar-contract.test.ts
git commit -m "feat(waybar): resolveAssetSourceRoot p/ install de sistema"
```

---

### Task 3: `appBin` correto no install de sistema

**Files:**
- Modify: `src/waybar-contract.ts` (`getDefaultWaybarAssetPaths`)
- Test: `tests/waybar-contract.test.ts`

**Interfaces:**
- Consumes: `isCompiledBinary` (Task 1).
- Produces: `getDefaultWaybarAssetPaths().appBin` = `'agent-bar'` under a compiled binary, else `$HOME/.local/bin/agent-bar`.

- [ ] **Step 1: Write the failing test**

```ts
it('uses a PATH-resolved appBin under a compiled (system) binary', () => {
  process.env.AGENT_BAR_FORCE_COMPILED = '1';
  try {
    expect(getDefaultWaybarAssetPaths().appBin).toBe('agent-bar');
  } finally {
    delete process.env.AGENT_BAR_FORCE_COMPILED;
  }
});

it('uses ~/.local/bin appBin otherwise', () => {
  expect(getDefaultWaybarAssetPaths().appBin).toBe(`$HOME/.local/bin/${APP_NAME}`);
});
```
(Import `APP_NAME` + `getDefaultWaybarAssetPaths`.)

- [ ] **Step 2: Run, expect fail** — FAIL (appBin is always `$HOME/.local/bin/...`).

- [ ] **Step 3: Implement** in `getDefaultWaybarAssetPaths()`:
```ts
    appBin: isCompiledBinary() ? APP_NAME : `$HOME/.local/bin/${APP_NAME}`,
```
(`APP_NAME` is already imported in this file via app-identity.)

- [ ] **Step 4: Run, expect pass** — PASS.

- [ ] **Step 5: Commit**
```bash
git add src/waybar-contract.ts tests/waybar-contract.test.ts
git commit -m "feat(waybar): appBin via PATH no install de sistema"
```

---

### Task 4: `update` ciente de install de sistema (`'system'` kind)

**Files:**
- Modify: `src/update.ts` (`InstallKind`, `detectInstallKind`, `main`)
- Test: `tests/update.test.ts`

**Interfaces:**
- Consumes: `isCompiledBinary` (Task 1).
- Produces: `detectInstallKind(...)` returns `'system'` under a compiled binary; `main()` prints package-manager guidance and aborts without `bun add -g`.

- [ ] **Step 1: Write the failing test**

```ts
it("detects 'system' install under a compiled binary", () => {
  process.env.AGENT_BAR_FORCE_COMPILED = '1';
  try {
    expect(detectInstallKind('/whatever')).toBe('system');
  } finally {
    delete process.env.AGENT_BAR_FORCE_COMPILED;
  }
});
```
(Import `detectInstallKind`.)

- [ ] **Step 2: Run, expect fail** — FAIL (returns 'npm').

- [ ] **Step 3: Implement** in `src/update.ts`:
- Add `'system'` to `InstallKind`: `export type InstallKind = 'managed-git' | 'dev-git' | 'npm' | 'system';`
- At the top of `detectInstallKind`, before the `.git` check:
```ts
  if (isCompiledBinary()) return 'system';
```
(import `isCompiledBinary` from `./runtime`.)
- In `main()`, after computing `installKind`, before the `dev-git` branch:
```ts
  if (installKind === 'system') {
    p.log.info(colorize('Installed as a system package (standalone binary).', semantic.subtitle));
    p.log.info(colorize('Update it with your package manager, e.g. `paru -Syu agent-bar-bin`.', semantic.subtitle));
    p.outro(colorize('Nothing to do here', semantic.muted));
    return;
  }
```

- [ ] **Step 4: Run, expect pass** — `bun test tests/update.test.ts` → PASS. Confirm existing managed/npm/dev tests still pass (they don't set the override, so `isCompiledBinary()` is false).

- [ ] **Step 5: Commit**
```bash
git add src/update.ts tests/update.test.ts
git commit -m "feat(update): kind 'system' orienta gerenciador de pacotes"
```

---

### Task 5: CI — compila e anexa o binário no release

**Files:**
- Modify: `.github/workflows/publish.yml`

No unit test (CI). Verify by running the build commands locally + reviewing the workflow.

- [ ] **Step 1: Add steps after "Publish to npm"** in `publish.yml` (same `ubuntu-latest` job, sequential):

```yaml
      - name: Build standalone binary
        run: |
          VERSION=$(jq -r .version package.json)
          bun build --compile --target=bun-linux-x64 --minify --outfile agent-bar src/index.ts
          mkdir -p pkg/scripts pkg/icons
          cp agent-bar pkg/
          cp -r icons/. pkg/icons/
          cp scripts/agent-bar-open-terminal pkg/scripts/
          cp LICENSE pkg/
          tar czf "agent-bar-${VERSION}-x86_64.tar.gz" -C pkg .
          sha256sum "agent-bar-${VERSION}-x86_64.tar.gz" | tee "agent-bar-${VERSION}-x86_64.tar.gz.sha256"

      - name: Attach binary to release
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          VERSION=$(jq -r .version package.json)
          gh release upload "v${VERSION}" \
            "agent-bar-${VERSION}-x86_64.tar.gz" \
            "agent-bar-${VERSION}-x86_64.tar.gz.sha256" \
            --clobber
```

- [ ] **Step 2: Verify the build commands locally** (no Bun in PATH not required here — just that the recipe works):
```bash
VERSION=$(jq -r .version package.json)
bun build --compile --target=bun-linux-x64 --minify --outfile /tmp/agent-bar-ci src/index.ts
mkdir -p /tmp/pkg/scripts /tmp/pkg/icons
cp /tmp/agent-bar-ci /tmp/pkg/agent-bar && cp -r icons/. /tmp/pkg/icons/ && cp scripts/agent-bar-open-terminal /tmp/pkg/scripts/ && cp LICENSE /tmp/pkg/
tar czf "/tmp/agent-bar-${VERSION}-x86_64.tar.gz" -C /tmp/pkg .
tar tzf "/tmp/agent-bar-${VERSION}-x86_64.tar.gz"   # expect: ./agent-bar, ./icons/*, ./scripts/agent-bar-open-terminal, ./LICENSE
sha256sum "/tmp/agent-bar-${VERSION}-x86_64.tar.gz"
rm -rf /tmp/pkg /tmp/agent-bar-ci /tmp/agent-bar-*.tar.gz
```
Expected: tarball lists the binary + icons + helper + LICENSE.

- [ ] **Step 3: Commit**
```bash
git add .github/workflows/publish.yml
git commit -m "ci: compila e anexa binário standalone ao release"
```

---

### Task 6: Packaging — PKGBUILD, `.install`, `.SRCINFO`, version guard

**Files:**
- Create: `packaging/aur/PKGBUILD`, `packaging/aur/agent-bar-bin.install`, `packaging/aur/.SRCINFO`
- Modify: `package.json` (`release:check` gains a pkgver-sync guard)
- Test: `tests/package.test.ts` (assert the guard is wired)

- [ ] **Step 1: Create `packaging/aur/PKGBUILD`** (exact content from the spec §6; `pkgver=5.2.0`). The committed template uses a clearly-invalid placeholder `sha256sums=('REPLACE_AT_RELEASE')` — **never `'SKIP'`** (SKIP disables verification, defeating the supply-chain guarantee). The real hash is filled from the release `.sha256` before the first AUR submission.

- [ ] **Step 2: Create `packaging/aur/agent-bar-bin.install`** (spec §7: post_install / post_upgrade messages).

- [ ] **Step 3: Add the version-sync guard** to `package.json` `release:check`:
```
"release:check": "bun run check:pkgver && bun test && bun run typecheck && bun run lint && bun run build && bun pm pack --dry-run --ignore-scripts",
"check:pkgver": "grep -q \"^pkgver=$(jq -r .version package.json)$\" packaging/aur/PKGBUILD || { echo 'PKGBUILD pkgver != package.json version'; exit 1; }",
```

- [ ] **Step 4: Generate `.SRCINFO`** (Arch tooling, runs locally — parses PKGBUILD, no build):
```bash
cd packaging/aur && makepkg --printsrcinfo > .SRCINFO && cd -
```

- [ ] **Step 5: Write a test** asserting the guard exists (so it can't be silently dropped) in `tests/package.test.ts`:
```ts
it('guards PKGBUILD pkgver against package.json drift', () => {
  expect(pkg.scripts['release:check']).toContain('check:pkgver');
  expect(pkg.scripts['check:pkgver']).toContain('packaging/aur/PKGBUILD');
});
```

- [ ] **Step 6: Verify** — `bun run check:pkgver` exits 0 (pkgver 5.2.0 == version 5.2.0); `bun test tests/package.test.ts` passes; `makepkg --printsrcinfo` parses cleanly (and `namcap packaging/aur/PKGBUILD` if available — warnings only).

- [ ] **Step 7: Commit**
```bash
git add packaging/aur/ package.json tests/package.test.ts
git commit -m "build: PKGBUILD agent-bar-bin + guard de pkgver"
```

---

### Task 7: Docs + handoff

**Files:**
- Modify: `docs/runtime.md` (system-install path + assets in `/usr/share/agent-bar`), `docs/commands.md` (update behavior: a system install points to the package manager), `README.md` (AUR install option)

- [ ] **Step 1: README** — add an AUR install option under Install: `paru -S agent-bar-bin` (or `yay -S`), then `agent-bar setup`.

- [ ] **Step 2: runtime.md** — document the system install: binary at `/usr/bin/agent-bar`, assets at `/usr/share/agent-bar/`, `update` defers to the package manager.

- [ ] **Step 3: commands.md** — note that on a system (AUR) install, `agent-bar update` directs to the package manager.

- [ ] **Step 4: Verify** — `git diff --check`.

- [ ] **Step 5: Commit**
```bash
git add README.md docs/runtime.md docs/commands.md
git commit -m "docs: opção de install AUR + update de sistema"
```

---

### Final verification (after all tasks)

- [ ] `bun test && bun run typecheck && bun run lint` — all green.
- [ ] Compiled smoke (with `AGENT_BAR_ASSET_DIR` pointing at the repo): build the binary, run `--version`, `--provider claude`, and `setup` in a temp `XDG_CONFIG_HOME` — confirm it reads assets and writes `exec: agent-bar` in the generated module.
- [ ] `/code-review master`.

### Post-implementation (release + AUR — manual)

Not code tasks; the maintainer does these after merge:
1. Cut a release (bump version, CHANGELOG, tag) → CI builds + attaches `agent-bar-<ver>-x86_64.tar.gz` + `.sha256`.
2. Update `packaging/aur/PKGBUILD` `pkgver` + `sha256sums` (from the `.sha256`), regenerate `.SRCINFO`, commit.
3. **Handoff (your AUR account):** push `PKGBUILD` + `.install` + `.SRCINFO` to `ssh://aur@aur.archlinux.org/agent-bar-bin.git`. Requires an AUR account + registered SSH key.
