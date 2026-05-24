# Install Pollution Prevention Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Impedir que `bun add @noctuacore/agent-bar` (sem `-g`) polua `$HOME` com `package.json`/lockfile, e dar um caminho de cleanup quando acontecer.

**Architecture:** Três frentes independentes. (A) README usa snippet imune a esquecimento de `-g`. (B) Script `preinstall` no `package.json` aborta install local com `INIT_CWD === $HOME`. (C) Novo comando `agent-bar doctor` (em `src/doctor.ts`) detecta lixo órfão em `$HOME` e oferece cleanup; `agent-bar setup` ganha hint não-destrutivo apontando pro doctor quando lixo está presente.

**Tech Stack:** Bun, TypeScript strict, `@clack/prompts`, `bun:test`. Sem deps novas.

**Spec:** `docs/superpowers/specs/2026-05-23-install-pollution-prevention-design.md`

---

## File Map

- **Create**
  - `src/doctor.ts` — scan + cleanup logic + interactive `main()`.
  - `tests/doctor.test.ts` — unit tests pra `scan` e `runDoctor`.
- **Modify**
  - `package.json` — adicionar `scripts.preinstall`.
  - `tests/package.test.ts` — assertar shape do `preinstall`.
  - `src/cli.ts` — adicionar `'doctor'` ao union `command` + case no `parseArgs`.
  - `tests/cli.test.ts` — assertar que `doctor` é reconhecido.
  - `src/index.ts` — dispatcher chama `src/doctor.ts` no comando `doctor`.
  - `src/setup.ts` — após `p.outro` de sucesso, printar hint se `scan` detectar lixo.
  - `README.md` — snippet `cd /tmp && bun add -g ...` + nota de troubleshooting.
  - `docs/commands.md` — entry para `doctor`.
  - `CHANGELOG.md` — `[Unreleased]` com as 3 frentes.

Ordem das tasks vai do mais isolado pro mais integrado, pra commits pequenos e independentes.

---

## Task 1: Preinstall guard

**Files:**
- Modify: `package.json` (campo `scripts`)
- Test: `tests/package.test.ts`

- [ ] **Step 1: Escrever os asserts antes de mexer no package.json**

Adicionar em `tests/package.test.ts` um novo `it` dentro do mesmo `describe('npm package contract', ...)`:

```ts
  it('refuses local install in $HOME via preinstall guard', () => {
    const preinstall = (pkg.scripts as Record<string, string | undefined>).preinstall;
    expect(preinstall).toBeDefined();
    expect(preinstall).toContain('INIT_CWD');
    expect(preinstall).toContain('homedir');
    expect(preinstall).toContain('process.exit(1)');
    // Mensagem precisa orientar o usuário pro snippet seguro
    expect(preinstall).toContain('bun add -g @noctuacore/agent-bar');
  });
```

- [ ] **Step 2: Rodar o teste e ver falhar**

Run: `bun test tests/package.test.ts`
Expected: 1 fail — `preinstall` is undefined.

- [ ] **Step 3: Adicionar o `preinstall` no `package.json`**

No bloco `"scripts": { ... }`, adicionar **a primeira chave** (preinstall roda antes de tudo):

```json
"preinstall": "node -e \"const os=require('os');const c=process.env.INIT_CWD||process.cwd();if(c===os.homedir()){console.error('\\n[agent-bar] Install must be global. Local install in $HOME pollutes the home directory.\\n  Run from a safe dir:  cd /tmp && bun add -g @noctuacore/agent-bar\\n');process.exit(1)}\"",
```

Notas:
- Comando inline em `node -e` (não criar arquivo em `scripts/` — CLAUDE.md §1 proíbe TS lá).
- Aspas escapadas pra sobreviver ao JSON parser.
- `INIT_CWD` é setado por npm/bun pro dir de invocação do usuário, mesmo durante lifecycle scripts.

- [ ] **Step 4: Rodar testes do package e ver passar**

Run: `bun test tests/package.test.ts`
Expected: all pass.

- [ ] **Step 5: Smoke manual do guard**

