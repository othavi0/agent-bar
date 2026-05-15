# Fase 2 — Remoção total da camada legacy — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remover toda a camada de compatibilidade dos nomes antigos `qbar` e `agent-bar-omarchy` do `agent-bar` (constantes, migração de paths, limpeza de assets Waybar, símlink/bin alias), publicar como `4.0.0`.

**Architecture:** Remoção mecânica ordenada pelo grafo de dependências. As constantes legacy vivem em `src/app-identity.ts` (folha); 6 arquivos as consomem. A ordem das tasks remove os **consumidores** primeiro e as **constantes** por último, para o build (`tsc`) ficar verde a cada commit. `bun run typecheck` é a rede de segurança principal: import legacy órfão falha a compilação; `bun run lint` (Biome) trata import não usado como erro.

**Tech Stack:** TypeScript strict, Bun (`bun:test`), Biome.

---

## Contexto para o engenheiro

O `agent-bar` (monitor de quota LLM para Waybar) teve dois nomes anteriores: `qbar`
e `agent-bar-omarchy`. Esta fase apaga a compatibilidade com ambos.

Convenções: **Bun apenas** (`bun test`, `bun run typecheck`, `bun run lint` — nunca
npm/node). TypeScript strict. Biome: 2 espaços, aspas simples, 120 colunas, **import
não usado = erro**. Commits em Português, Conventional Commits, subject ≤ 50 chars.

Spec de origem: `docs/superpowers/specs/2026-05-15-fase-2-remocao-legacy-design.md`.

**O que NÃO se remove** (não tocar):
- `migrateSettings()` em `src/settings.ts` — migração de **schema** v1→v2.
- A constante `LEGACY_DEFAULT_PROVIDERS` em `src/settings.ts` — é usada por
  `migrateSettings()`; é dado de schema, não identidade legacy.
- Qualquer helper genérico (`removePathIfExists`, `escapeRegex`, etc).

## Grafo de dependências (define a ordem)

```
app-identity.ts (constantes LEGACY_*/QBAR_LEGACY_*)  ← folha
  ├─ config.ts            (CONFIG.paths.<legacy>)
  │    ├─ cache.ts
  │    └─ uninstall.ts
  ├─ settings.ts
  ├─ setup.ts
  ├─ waybar-contract.ts   (funções getLegacy*/cleanupLegacy*)
  │    ├─ waybar-integration.ts
  │    └─ uninstall.ts
  └─ waybar-integration.ts
```

Ordem das tasks: **1** (arquivos independentes) → **2** (cluster Waybar) →
**3** (cluster estado) → **4** (folha: config + app-identity) → **5** (docs) →
**6** (verificação final).

## Estrutura de arquivos

| Arquivo | Ação |
| --- | --- |
| `scripts/agent-bar-omarchy`, `scripts/agent-bar-omarchy-open-terminal` | Deletar |
| `snippets/` (3 arquivos) | Deletar o diretório |
| `package.json` | Remover `bin` alias; `version` → `4.0.0` |
| `src/tui/index.ts` | Corrigir comentário |
| `src/waybar-integration.ts`, `src/waybar-contract.ts`, `src/uninstall.ts` | Remover superfície legacy |
| `src/setup.ts`, `src/cache.ts`, `src/settings.ts` | Remover superfície legacy |
| `src/config.ts`, `src/app-identity.ts` | Remover paths/constantes legacy |
| `tests/waybar-integration.test.ts`, `tests/cache.test.ts`, `tests/settings.test.ts`, `tests/app-identity.test.ts` | Deletar testes de migração legacy |
| `README.md`, `CLAUDE.md`, `AGENTS.md`, `docs/integration.md`, `CHANGELOG.md` | Sincronizar |

---

## Task 1: Arquivos independentes — scripts, snippets, package.json, comentário

Mudanças sem acoplamento com o código TypeScript do core. Os scripts são wrappers
Bash; `snippets/` não é importado por `src/`; o `bin` alias e a versão são metadados.

**Files:**
- Delete: `scripts/agent-bar-omarchy`, `scripts/agent-bar-omarchy-open-terminal`
- Delete: `snippets/` (diretório inteiro)
- Modify: `package.json`, `src/tui/index.ts`