Run:
```bash
INIT_CWD="$HOME" node -e "$(node -e "console.log(JSON.parse(require('fs').readFileSync('package.json')).scripts.preinstall.replace(/^node -e /, ''))")"
```
Expected: exit code 1, mensagem no stderr incluindo `bun add -g @noctuacore/agent-bar`.

Alternativa mais simples:
```bash
INIT_CWD="$HOME" bun install --dry-run 2>&1 | head -20
```
Deveria mostrar a mensagem do guard. Se você está no repo (cwd == repo, não $HOME), o `INIT_CWD` precisa ser forçado pra reproduzir.

- [ ] **Step 6: Commit**

```bash
git add package.json tests/package.test.ts
git commit -m "feat: refuse local install in \$HOME via preinstall"
```

---

## Task 2: Doctor — `scan()` puro (TDD)

**Files:**
- Create: `src/doctor.ts`
- Create: `tests/doctor.test.ts`

`scan` é uma função pura que inspeciona um diretório (parametrizado pra
testes) e retorna o que encontrou. Sem side effects, sem TUI.

- [ ] **Step 1: Escrever os testes de `scan`**

Criar `tests/doctor.test.ts`:

```ts
import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { scan } from '../src/doctor';

function makeHome(): string {
  return mkdtempSync(join(tmpdir(), 'agent-bar-doctor-'));
}

describe('doctor.scan', () => {
  let home: string;

  beforeEach(() => {
    home = makeHome();
  });

  afterEach(() => {
    rmSync(home, { recursive: true, force: true });
  });

  it('returns clean findings when $HOME has nothing relevant', async () => {
    const findings = await scan(home);
    expect(findings.packageJsonOrphan).toBe(false);
    expect(findings.packageJsonMixed).toBe(false);
    expect(findings.nodeModulesDir).toBe(null);
    expect(findings.lockfiles).toEqual([]);
  });

  it('detects orphan package.json with only @noctuacore/agent-bar', async () => {
    writeFileSync(
      join(home, 'package.json'),
      JSON.stringify({ dependencies: { '@noctuacore/agent-bar': '^4.0.0' } }),
    );
    const findings = await scan(home);
    expect(findings.packageJsonOrphan).toBe(true);
    expect(findings.packageJsonMixed).toBe(false);
  });

  it('flags mixed package.json (agent-bar + other deps)', async () => {
    writeFileSync(
      join(home, 'package.json'),
      JSON.stringify({
        dependencies: { '@noctuacore/agent-bar': '^4.0.0', other: '1.0.0' },
      }),
    );
    const findings = await scan(home);
    expect(findings.packageJsonOrphan).toBe(false);
    expect(findings.packageJsonMixed).toBe(true);
  });

  it('ignores package.json that does not mention agent-bar', async () => {
    writeFileSync(join(home, 'package.json'), JSON.stringify({ dependencies: { other: '1.0.0' } }));
    const findings = await scan(home);
    expect(findings.packageJsonOrphan).toBe(false);
    expect(findings.packageJsonMixed).toBe(false);
  });

  it('detects node_modules/@noctuacore/agent-bar', async () => {
    mkdirSync(join(home, 'node_modules', '@noctuacore', 'agent-bar'), { recursive: true });
    const findings = await scan(home);
    expect(findings.nodeModulesDir).toBe(join(home, 'node_modules', '@noctuacore', 'agent-bar'));
  });

  it('lists lockfiles only when package.json is orphan or missing', async () => {
    writeFileSync(join(home, 'bun.lock'), '');
    writeFileSync(join(home, 'package-lock.json'), '{}');
    const orphanLess = await scan(home);
    expect(orphanLess.lockfiles).toEqual([
      join(home, 'bun.lock'),
      join(home, 'package-lock.json'),
    ]);

    writeFileSync(
      join(home, 'package.json'),
      JSON.stringify({ dependencies: { other: '1.0.0' } }),
    );
    const legit = await scan(home);
    expect(legit.lockfiles).toEqual([]);
  });

  it('considers devDependencies too when classifying package.json', async () => {
    writeFileSync(
      join(home, 'package.json'),
      JSON.stringify({ devDependencies: { '@noctuacore/agent-bar': '^4.0.0' } }),
    );
    const findings = await scan(home);
    expect(findings.packageJsonOrphan).toBe(true);
  });
});
```