- [ ] **Step 1: Deletar os scripts e snippets legacy**

Run:
```bash
git rm scripts/agent-bar-omarchy scripts/agent-bar-omarchy-open-terminal
git rm -r snippets
```

- [ ] **Step 2: Editar `package.json`**

No bloco `"bin"`, remover a linha `"agent-bar-omarchy": "./scripts/agent-bar-omarchy"`.
O bloco deve ficar:

```json
  "bin": {
    "agent-bar": "./scripts/agent-bar"
  },
```

Alterar `"version": "3.0.0"` para `"version": "4.0.0"`.

- [ ] **Step 3: Corrigir o comentário em `src/tui/index.ts`**

Localizar o comentário que menciona "Omarchy" (algo como
`// Block-style logo inspired by Omarchy branding`) e trocar por:

```typescript
// Block-style logo
```

- [ ] **Step 4: Verificar**

Run: `bun test && bun run typecheck && bun run lint`
Expected: PASS. Atenção: se algum teste (provável `tests/cli.test.ts`) afirmar a
string de versão `3.0.0`, atualizar a asserção para `4.0.0` — e só então a suíte
fica verde.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "chore: remove scripts/snippets legacy e bump v4.0.0"
```

---

## Task 2: Cluster Waybar — `waybar-integration.ts`, `waybar-contract.ts`, `uninstall.ts`

Estes três arquivos formam um cluster: `waybar-integration.ts` e `uninstall.ts`
importam funções legacy de `waybar-contract.ts`. Os três são editados no mesmo
commit para o build não quebrar.

**Files:**
- Modify: `src/waybar-integration.ts`, `src/waybar-contract.ts`, `src/uninstall.ts`
- Modify: `tests/waybar-integration.test.ts`

- [ ] **Step 1: Editar `src/waybar-contract.ts`**

Deletar por completo as 3 funções legacy:
- `getLegacyWaybarAssetPaths` (a função inteira)
- `getQbarLegacyWaybarAssetPaths` (a função inteira)
- `cleanupLegacyWaybarAssets` (a função inteira)

No bloco de import de `./app-identity`, remover os 4 nomes legacy:
`LEGACY_TERMINAL_HELPER_NAME`, `LEGACY_WAYBAR_NAMESPACE`,
`QBAR_LEGACY_TERMINAL_HELPER_NAME`, `QBAR_LEGACY_WAYBAR_NAMESPACE`. O import deve
ficar exatamente:

```typescript
import {
  APP_HIDDEN_CLASS,
  APP_NAME,
  TERMINAL_HELPER_NAME,
  WAYBAR_MODULE_PREFIX,
  WAYBAR_NAMESPACE,
  WAYBAR_SELECTOR_PREFIX,
} from './app-identity';
```

- [ ] **Step 2: Editar `src/waybar-integration.ts` — imports e constantes**

No import de `./app-identity`, manter apenas `APP_NAME`, `BACKUP_SUFFIX`,
`WAYBAR_MODULE_PREFIX`, `WAYBAR_NAMESPACE`; remover `LEGACY_APP_NAME`,
`LEGACY_BACKUP_SUFFIX`, `LEGACY_WAYBAR_MODULE_PREFIX`, `LEGACY_WAYBAR_NAMESPACE`,
`QBAR_LEGACY_APP_NAME`, `QBAR_LEGACY_BACKUP_SUFFIX`,
`QBAR_LEGACY_WAYBAR_MODULE_PREFIX`, `QBAR_LEGACY_WAYBAR_NAMESPACE`. Fica:

```typescript
import { APP_NAME, BACKUP_SUFFIX, WAYBAR_MODULE_PREFIX, WAYBAR_NAMESPACE } from './app-identity';
```

No import de `./waybar-contract`, remover `cleanupLegacyWaybarAssets`. Fica:

```typescript
import {
  exportWaybarCss,
  exportWaybarModules,
  getDefaultWaybarAssetPaths,
  normalizeProviderSelection,
  type WaybarProviderId,
} from './waybar-contract';
```

Deletar as constantes `LEGACY_STYLE_IMPORT` e `QBAR_LEGACY_STYLE_IMPORT` (manter
`APP_STYLE_IMPORT`).

Trocar `MANAGED_MODULE_PREFIXES` para conter só o prefixo atual:

```typescript
const MANAGED_MODULE_PREFIXES = [WAYBAR_MODULE_PREFIX];
```

- [ ] **Step 3: Editar `src/waybar-integration.ts` — funções `backupIfNeeded` e `stripManagedStyleImports`**

Substituir `backupIfNeeded` por (remove as checagens de sufixo legacy):

```typescript
function backupIfNeeded(path: string): void {
  const backupPath = `${path}${BACKUP_SUFFIX}`;
  if (!existsSync(backupPath) && existsSync(path)) {
    copyFileSync(path, backupPath);
  }
}
```

Substituir `stripManagedStyleImports` por (remove os strips de comentário e import
dos namespaces legacy):

```typescript
function stripManagedStyleImports(content: string): string {
  return content
    .replace(new RegExp(`^\\s*\\/\\*\\s*${escapeRegex(APP_NAME)} managed import\\s*\\*\\/\\n?`, 'm'), '')
    .replace(
      new RegExp(`^\\s*@import\\s+url\\((['"])\\./${escapeRegex(WAYBAR_NAMESPACE)}/style\\.css\\1\\);?\\n?`, 'm'),
      '',
    )
    .replace(/^\s*\n/, '');
}
```

- [ ] **Step 4: Editar `src/waybar-integration.ts` — funções legacy e `ensureIncludePath`**

Deletar por completo: `getLegacyWaybarIntegrationPaths`,
`getQbarLegacyWaybarIntegrationPaths`, `getLegacyModuleIDs`, `getQbarLegacyModuleIDs`,
e `getManagedWaybarRoot` (esta fica órfã após os passos seguintes). Manter
`getDefaultWaybarIntegrationPaths` e `getAppModuleIDs`.

`ensureIncludePath` não precisa mais filtrar paths legacy — remover o 3º parâmetro.
Substituir a função inteira por:

```typescript
function ensureIncludePath(content: string, includePath: string): { content: string; changed: boolean } {
  const rewriteResult = rewriteStringArrayProperty(content, 'include', (values) => {
    const next = [...values];
    if (!next.includes(includePath)) {
      next.push(includePath);
    }
    return next;
  });

  if (rewriteResult.found) {
    return { content: rewriteResult.content, changed: rewriteResult.changed };
  }

  const includeProperty = `"include": ${formatStringArray([includePath], '  ')}`;
  return {
    content: insertPropertyIntoFirstObject(content, includeProperty),
    changed: true,
  };
}
```

- [ ] **Step 5: Editar `src/waybar-integration.ts` — `applyWaybarIntegration` e `removeWaybarIntegration`**

Em `applyWaybarIntegration`: remover as linhas que declaram `legacyPaths` e
`qbarLegacyPaths`. A chamada de `ensureIncludePath` passa a ter 2 argumentos:

```typescript
    const includeResult = ensureIncludePath(currentConfig, paths.modulesIncludePath);