- [ ] **Step 2: Rodar e ver falhar (módulo não existe)**

Run: `bun test tests/doctor.test.ts`
Expected: fail — `Cannot find module '../src/doctor'`.

- [ ] **Step 3: Implementar `scan` em `src/doctor.ts`**

Criar `src/doctor.ts` com **só** o necessário pros testes acima:

```ts
#!/usr/bin/env bun

import { existsSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const TARGET_PACKAGE = '@noctuacore/agent-bar';
const LOCKFILE_NAMES = ['bun.lock', 'bun.lockb', 'package-lock.json'] as const;

export interface DoctorFindings {
  packageJsonPath: string | null;
  packageJsonOrphan: boolean;
  packageJsonMixed: boolean;
  nodeModulesDir: string | null;
  lockfiles: string[];
}

interface PackageJsonShape {
  dependencies?: Record<string, string>;
  devDependencies?: Record<string, string>;
}

function readJson(path: string): PackageJsonShape | null {
  try {
    return JSON.parse(readFileSync(path, 'utf8')) as PackageJsonShape;
  } catch {
    return null;
  }
}

function classifyPackageJson(
  pkg: PackageJsonShape | null,
): { orphan: boolean; mixed: boolean } {
  if (!pkg) return { orphan: false, mixed: false };
  const deps = {
    ...(pkg.dependencies ?? {}),
    ...(pkg.devDependencies ?? {}),
  };
  const names = Object.keys(deps);
  if (!names.includes(TARGET_PACKAGE)) return { orphan: false, mixed: false };
  if (names.length === 1) return { orphan: true, mixed: false };
  return { orphan: false, mixed: true };
}

function findNodeModulesDir(home: string): string | null {
  const dir = join(home, 'node_modules', '@noctuacore', 'agent-bar');
  try {
    if (statSync(dir).isDirectory()) return dir;
  } catch {}
  return null;
}

function findLockfiles(home: string, packageJsonClassification: 'orphan' | 'mixed' | 'absent'): string[] {
  if (packageJsonClassification === 'mixed') return [];
  return LOCKFILE_NAMES.map((name) => join(home, name)).filter((p) => existsSync(p));
}

export async function scan(home: string): Promise<DoctorFindings> {
  const packageJsonPath = join(home, 'package.json');
  const pkg = existsSync(packageJsonPath) ? readJson(packageJsonPath) : null;
  const { orphan, mixed } = classifyPackageJson(pkg);
  const classification: 'orphan' | 'mixed' | 'absent' = orphan ? 'orphan' : mixed ? 'mixed' : 'absent';

  return {
    packageJsonPath: pkg ? packageJsonPath : null,
    packageJsonOrphan: orphan,
    packageJsonMixed: mixed,
    nodeModulesDir: findNodeModulesDir(home),
    lockfiles: findLockfiles(home, classification),
  };
}
```

Notas:
- `packageJsonPath` é útil pra Task 3 (cleanup) — testes desta task não checam, mas não atrapalha.
- `findLockfiles` retorna vazio quando `mixed`: usuário tem projeto legítimo, não toca lockfile.
- Quando `package.json` ausente e há lockfile, ainda retorna o lockfile — lixo órfão puro.

- [ ] **Step 4: Rodar testes e ver passar**

Run: `bun test tests/doctor.test.ts`
Expected: all 7 tests pass.

- [ ] **Step 5: Typecheck**

Run: `bun run typecheck`
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add src/doctor.ts tests/doctor.test.ts
git commit -m "feat: add doctor scan for \$HOME pollution"
```

---

## Task 3: Doctor — `runDoctor()` orquestração

**Files:**
- Modify: `src/doctor.ts`
- Modify: `tests/doctor.test.ts`

`runDoctor` decide o status, chama `confirm`, e remove artefatos. Sem TUI ainda — recebe `confirm` como callback injetável.

- [ ] **Step 1: Escrever testes de `runDoctor`**

Adicionar ao `tests/doctor.test.ts` um novo `describe`:

```ts
import { runDoctor } from '../src/doctor';

describe('doctor.runDoctor', () => {
  let home: string;

  beforeEach(() => {
    home = makeHome();
  });

  afterEach(() => {
    rmSync(home, { recursive: true, force: true });
  });

  it('returns clean when $HOME has nothing', async () => {
    const result = await runDoctor({ home, confirm: async () => true });
    expect(result.status).toBe('clean');
    expect(result.removed).toEqual([]);
  });

  it('removes orphan package.json + lockfile + node_modules when confirmed', async () => {
    writeFileSync(
      join(home, 'package.json'),
      JSON.stringify({ dependencies: { '@noctuacore/agent-bar': '^4.0.0' } }),
    );
    writeFileSync(join(home, 'bun.lock'), '');
    mkdirSync(join(home, 'node_modules', '@noctuacore', 'agent-bar'), { recursive: true });

    const result = await runDoctor({ home, confirm: async () => true });

    expect(result.status).toBe('cleaned');
    expect(result.removed.sort()).toEqual(
      [
        join(home, 'package.json'),
        join(home, 'bun.lock'),
        join(home, 'node_modules', '@noctuacore', 'agent-bar'),
      ].sort(),
    );
    expect(existsSync(join(home, 'package.json'))).toBe(false);
    expect(existsSync(join(home, 'bun.lock'))).toBe(false);
    expect(existsSync(join(home, 'node_modules', '@noctuacore', 'agent-bar'))).toBe(false);
  });

  it('on mixed package.json: removes node_modules but keeps package.json + lockfile', async () => {
    writeFileSync(
      join(home, 'package.json'),
      JSON.stringify({
        dependencies: { '@noctuacore/agent-bar': '^4.0.0', other: '1.0.0' },
      }),
    );
    writeFileSync(join(home, 'bun.lock'), '');
    mkdirSync(join(home, 'node_modules', '@noctuacore', 'agent-bar'), { recursive: true });

    const result = await runDoctor({ home, confirm: async () => true });

    expect(result.status).toBe('mixed-only');
    expect(result.removed).toEqual([join(home, 'node_modules', '@noctuacore', 'agent-bar')]);
    expect(existsSync(join(home, 'package.json'))).toBe(true);
    expect(existsSync(join(home, 'bun.lock'))).toBe(true);
  });

  it('returns cancelled when confirm rejects', async () => {
    writeFileSync(
      join(home, 'package.json'),
      JSON.stringify({ dependencies: { '@noctuacore/agent-bar': '^4.0.0' } }),
    );
    const result = await runDoctor({ home, confirm: async () => false });
    expect(result.status).toBe('cancelled');
    expect(result.removed).toEqual([]);
    expect(existsSync(join(home, 'package.json'))).toBe(true);
  });

  it('--dry-run: reports without removing', async () => {
    writeFileSync(
      join(home, 'package.json'),
      JSON.stringify({ dependencies: { '@noctuacore/agent-bar': '^4.0.0' } }),
    );
    const result = await runDoctor({ home, dryRun: true, confirm: async () => true });
    expect(result.status).toBe('cleaned');
    expect(result.removed).toEqual([join(home, 'package.json')]);
    expect(existsSync(join(home, 'package.json'))).toBe(true);
  });

  it('--yes: skips confirm callback', async () => {
    writeFileSync(
      join(home, 'package.json'),
      JSON.stringify({ dependencies: { '@noctuacore/agent-bar': '^4.0.0' } }),
    );
    let confirmCalled = false;
    const result = await runDoctor({
      home,
      yes: true,
      confirm: async () => {
        confirmCalled = true;
        return false;
      },
    });
    expect(confirmCalled).toBe(false);
    expect(result.status).toBe('cleaned');
    expect(existsSync(join(home, 'package.json'))).toBe(false);
  });
});
```

Adicionar import do `existsSync`:

```ts
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
```

- [ ] **Step 2: Rodar e ver falhar**

Run: `bun test tests/doctor.test.ts`
Expected: fail — `runDoctor` not exported.

- [ ] **Step 3: Implementar `runDoctor` em `src/doctor.ts`**

Adicionar ao final de `src/doctor.ts` (antes de qualquer `if (import.meta.main)`):

```ts
import { rmSync, unlinkSync } from 'node:fs';