```

Remover o bloco `for (const includePath of [legacyPaths..., qbarLegacyPaths...]) { ... }`
que apaga includes legacy, e a chamada `cleanupLegacyWaybarAssets(...)`.

Em `removeWaybarIntegration`: remover as linhas que declaram `legacyPaths` e
`qbarLegacyPaths`. A chamada `removeIncludePaths` passa só o path atual:

```typescript
    const includeResult = removeIncludePaths(currentConfig, [paths.modulesIncludePath]);
```

No loop de `removedIncludes`, manter só os 2 paths atuais:

```typescript
  for (const path of [paths.modulesIncludePath, paths.styleIncludePath]) {
```

- [ ] **Step 6: Editar `src/uninstall.ts`**

No import de `./app-identity`, remover `LEGACY_APP_NAME` e `QBAR_LEGACY_APP_NAME`
(manter `APP_NAME`). No import de `./waybar-contract`, remover
`getLegacyWaybarAssetPaths` e `getQbarLegacyWaybarAssetPaths` (manter
`getDefaultWaybarAssetPaths`).

Deletar as declarações: `legacyDefaults`, `qbarLegacyDefaults`,
`LEGACY_SETTINGS_DIR`, `QBAR_LEGACY_SETTINGS_DIR`, `LEGACY_SYMLINK`,
`QBAR_LEGACY_SYMLINK`.

No `p.note([...])`, trocar a primeira linha
`This removes ${APP_NAME} integration and owned paths, plus legacy ...artifacts:`
por `This removes ${APP_NAME} integration and owned paths:` e remover as linhas de
bullet que referenciam `legacyDefaults.*`, `qbarLegacyDefaults.*`,
`LEGACY_SETTINGS_DIR`, `QBAR_LEGACY_SETTINGS_DIR`, `CONFIG.paths.legacyCache`,
`CONFIG.paths.qbarLegacyCache`, `CONFIG.paths.waybarLegacyCache`,
`CONFIG.paths.waybarQbarLegacyCache`, `LEGACY_SYMLINK`, `QBAR_LEGACY_SYMLINK`.
Os bullets que ficam: `waybarConfigPath`, `waybarStylePath`, `modulesIncludePath`,
`styleIncludePath`, `defaults.waybarDir`, `defaults.terminalScript`, `SETTINGS_DIR`,
`CONFIG.paths.cache`, `APP_SYMLINK`.

No bloco de chamadas `removePathIfExists(...)`, remover as 9 chamadas legacy
(`legacyDefaults.waybarDir`, `legacyDefaults.terminalScript`,
`qbarLegacyDefaults.waybarDir`, `qbarLegacyDefaults.terminalScript`,
`LEGACY_SETTINGS_DIR`, `QBAR_LEGACY_SETTINGS_DIR`, `CONFIG.paths.legacyCache`,
`CONFIG.paths.qbarLegacyCache`, `CONFIG.paths.waybarLegacyCache`,
`CONFIG.paths.waybarQbarLegacyCache`, `LEGACY_SYMLINK`, `QBAR_LEGACY_SYMLINK`).
As que ficam: `defaults.waybarDir`, `defaults.terminalScript`, `SETTINGS_DIR`,
`CONFIG.paths.cache`, `APP_SYMLINK`.

- [ ] **Step 7: Deletar o teste de migração legacy de Waybar**

Em `tests/waybar-integration.test.ts`, deletar o bloco de teste
"migrates legacy agent-bar-omarchy wiring to agent-bar and removes it cleanly"
(o `it`/`describe` inteiro). Se o arquivo importar símbolos legacy que deixaram de
existir (`getLegacyWaybarIntegrationPaths`, `LEGACY_STYLE_IMPORT`, etc.), remover
esses imports também.

- [ ] **Step 8: Verificar**

Run: `bun test && bun run typecheck && bun run lint`
Expected: PASS. O typecheck confirma que nenhum import legacy ficou órfão; o lint
confirma que nenhum import não usado sobrou; a suíte de testes passa (os testes de
contrato Waybar core continuam verdes).

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "refactor: remove legacy da integração Waybar"
```

---

## Task 3: Cluster de estado — `setup.ts`, `cache.ts`, `settings.ts`

Estes três arquivos consomem constantes legacy de `app-identity.ts` (`setup.ts`,
`settings.ts`) ou paths legacy de `config.ts` (`cache.ts`). Removem a superfície
legacy mas as constantes/paths continuam existindo até a Task 4 — build verde.

**Files:**
- Modify: `src/setup.ts`, `src/cache.ts`, `src/settings.ts`
- Modify: `tests/cache.test.ts`, `tests/settings.test.ts`

- [ ] **Step 1: Editar `src/setup.ts`**

No import de `./app-identity`, remover `LEGACY_APP_NAME` e `QBAR_LEGACY_APP_NAME`
(manter `APP_NAME`).

Em `createSymlink()`, remover as declarações `legacyLink`, `legacyTarget`,
`qbarLegacyLink` e os blocos que tentam `unlinkSync`/`symlinkSync` deles. A função
deve ficar:

```typescript
export function createSymlink(): string {
  const localBin = join(HOME, '.local', 'bin');
  const link = join(localBin, APP_NAME);
  const target = join(REPO_ROOT, 'scripts', APP_NAME);

  mkdirSync(localBin, { recursive: true });

  try {
    unlinkSync(link);
  } catch {}

  symlinkSync(target, link);
  return link;
}
```

- [ ] **Step 2: Editar `src/cache.ts`**

Remover: o campo `private legacyCacheDirs: string[];`, o campo
`private migrationAttempted = false;`, o parâmetro `legacyCacheDirs` do construtor,
e a função `migrateLegacyCache()` inteira. Em `ensureDir()`, remover a linha
`await this.migrateLegacyCache();`.

O construtor e `ensureDir` ficam:

```typescript
  constructor(cacheDir: string = CONFIG.paths.cache) {
    this.cacheDir = cacheDir;
  }
```

```typescript
  async ensureDir(): Promise<void> {
    try {
      await mkdir(this.cacheDir, { recursive: true });
    } catch (error) {
      logger.error('Failed to create cache directory', { error, dir: this.cacheDir });
    }
  }
```

Ajustar os imports: a linha 1 `import { cp, mkdir, readdir, rename, rm, unlink } from 'fs/promises';`
deve ficar `import { mkdir, unlink } from 'fs/promises';` (`cp`, `readdir`, `rename`,
`rm` só eram usados por `migrateLegacyCache`). O import `import { APP_NAME } from './app-identity';`
fica órfão (só era usado no `logger.info` da migração) — removê-lo.

- [ ] **Step 3: Editar `src/settings.ts`**

No import de `./app-identity`, remover `LEGACY_APP_NAME` e `QBAR_LEGACY_APP_NAME`
(manter `APP_NAME`).

Remover: a função `migrateLegacySettingsSync()` inteira, a constante
`const attemptedSettingsMigrations = new Set<string>();`, e as 4 chamadas a
`migrateLegacySettingsSync();` (em `loadSettings`, `loadSettingsSync`,
`saveSettings`, `getSettingsPath`).

Simplificar a interface `SettingsPaths` e a função `getSettingsPaths()` — remover
os campos legacy. Ficam:

```typescript
interface SettingsPaths {
  settingsDir: string;
  settingsFile: string;
}
```

```typescript
function getSettingsPaths(): SettingsPaths {
  const xdgConfigHome = process.env.XDG_CONFIG_HOME ?? Bun.env.XDG_CONFIG_HOME ?? join(homedir(), '.config');
  const settingsDir = join(xdgConfigHome, APP_NAME);

  return {
    settingsDir,
    settingsFile: join(settingsDir, 'settings.json'),
  };
}
```

Ajustar os imports de `node:fs`: após remover `migrateLegacySettingsSync` (que usava
`mkdirSync` e `renameSync`), a linha `import { existsSync, mkdirSync, readFileSync, renameSync } from 'node:fs';`
deve ficar `import { existsSync, readFileSync } from 'node:fs';`.

**Não tocar** em `migrateSettings()`, `LEGACY_DEFAULT_PROVIDERS`, nem na lógica de
schema v1→v2.

- [ ] **Step 4: Deletar os testes de migração de path legacy**

Em `tests/cache.test.ts`, deletar o teste
"moves legacy cache files into the new directory" (o `it` inteiro e o `describe`
que o envolve, se ficar vazio).

Em `tests/settings.test.ts`, deletar os 3 testes de migração de path:
"moves agent-bar-omarchy settings into the new namespace",
"still moves old qbar settings into the new namespace", e
"does not overwrite existing new settings when a legacy directory still exists".
**Manter** o teste de migração de schema v1→v2 ("adds Copilot to legacy default
provider settings") — renomeá-lo para "adds Copilot to default providers on v1→v2
schema upgrade" para deixar explícito que é schema, não path.

Se algum desses arquivos de teste importar símbolos que deixaram de existir, ajustar
os imports.

- [ ] **Step 5: Verificar**

Run: `bun test && bun run typecheck && bun run lint`
Expected: PASS. typecheck e lint limpos; a suíte passa (com menos testes — os de
migração de path foram removidos; o teste de schema v1→v2 continua verde).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: remove migração legacy de cache/settings"
```

---

## Task 4: Folha — `config.ts` e `app-identity.ts`

Após as Tasks 2 e 3, nada mais consome os paths legacy de `config.ts` nem as
constantes legacy de `app-identity.ts`. Agora elas são removidas.

**Files:**
- Modify: `src/config.ts`, `src/app-identity.ts`
- Modify: `tests/app-identity.test.ts`

- [ ] **Step 1: Editar `src/config.ts`**

No import de `./app-identity`, remover `LEGACY_APP_NAME` e `QBAR_LEGACY_APP_NAME`.
Fica:

```typescript
import { APP_NAME } from './app-identity';
```

Em `CONFIG.paths`, remover as 6 chaves legacy: `legacyCache`, `qbarLegacyCache`,
`waybarLegacyCache`, `waybarQbarLegacyCache`, `legacyConfig`, `qbarLegacyConfig`.
O início de `paths` deve ficar:

```typescript
  paths: {
    cache: join(XDG_CACHE_HOME, APP_NAME),
    config: join(XDG_CONFIG_HOME, APP_NAME),

    // Provider credential paths
```

(O resto de `paths` — `claude`, `codex`, `amp`, `copilot` — fica intacto.)

- [ ] **Step 2: Editar `src/app-identity.ts`**

Deletar as 16 constantes legacy: `LEGACY_APP_NAME`, `QBAR_LEGACY_APP_NAME`,
`LEGACY_APP_BASE_CLASS`, `QBAR_LEGACY_APP_BASE_CLASS`, `LEGACY_APP_HIDDEN_CLASS`,
`QBAR_LEGACY_APP_HIDDEN_CLASS`, `LEGACY_WAYBAR_NAMESPACE`,
`QBAR_LEGACY_WAYBAR_NAMESPACE`, `LEGACY_WAYBAR_MODULE_PREFIX`,
`QBAR_LEGACY_WAYBAR_MODULE_PREFIX`, `LEGACY_WAYBAR_SELECTOR_PREFIX`,
`QBAR_LEGACY_WAYBAR_SELECTOR_PREFIX`, `LEGACY_TERMINAL_HELPER_NAME`,
`QBAR_LEGACY_TERMINAL_HELPER_NAME`, `LEGACY_BACKUP_SUFFIX`,
`QBAR_LEGACY_BACKUP_SUFFIX`.

O arquivo inteiro deve ficar exatamente:

```typescript
export const APP_NAME = 'agent-bar';
export const APP_WINDOW_TITLE = 'Agent Bar';

export const APP_BASE_CLASS = APP_NAME;
export const APP_HIDDEN_CLASS = `${APP_NAME}-hidden`;

export const WAYBAR_NAMESPACE = APP_NAME;
export const WAYBAR_MODULE_PREFIX = `custom/${WAYBAR_NAMESPACE}-`;
export const WAYBAR_SELECTOR_PREFIX = `#custom-${WAYBAR_NAMESPACE}-`;

export const TERMINAL_HELPER_NAME = `${APP_NAME}-open-terminal`;

export const BACKUP_SUFFIX = `.${APP_NAME}-backup`;
```

- [ ] **Step 3: Deletar o teste de namespace legacy**

Em `tests/app-identity.test.ts`, deletar o bloco de teste
"keeps agent-bar-omarchy as the compatibility namespace" (o `describe`/`it`
inteiro). Remover qualquer import de constante legacy que tenha ficado órfão.

- [ ] **Step 4: Verificar**

Run: `bun test && bun run typecheck && bun run lint`
Expected: PASS. Este é o momento de verdade — se qualquer arquivo ainda referenciava
uma constante/path legacy, o `tsc` falha aqui. Suíte verde, typecheck e lint limpos.

- [ ] **Step 5: Verificar ausência total de resíduo em `src/` e `tests/`**

Run: `grep -rniE "qbar|agent-bar-omarchy|omarchy" src tests`
Expected: nenhuma saída. Se aparecer qualquer ocorrência, removê-la antes de
commitar (não deve sobrar nenhuma referência legacy em código ou teste).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: remove constantes e paths legacy"
```

---

## Task 5: Sincronizar documentação

Atualiza docs para refletir a ausência do legacy e a versão 4.0.0.

**Files:**
- Modify: `README.md`, `CLAUDE.md`, `AGENTS.md`, `docs/integration.md`, `CHANGELOG.md`

- [ ] **Step 1: `README.md`**

Trocar o título `<h1 align="center">Agent Bar Omarchy</h1>` por
`<h1 align="center">Agent Bar</h1>`. Verificar o resto do arquivo: qualquer menção
a `agent-bar-omarchy` ou `qbar` deve ser removida ou corrigida para `agent-bar`.

- [ ] **Step 2: `CLAUDE.md`**

Corrigir:
- Título `# agent-bar-omarchy — Claude Code` → `# agent-bar — Claude Code`.
- A linha com `./scripts/agent-bar-omarchy` → usar `./scripts/agent-bar`.
- A linha com `agent-bar-omarchy setup`/`uninstall`/etc → `agent-bar setup` etc.
- A menção a `~/.config/agent-bar-omarchy` → `~/.config/agent-bar`.
- Remover por completo a linha sobre compatibilidade `qbar` ("qbar is legacy
  migration/removal compatibility only...").

- [ ] **Step 3: `docs/integration.md`**

Remover (ou reescrever) o parágrafo que descreve a renomeação a partir de
`agent-bar-omarchy` / a migração legacy de setup. Após esta fase não há migração de
nome — o texto deve descrever apenas o modelo de ownership atual de `agent-bar`.
Verificar também se `docs/integration.md` (ou qualquer arquivo em `docs/`) referencia
o diretório `snippets/`; se sim, remover a referência (o diretório não existe mais).

- [ ] **Step 4: `AGENTS.md` — correções pontuais**

(O rewrite completo do AGENTS.md fica para a Fase 3; aqui só se corrige o que ficou
factualmente errado.)
- Na tabela de "Runtime and Owned Paths", remover a linha
  `~/.local/bin/agent-bar-omarchy | Compatibility symlink created by setup`.
- Reescrever a seção "Legacy Policy": remover toda afirmação de que `qbar` e
  `agent-bar-omarchy` são compatibilidade suportada (deixou de ser verdade).
  Manter a orientação de **não reintroduzir** superfícies removidas (Antigravity,
  `llm-usage`, acoplamento a tema Omarchy). Uma forma enxuta:

  ```markdown
  ## Legacy Policy

  The product name and public namespace is **`agent-bar`**. The previous names
  `agent-bar-omarchy` and `qbar` were fully removed in `4.0.0` — do not reintroduce
  them as commands, module IDs, CSS selectors, settings paths, symlinks, or cache
  keys. Historical `CHANGELOG.md` entries that mention them are fine.

  Also do not reintroduce other removed surfaces such as Antigravity, `llm-usage`,
  external theme-repo dependencies, or Omarchy theme coupling. The app is
  theme-agnostic and owns its generated Waybar integration.
  ```
- Corrigir qualquer outra menção a `agent-bar-omarchy` como comando/namespace ativo.

- [ ] **Step 5: `CHANGELOG.md`**

Adicionar uma nova seção no topo (acima da entrada mais recente) documentando a
versão `4.0.0`. Conteúdo mínimo:

```markdown
## [4.0.0]

### Removed
- Removed the `qbar` and `agent-bar-omarchy` compatibility layer entirely:
  legacy identity constants, settings/cache path migration, Waybar legacy-asset
  cleanup, the `agent-bar-omarchy` CLI symlink and `bin` alias, and the `snippets/`
  manual examples.

### Breaking
- The `agent-bar-omarchy` command no longer exists. Installations still using the
  old name must reinstall as `agent-bar`; old settings/cache under the previous
  names are not migrated.
```

Não alterar as entradas históricas existentes (menções a `qbar`/`agent-bar-omarchy`
no histórico são legítimas).

- [ ] **Step 6: Verificar**

Run: `git diff --check && bun test && bun run typecheck && bun run lint`
Expected: PASS. (Mudanças de docs não afetam o build, mas a verificação confirma que
nada foi quebrado.)

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "docs: sincroniza docs com a remoção do legacy"
```

---

## Task 6: Verificação final da fase

**Files:** nenhum (apenas verificação).

- [ ] **Step 1: Suíte completa + typecheck + lint**

Run: `bun test && bun run typecheck && bun run lint`
Expected: PASS — todos os testes verdes, zero erro de tipo, zero issue de lint.

- [ ] **Step 2: Confirmar ausência total de resíduo legacy**

Run: `grep -rniE "qbar|agent-bar-omarchy|omarchy" src tests; echo "--- docs ---"; grep -rniE "qbar|agent-bar-omarchy" README.md CLAUDE.md docs/integration.md`
Expected: nenhuma saída em `src`/`tests`. Em docs, no máximo as entradas históricas
intencionais do `CHANGELOG.md` (que não está nessa lista) — `README.md`, `CLAUDE.md`
e `docs/integration.md` não devem ter nenhuma menção legacy.

- [ ] **Step 3: Confirmar artefatos deletados e versão**

Run: `ls scripts/ snippets/ 2>&1; grep '"version"' package.json; grep -c "agent-bar-omarchy" package.json`
Expected: `scripts/` contém só `agent-bar` e `agent-bar-open-terminal`; `snippets/`
não existe (erro "No such file"); `version` é `4.0.0`; `grep -c` de
`agent-bar-omarchy` em `package.json` retorna `0`.

- [ ] **Step 4: Confirmar histórico**

Run: `git log --oneline -6`
Expected: os 5 commits da fase (`chore: remove scripts/snippets...`,
`refactor: remove legacy da integração Waybar`,
`refactor: remove migração legacy de cache/settings`,
`refactor: remove constantes e paths legacy`,
`docs: sincroniza docs...`) acima do commit do spec.

---

## Self-Review (preenchido pelo autor do plano)

**Cobertura do spec:**
- Deletar `scripts/agent-bar-omarchy*` → Task 1 ✓
- Deletar `snippets/` → Task 1 ✓
- `package.json` bin alias + `version` 4.0.0 → Task 1 ✓
- `src/tui/index.ts` comentário → Task 1 ✓
- `app-identity.ts` 16 constantes → Task 4 ✓
- `config.ts` 6 paths → Task 4 ✓
- `cache.ts` migração → Task 3 ✓
- `settings.ts` path migration (mantendo schema migration) → Task 3 ✓
- `setup.ts` símlinks legacy → Task 3 ✓
- `uninstall.ts` superfície legacy → Task 2 ✓
- `waybar-integration.ts` superfície legacy → Task 2 ✓
- `waybar-contract.ts` funções legacy → Task 2 ✓
- Testes de path migration deletados → Tasks 2, 3, 4 ✓
- Teste de schema migration mantido → Task 3, Step 4 ✓
- Docs (README, CLAUDE.md, AGENTS.md, integration.md, CHANGELOG) → Task 5 ✓
- Verificação de resíduo zero → Task 4 Step 5 + Task 6 Step 2 ✓

**Placeholders:** nenhum — cada remoção nomeia os símbolos exatos; cada transformação
não-trivial mostra o código resultante completo.

**Consistência:** a ordem das tasks respeita o grafo de dependências — consumidores
(Tasks 2, 3) antes da folha (Task 4). Cada task termina com build verde
(`typecheck` + `lint` + `test`). Os nomes de símbolos (`LEGACY_*`, `QBAR_LEGACY_*`,
`getLegacy*`, `migrateLegacy*`) são usados de forma consistente entre tasks.

**Risco conhecido:** ajuste de imports após remoções é guiado por `lint` (import não
usado = erro Biome) e `typecheck`. Os passos já anteciparam os imports que ficam
órfãos (`APP_NAME` em `cache.ts`; `cp`/`readdir`/`rename`/`rm` em `cache.ts`;
`mkdirSync`/`renameSync` em `settings.ts`; `getManagedWaybarRoot` em
`waybar-integration.ts`); se algum outro aparecer, o lint aponta e o implementador
remove.