export type DoctorStatus = 'clean' | 'cancelled' | 'cleaned' | 'mixed-only';

export interface DoctorResult {
  status: DoctorStatus;
  removed: string[];
  findings: DoctorFindings;
}

export interface DoctorOptions {
  home: string;
  dryRun?: boolean;
  yes?: boolean;
  confirm: (findings: DoctorFindings) => Promise<boolean>;
}

function plannedRemovals(findings: DoctorFindings): string[] {
  const items: string[] = [];
  if (findings.packageJsonOrphan && findings.packageJsonPath) items.push(findings.packageJsonPath);
  if (findings.nodeModulesDir) items.push(findings.nodeModulesDir);
  if (!findings.packageJsonMixed) items.push(...findings.lockfiles);
  return items;
}

function performRemoval(path: string): void {
  try {
    rmSync(path, { recursive: true, force: true });
  } catch {
    // fallback for older fs semantics
    try {
      unlinkSync(path);
    } catch {}
  }
}

export async function runDoctor(options: DoctorOptions): Promise<DoctorResult> {
  const findings = await scan(options.home);

  const nothingToDo =
    !findings.packageJsonOrphan &&
    !findings.packageJsonMixed &&
    !findings.nodeModulesDir &&
    findings.lockfiles.length === 0;

  if (nothingToDo) {
    return { status: 'clean', removed: [], findings };
  }

  const approved = options.yes ? true : await options.confirm(findings);
  if (!approved) {
    return { status: 'cancelled', removed: [], findings };
  }

  const removals = plannedRemovals(findings);

  if (!options.dryRun) {
    for (const path of removals) {
      performRemoval(path);
    }
  }

  const status: DoctorStatus = findings.packageJsonMixed && !findings.packageJsonOrphan ? 'mixed-only' : 'cleaned';
  return { status, removed: removals, findings };
}
```

Mover o `import` adicional (`rmSync`, `unlinkSync`) pro topo do arquivo junto com os outros imports de `node:fs`.

- [ ] **Step 4: Rodar testes e ver passar**

Run: `bun test tests/doctor.test.ts`
Expected: all tests pass (7 scan + 6 runDoctor).

- [ ] **Step 5: Typecheck**

Run: `bun run typecheck`
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add src/doctor.ts tests/doctor.test.ts
git commit -m "feat: add doctor cleanup orchestration"
```

---

## Task 4: CLI — adicionar comando `doctor`

**Files:**
- Modify: `src/cli.ts` (union `command` + `parseArgs` switch + help)
- Modify: `tests/cli.test.ts`

- [ ] **Step 1: Escrever asserts no `cli.test.ts`**

Adicionar no `tests/cli.test.ts` (dentro do `describe` existente que cobre `parseArgs`):

```ts
  it('parses the doctor command', () => {
    expect(parseArgs(['doctor']).command).toBe('doctor');
  });

  it('parses --dry-run and --yes flags for doctor', () => {
    const opts = parseArgs(['doctor', '--dry-run', '--yes']);
    expect(opts.command).toBe('doctor');
    expect(opts.dryRun).toBe(true);
    expect(opts.yes).toBe(true);
  });
```

(Se `tests/cli.test.ts` usa imports diferentes, inspecionar o arquivo antes — o
import de `parseArgs` é o mesmo padrão das outras suites.)

- [ ] **Step 2: Rodar e ver falhar**

Run: `bun test tests/cli.test.ts`
Expected: fail — `'doctor'` não está no union ou `dryRun`/`yes` não existem em `CliOptions`.

- [ ] **Step 3: Atualizar `CliOptions` e `parseArgs` em `src/cli.ts`**

Mudar o tipo `command` em `CliOptions` (linha ~7) pra incluir `'doctor'`:

```ts
  command:
    | 'waybar'
    | 'terminal'
    | 'menu'
    | 'status'
    | 'help'
    | 'action-right'
    | 'setup'
    | 'assets-install'
    | 'export-waybar-modules'
    | 'export-waybar-css'
    | 'update'
    | 'uninstall'
    | 'remove'
    | 'doctor';
```

Adicionar dois campos opcionais ao final do interface:

```ts
  dryRun?: boolean;
  yes?: boolean;
```

Em `parseArgs`, adicionar antes do `case 'action-right':`:

```ts
      case 'doctor':
        options.command = 'doctor';
        break;
```

E adicionar dois casos pras flags (perto dos outros `--*`):

```ts
      case '--dry-run':
        options.dryRun = true;
        break;
      case '--yes':
      case '-y':
        options.yes = true;
        break;
```

- [ ] **Step 4: Atualizar help em `showHelp`**

Em `src/cli.ts`, na seção `// Commands` do `showHelp`, adicionar logo após o
`cmdLine('remove', ...)`:

```ts
  console.log(cmdLine('doctor', `Detect & clean ${APP_NAME} leftovers in $HOME`));
```

- [ ] **Step 5: Rodar testes**

Run: `bun test tests/cli.test.ts`
Expected: all pass.

- [ ] **Step 6: Typecheck**

Run: `bun run typecheck`
Expected: no errors (especialmente o switch exaustivo em `src/index.ts` ainda
compila — `doctor` ainda não tem branch lá, mas como o switch é via `if`-chain
de strings, TS não força exaustividade).

- [ ] **Step 7: Commit**

```bash
git add src/cli.ts tests/cli.test.ts
git commit -m "feat: add doctor command to CLI parser"
```

---

## Task 5: Wire `doctor` no dispatcher + `main()` interativa

**Files:**
- Modify: `src/doctor.ts` — adicionar `main()` interativa com clack
- Modify: `src/index.ts` — case `doctor`

- [ ] **Step 1: Adicionar `main()` em `src/doctor.ts`**

No topo de `src/doctor.ts`, adicionar imports:

```ts
import * as p from '@clack/prompts';
import { colorize, semantic } from './tui/colors';
import { printCommandHeader, printKeyValues, printWarning } from './tui/terminal-ui';
```

Adicionar ao final do arquivo (antes do bloco `if (import.meta.main)`):

```ts
function describeFindings(findings: DoctorFindings): Array<[string, string]> {
  const rows: Array<[string, string]> = [];
  if (findings.packageJsonOrphan && findings.packageJsonPath) {
    rows.push(['Orphan package.json', findings.packageJsonPath]);
  }
  if (findings.packageJsonMixed && findings.packageJsonPath) {
    rows.push(['Mixed package.json (kept)', findings.packageJsonPath]);
  }
  if (findings.nodeModulesDir) {
    rows.push(['node_modules', findings.nodeModulesDir]);
  }
  for (const lock of findings.lockfiles) {
    rows.push(['Lockfile', lock]);
  }
  return rows;
}

export async function main(argv: string[] = process.argv.slice(2)): Promise<void> {
  console.clear();
  printCommandHeader('doctor', `Detect & clean ${TARGET_PACKAGE} leftovers in $HOME`);

  const dryRun = argv.includes('--dry-run');
  const yes = argv.includes('--yes') || argv.includes('-y');
  const home = process.env.HOME ?? process.env.USERPROFILE ?? '';

  if (!home) {
    p.log.error(colorize('Could not resolve $HOME', semantic.danger));
    process.exit(1);
  }

  try {
    const result = await runDoctor({
      home,
      dryRun,
      yes,
      confirm: async (findings) => {
        const rows = describeFindings(findings);
        if (rows.length === 0) return false;
        printKeyValues('Found', rows);

        if (findings.packageJsonMixed) {
          printWarning('package.json kept', [
            'It contains other dependencies — likely a real project.',
            'Only node_modules/@noctuacore will be removed.',
          ]);
        }

        const proceed = await p.confirm({
          message: dryRun ? 'Show what would be removed?' : 'Remove the leftovers above?',
          initialValue: true,
        });
        return !p.isCancel(proceed) && proceed;
      },
    });

    if (result.status === 'clean') {
      p.outro(colorize('Nothing to clean — $HOME is tidy.', semantic.good));
      return;
    }

    if (result.status === 'cancelled') {
      p.outro(colorize('Doctor cancelled', semantic.muted));
      return;
    }

    if (dryRun) {
      p.outro(colorize(`Dry run — would remove ${result.removed.length} item(s).`, semantic.good));
      return;
    }

    if (result.status === 'mixed-only') {
      p.outro(colorize(`Removed ${result.removed.length} item(s). package.json kept.`, semantic.good));
      return;
    }

    p.outro(colorize(`Cleaned ${result.removed.length} item(s) from $HOME.`, semantic.good));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    p.log.error(colorize(message, semantic.danger));
    p.outro(colorize('Doctor failed', semantic.danger));
    process.exit(1);
  }
}

if (import.meta.main) {
  main().catch((e) => {
    console.error('Doctor failed:', e);
    process.exit(1);
  });
}
```

- [ ] **Step 2: Adicionar case em `src/index.ts`**

Em `src/index.ts`, adicionar antes do `// Handle update` block:

```ts
  // Handle doctor
  if (options.command === 'doctor') {
    const { main: doctorMain } = await import('./doctor');
    await doctorMain(process.argv.slice(2));
    process.exit(0);
  }
```

- [ ] **Step 3: Smoke manual**

Run em `$HOME` limpo:
```bash
bun run start doctor
```
Expected: header impresso, "Nothing to clean — $HOME is tidy.", exit 0.

Run com lixo simulado em temp:
```bash
TMP=$(mktemp -d)
echo '{"dependencies":{"@noctuacore/agent-bar":"^4.0.0"}}' > "$TMP/package.json"
HOME="$TMP" bun run start doctor --yes
ls "$TMP"
```
Expected: `package.json` removido, exit 0.

- [ ] **Step 4: Typecheck**

Run: `bun run typecheck`
Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add src/doctor.ts src/index.ts
git commit -m "feat: wire interactive doctor command"
```

---

## Task 6: Hint de doctor no `setup`

**Files:**
- Modify: `src/setup.ts`

- [ ] **Step 1: Importar `scan` em `src/setup.ts`**

No topo:

```ts
import { scan } from './doctor';
```

- [ ] **Step 2: Adicionar hint após `p.outro` de sucesso**

Em `runSetup`, **antes** do `p.outro(colorize('Setup complete', semantic.good))` (linha ~144), inserir:

```ts
    try {
      const findings = await scan(HOME);
      const hasLeftovers =
        findings.packageJsonOrphan ||
        findings.packageJsonMixed ||
        findings.nodeModulesDir !== null ||
        findings.lockfiles.length > 0;
      if (hasLeftovers) {
        p.log.warn(
          colorize(
            `Detected leftover install in $HOME. Run \`${APP_NAME} doctor\` to clean up.`,
            semantic.warning,
          ),
        );
      }
    } catch {
      // Scan must never block setup.
    }
```

- [ ] **Step 3: Smoke manual**

```bash
TMP=$(mktemp -d)
echo '{"dependencies":{"@noctuacore/agent-bar":"^4.0.0"}}' > "$TMP/package.json"
# Não rode `agent-bar setup` real — vai mexer no Waybar.
# Em vez disso, importe e chame scan diretamente:
HOME="$TMP" bun -e "import('./src/doctor').then(m => m.scan(process.env.HOME).then(console.log))"
```
Expected: findings com `packageJsonOrphan: true`.

(Não vamos invocar `setup` ao vivo — CLAUDE.md §1 proíbe mutar desktop como verificação. O comportamento do hint está coberto pela inspeção visual do código + os testes de scan.)

- [ ] **Step 4: Typecheck**

Run: `bun run typecheck`
Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add src/setup.ts
git commit -m "feat: setup hints when \$HOME has leftovers"
```

---

## Task 7: README + docs/commands.md + CHANGELOG

**Files:**
- Modify: `README.md`
- Modify: `docs/commands.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Atualizar `README.md` — seção Install**

Substituir:

```bash
bun add -g @noctuacore/agent-bar
agent-bar setup
```

por:

```bash
cd /tmp && bun add -g @noctuacore/agent-bar && agent-bar setup
```

E adicionar nota logo abaixo do bloco:

```markdown
> Run the command from `/tmp` (or any non-`$HOME` dir) and keep the `-g` flag.
> Without `-g`, `bun add` treats the current directory as a project and writes
> `package.json` + `bun.lock` there. If that happened, run
> `agent-bar doctor` to clean up.
```

Adicionar `agent-bar doctor` à lista em `## Commands`:

```bash
agent-bar doctor      # Detect & clean leftovers in $HOME
```

- [ ] **Step 2: Atualizar `docs/commands.md`**

Adicionar uma seção pro `doctor` (estilo igual ao das outras entries existentes
no arquivo — inspecionar formato antes de escrever):

```markdown
## `doctor`

Detect and clean `@noctuacore/agent-bar` artifacts accidentally written to
`$HOME` by a local install (`bun add` without `-g`).

```bash
agent-bar doctor              # interactive
agent-bar doctor --dry-run    # report without removing
agent-bar doctor --yes        # non-interactive, remove without prompting
```

Removes:
- `~/package.json` only when `@noctuacore/agent-bar` is the *only* dep.
- `~/node_modules/@noctuacore/agent-bar/` always.
- `~/bun.lock`, `~/bun.lockb`, `~/package-lock.json` when `package.json` is
  orphan or absent.

If `~/package.json` has other dependencies, it is preserved — only the
agent-bar copy in `node_modules` is removed.
```

- [ ] **Step 3: Atualizar `CHANGELOG.md`**

Sob `## [Unreleased]` (criar a seção se não existir):

```markdown
### Added
- `agent-bar doctor` command: detects and cleans `@noctuacore/agent-bar`
  leftovers (`package.json`, lockfiles, `node_modules/@noctuacore/`) in `$HOME`
  caused by `bun add` / `npm i` without `-g`.
- `setup` now warns when `$HOME` has leftover install artifacts and points to
  `agent-bar doctor`.

### Changed
- `package.json` ships a `preinstall` script that refuses local install in
  `$HOME` (when `INIT_CWD === os.homedir()`) and instructs the user to run
  `cd /tmp && bun add -g @noctuacore/agent-bar`.
- README install snippet now uses `cd /tmp && bun add -g ...` to remain safe
  if `-g` is accidentally dropped.
```

- [ ] **Step 4: Verificação final**

Run:
```bash
bun test && bun run typecheck && bun run lint
```
Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add README.md docs/commands.md CHANGELOG.md
git commit -m "docs: document doctor command and install pollution prevention"
```

---

## Self-Review (já feito)

- **Spec coverage:**
  - A (README) → Task 7.
  - B (preinstall) → Task 1.
  - C scan + cleanup → Tasks 2, 3.
  - C CLI integration → Task 4, 5.
  - C setup hint → Task 6.
  - Testes enumerados no spec § Testes → cobertos por Tasks 1, 2, 3, 4.
- **Placeholders:** nenhum. Todo step tem código ou comando concreto.
- **Consistency:**
  - `scan(home)` → `Promise<DoctorFindings>` em todas as tasks.
  - `runDoctor(options)` → `Promise<DoctorResult>` (Task 3) consumido em Task 5.
  - Campo `packageJsonPath` introduzido em Task 2, usado em `runDoctor`/`describeFindings` nas Tasks 3 e 5.
  - Flags `--dry-run`/`--yes` introduzidas em Task 4, usadas em Task 5.
  - `TARGET_PACKAGE` const em `doctor.ts` reutilizada no header em Task 5.
