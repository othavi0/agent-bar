# Fase 3b — Consolidação — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fechar o backlog de limpezas do mutirão e adicionar testes de alto valor — sem alterar comportamento observável, exceto corrigir um bug latente de XML-escape e remover uma exportação morta.

**Architecture:** Quatro frentes independentes — limpezas pontuais (tipos/dead code), correção do conflito spinner/stdout no install, migração do fallback `generic` dos formatters para o pipeline de builders, e novos testes (teste do `BaseProvider` + fixtures de snapshot mais ricas).

**Tech Stack:** TypeScript strict, Bun (`bun:test`), Biome.

---

## Contexto para o engenheiro

`agent-bar` é um monitor de quota LLM para Waybar (TypeScript/Bun). Spec de origem:
`docs/superpowers/specs/2026-05-15-fase-3b-consolidacao-design.md`.

Convenções: **Bun apenas** (`bun test`, `bun run typecheck`, `bun run lint` — nunca
npm/node). TypeScript strict. Biome (2 espaços, aspas simples, 120 colunas, import
não usado = erro). Commits em Português, Conventional Commits, subject ≤ 50 chars.
Hoje a suíte tem 345 testes, 0 falhas.

O pipeline de formatters (estabelecido na Fase 3a): `formatters/builders/{claude,
codex,amp,copilot,shared}.ts` são builders puros que emitem `Line[]`;
`render-ansi.ts`/`render-pango.ts`/`tui/render-colorize.ts` são os renderers;
`terminal.ts`/`waybar.ts`/`tui/list-all.ts` são dispatchers finos. `render-pango.ts`
faz escape XML automático do conteúdo dos segments.

## Estrutura de arquivos

| Arquivo | Ação |
| --- | --- |
| `src/app-identity.ts` | Remover `APP_WINDOW_TITLE` |
| `src/providers/codex.ts` | `isRecord` guard no branch id=1; import inline → topo |
| `src/providers/amp.ts` | Import inline → topo |
| `src/install.ts` | Corrigir conflito spinner/stdout no `bun add -g` |
| `src/formatters/builders/generic.ts` | **Criar** — builder puro do fallback |
| `src/formatters/terminal.ts` | Rotear `generic` pelo pipeline; remover helpers órfãos |
| `src/formatters/waybar.ts` | Rotear `generic` pelo pipeline; remover helpers órfãos |
| `tests/providers/base.test.ts` | **Criar** — teste do `BaseProvider` |
| `tests/formatters-snapshot.test.ts` | Fixtures novas (`p.account`, builders ricos) |

---

## Task 1: Limpezas de tipos e dead code

Três limpezas pequenas, sem mudança de comportamento, verificadas por
`typecheck`/`lint` e pela suíte continuar verde.

**Files:**
- Modify: `src/app-identity.ts`, `src/providers/codex.ts`, `src/providers/amp.ts`

- [ ] **Step 1: Remover `APP_WINDOW_TITLE` de `src/app-identity.ts`**

Confirmar que nada o importa:
Run: `grep -rn "APP_WINDOW_TITLE" src tests`
Expected: ocorrências SÓ em `src/app-identity.ts`. Se aparecer em qualquer outro
arquivo, PARE e reporte.

Então deletar a linha `export const APP_WINDOW_TITLE = 'Agent Bar';` de
`src/app-identity.ts`.

- [ ] **Step 2: `isRecord` guard no branch id=1 do Codex**

Em `src/providers/codex.ts`, no handler `rl.on('line', ...)` de
`fetchRateLimitsViaAppServer`, o branch `id === 1` faz hoje:

```typescript
if (msg?.id === 1 && msg.result) {
  const accountResult = msg.result as CodexAppServerAccountReadResult;
```

Trocar a condição para usar o guard `isRecord` (a função `isRecord` já existe no
escopo de módulo de `codex.ts`, adicionada na Fase 1):

```typescript
if (msg?.id === 1 && isRecord(msg.result)) {
  const accountResult = msg.result as CodexAppServerAccountReadResult;
```

Comportamento inalterado — `isRecord` apenas estreita `msg.result` (de `unknown`
para `Record<string, unknown>`) antes do cast já existente, igual ao que o branch
`id === 2` já faz.

- [ ] **Step 3: Imports inline de tipo → topo**

Em `src/providers/codex.ts`: localizar os usos de `import('./types').CodexQuotaExtra`
(ex.: `let codexCredits: import('./types').CodexQuotaExtra['extraUsage'] | undefined`
e `const extra: import('./types').CodexQuotaExtra = {}`). Adicionar `CodexQuotaExtra`
ao `import type { ... } from './types'` no topo do arquivo e usar o nome direto
(`CodexQuotaExtra` em vez de `import('./types').CodexQuotaExtra`).

Em `src/providers/amp.ts`: idem para `import('./types').AmpQuotaExtra` — adicionar
`AmpQuotaExtra` ao `import type` do topo e usar o nome direto.

- [ ] **Step 4: Verificar**

Run: `bun test && bun run typecheck && bun run lint`
Expected: PASS — 345 testes, 0 falhas; typecheck e lint limpos. Nenhuma mudança de
comportamento — apenas remoção de dead code e ajuste de tipos.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: remove dead code e endurece tipos"
```

---

## Task 2: Corrigir conflito spinner/stdout no `install.ts`

Em `src/install.ts`, `ensureBunGlobalPackage` mostra um `p.spinner()` do
`@clack/prompts` enquanto roda `bun add -g <pkg>` via `runInteractive`, que faz
`Bun.spawn` com `stdout`/`stderr: 'inherit'`. O subprocesso escreve no terminal e
corrompe a animação do spinner.

**Files:**
- Modify: `src/install.ts`

- [ ] **Step 1: Ler `src/install.ts` e mapear o uso**

Ler o arquivo inteiro. Identificar: a função `runInteractive` (faz `Bun.spawn` com
`stdin`/`stdout`/`stderr: 'inherit'`), e `ensureBunGlobalPackage` (inicia o spinner,
chama `runInteractive('bun', ['add', '-g', pkg])`, e em sucesso/falha para o
spinner). Verificar se `runInteractive` é usado em outros pontos além de
`ensureBunGlobalPackage`.

- [ ] **Step 2: Fazer a instalação capturar a saída em vez de herdar**

A chamada de `bun add -g` que roda sob o spinner deve **capturar** stdout/stderr
(`'pipe'`) em vez de herdar. Implementar assim:

- Se `runInteractive` é usado SÓ por `ensureBunGlobalPackage`: trocar o spawn da
  instalação para capturar a saída. Uma forma direta — substituir a chamada
  `runInteractive('bun', ['add', '-g', pkg])` por um spawn local que captura:

  ```typescript
  const proc = Bun.spawn(['bun', 'add', '-g', pkg], {
    stdout: 'pipe',
    stderr: 'pipe',
  });
  const stdout = await new Response(proc.stdout).text();
  const stderr = await new Response(proc.stderr).text();
  const code = await proc.exited;
  ```

  Manter `code` para a checagem de sucesso existente.

- Se `runInteractive` for usado em outros pontos que genuinamente precisam de
  `inherit` (ex.: fluxos de login interativos), NÃO mudar `runInteractive` —
  apenas a chamada de instalação usa o spawn capturado acima.

No caminho de **falha** (`code !== 0` ou o binário não aparece), antes de
`spinner.error(...)`, emitir a saída capturada para o usuário ver o diagnóstico
(ex.: via `p.log.error` / `p.log.message` com `stderr || stdout`). No caminho de
**sucesso**, a saída capturada é descartada. O spinner permanece limpo durante toda
a instalação.

- [ ] **Step 3: Verificar**

Run: `bun test && bun run typecheck && bun run lint`
Expected: PASS. `install.ts` não tem teste automatizado; a verificação é por
`typecheck`/`lint` e pela suíte não regredir. Confirmar por leitura que: o spinner
não recebe mais saída de subprocesso herdada; o diagnóstico de falha é preservado.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "fix: evita conflito de stdout do spinner no install"
```

---

## Task 3: Migrar o fallback `generic` para o pipeline

`buildGenericTerminal` (`terminal.ts`) e `buildGenericTooltip` (`waybar.ts`) — o
fallback para um provider sem builder dedicado — ainda montam ANSI/Pango à mão. Isso
mantém helpers duplicados em `terminal.ts`/`waybar.ts` (`TOOLTIP_BORDER`,
`escapeXml`, mapas de cor, `renderPangoLocal`, `buildHeader`/`buildFooter` locais) e
`buildGenericTooltip` não escapa XML do conteúdo dinâmico (bug latente).

**Files:**
- Create: `src/formatters/builders/generic.ts`
- Modify: `src/formatters/terminal.ts`, `src/formatters/waybar.ts`

- [ ] **Step 1: Ler o código atual**

Ler: `src/formatters/terminal.ts` (a função `buildGenericTerminal` e os helpers que
ela usa), `src/formatters/waybar.ts` (`buildGenericTooltip` e seus helpers), e
`src/formatters/builders/claude.ts` + `builders/amp.ts` (o padrão de builder puro a
seguir), `builders/types.ts` (`BuildOptions`), `builders/shared.ts`,
`render-ansi.ts`/`render-pango.ts`. Mapear exatamente o conteúdo que o `generic`
renderiza hoje em cada superfície (header com o nome do provider, uma linha de
percentual OU de erro, footer).

- [ ] **Step 2: Criar `src/formatters/builders/generic.ts`**

Criar um builder puro `buildGeneric(quota, options): Line[]`, seguindo o padrão de
`builders/claude.ts` (puro: sem I/O, sem markup — só compõe `Line[]` de `Segment`s,
usando as primitivas de `builders/shared.ts`). Deve reproduzir o conteúdo atual do
fallback `generic`: o header com `displayName`/`provider`, a linha de erro quando
`quota.error`, ou a linha de percentual de `quota.primary` quando disponível, e o
footer. As diferenças entre terminal e Waybar (se houver — ex.: largura de header)
vão para `BuildOptions`, como nos outros builders.

- [ ] **Step 3: Rotear o `generic` pelos renderers nos dois entry points**

Em `terminal.ts`: o dispatcher, para um provider sem builder dedicado, passa a
chamar `buildGeneric(quota, options)` + `renderAnsi`. Remover `buildGenericTerminal`.

Em `waybar.ts`: idem, `buildGeneric(quota, options)` + `renderPango`. Remover
`buildGenericTooltip`.

- [ ] **Step 4: Remover os helpers que ficaram órfãos**

Após rotear o `generic` pelo pipeline, os helpers locais que existiam só para o
fallback (`TOOLTIP_BORDER`, `escapeXml`, `renderPangoLocal`, os mapas de cor locais,
`buildHeader`/`buildFooter` locais — conforme o que cada arquivo tiver) ficam sem
consumidor. Removê-los. `bun run lint` (import/variável não usada = erro) e
`bun run typecheck` apontam o que ficou órfão. **Atenção:** qualquer helper que
ainda tenha consumidor legítimo (ex.: a lógica do texto compacto da pill do Waybar —
`buildText`/`formatProviderForWaybar` e o que elas usam) **permanece** — remover só
o que o lint/typecheck confirmar órfão.

- [ ] **Step 5: Verificar**

Run: `bun test && bun run typecheck && bun run lint`
Expected: PASS. Os snapshots de terminal/Waybar para `claude`/`codex`/`amp`/`copilot`
ficam **inalterados**. O caso `generic` no snapshot (se existir) pode mudar: o
conteúdo deve sair equivalente ao atual, EXCETO que o Pango do `generic` agora
escapa XML corretamente — se houver um snapshot de `generic` e ele mudar só por
escape XML correto, regenerá-lo é intencional; documentar no commit. Se um snapshot
não-`generic` mudar, algo regrediu — PARAR e reportar. typecheck e lint limpos,
0 warnings.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor: migra fallback generic para o pipeline"
```

---

## Task 4: Teste do `BaseProvider`

`src/providers/base.ts` (a classe abstrata `BaseProvider`, com o template-method
`getQuota()`) não tem teste direto. Criar um.

**Files:**
- Create: `tests/providers/base.test.ts`

- [ ] **Step 1: Ler `src/providers/base.ts` e uma suíte de provider existente**

Ler `src/providers/base.ts` (o template `getQuota()`: `buildBase` → gate
`isAvailable()` → `cache.getOrFetch` em try/catch com `logger.error` → `buildQuota`;
os abstratos `fetchRaw`/`buildQuota`/`unavailableError`; o concreto/sobrescrevível
`toUserFacingError`). Ler `tests/providers/amp.test.ts` para o padrão de mock de
`cache` (mockar `cache.getOrFetch` para executar o fetcher direto) e de `logger`.

- [ ] **Step 2: Escrever `tests/providers/base.test.ts`**

Criar a suíte com uma subclasse fake mínima de `BaseProvider`. Cobrir:

- **gate de disponibilidade:** `isAvailable()` → `false` faz `getQuota()` retornar
  `{ provider, displayName, available: false, error: <unavailableError()> }` sem
  chamar `fetchRaw`.
- **caminho de sucesso:** `isAvailable()` → `true`, `fetchRaw()` resolve um valor,
  `buildQuota(raw, base)` é chamado com esse valor e o `base`, e o resultado de
  `buildQuota` é o retorno de `getQuota()`.
- **caminho de erro:** `fetchRaw()` lança → `getQuota()` retorna
  `{ ...base, error: <toUserFacingError(err)> }` e `logger.error` é chamado.
- **`toUserFacingError`:** o default (`error.message` para um `Error`,
  `'Failed to fetch quota'` para um não-`Error`); e que uma subclasse pode
  sobrescrevê-lo.

Mockar `../../src/cache` (`cache.getOrFetch` executa o fetcher) e `../../src/logger`
no padrão de `tests/providers/amp.test.ts`/`codex.test.ts`. A subclasse fake define
`id`/`name`/`cacheKey` e implementa `isAvailable`/`fetchRaw`/`buildQuota`/
`unavailableError` de forma controlável por teste (ex.: campos configuráveis no
`beforeEach`).

Escrever as asserções com valores concretos (sem placeholders) — `expect(...)` para
cada comportamento acima.

- [ ] **Step 3: Verificar**

Run: `bun test tests/providers/base.test.ts && bun test && bun run typecheck && bun run lint`
Expected: a suíte nova passa; a suíte completa sobe de 345 para 345 + N (N = número
de testes adicionados); typecheck e lint limpos.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "test: cobre o BaseProvider"
```

---

## Task 5: Fixtures de snapshot mais ricas

Duas adições de fixture em `tests/formatters-snapshot.test.ts`: (C1) casos com
`p.account`, (C3) fixtures que exercitam branches de erro/edge de
`builders/claude.ts` e `builders/amp.ts`.

**Files:**
- Modify: `tests/formatters-snapshot.test.ts`
- Modify: `tests/__snapshots__/formatters-snapshot.test.ts.snap` (gerado)

- [ ] **Step 1: Ler `tests/formatters-snapshot.test.ts`**

Ler o arquivo inteiro: as factories de mock (`claudeHealthy`, `claudeError`,
`codexHealthy`, etc.), como cada caso de snapshot é estruturado (`describe`/`it` +
`toMatchSnapshot`), e como as 3 superfícies (terminal/Waybar/TUI) são exercitadas.
Ler `src/providers/types.ts` para os campos de `AmpQuota`/`CopilotQuota`/
`ClaudeQuota` (`account`, `extra.weeklyModels`, `extra.extraUsage`, `models`).

- [ ] **Step 2: Adicionar fixtures e casos com `p.account` (C1)**

Adicionar factories de mock para Amp e Copilot **com o campo `account` definido**
(ex.: `ampWithAccount()`, `copilotWithAccount()`), e os respectivos casos de
snapshot nas 3 superfícies, no mesmo estilo dos casos existentes. Isso exercita o
caminho de renderização de account (o gap que escondeu o bug de account duplicado
no Waybar).

- [ ] **Step 3: Adicionar fixtures ricas para os builders (C3)**

Adicionar:
- uma factory de Claude com `extra.weeklyModels` (ex.: Opus/Sonnet) **e**
  `extra.extraUsage` habilitado (`enabled: true`, `limit > 0`) — e o caso de
  snapshot nas 3 superfícies. Cobre `extraUsageLine` e a seção de modelos semanais
  de `builders/claude.ts`.
- uma factory de Amp exercitando estados de credits / fallback de modelos
  desconhecidos — e o caso de snapshot. Cobre os branches de `builders/amp.ts` que o
  caminho feliz não toca.

- [ ] **Step 4: Gerar os snapshots novos e verificar**

Run: `bun test tests/formatters-snapshot.test.ts`
Primeira execução: os casos novos não têm snapshot ainda; o `bun test` os **cria**
automaticamente no `.snap` (não é falha — `toMatchSnapshot` grava na primeira vez).
Rodar de novo para confirmar que passam de forma estável.

Run: `bun test`
Expected: suíte completa verde. Os snapshots **pré-existentes** NÃO mudaram —
confirmar com `git diff tests/__snapshots__/` que só há **adições** (entradas novas),
nenhuma linha de snapshot existente alterada. Se uma entrada existente mudou, uma
fixture nova colidiu com um caso existente — PARAR e reportar.

Run: `bun run typecheck && bun run lint`
Expected: limpos.

- [ ] **Step 5: Confirmar o ganho de cobertura**

Run: `bun test 2>&1 | grep -E "builders/(claude|amp)"`
Expected: a cobertura de funções de `builders/claude.ts` e `builders/amp.ts` subiu
em relação à baseline (claude ~33%, amp ~50%). Não há um número-alvo rígido; o
objetivo é que os branches de `extraUsage`/`weeklyModels` (Claude) e credits/edge
(Amp) passem a ser exercitados.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "test: amplia fixtures de snapshot dos formatters"
```

---

## Task 6: Verificação final da sub-fase

**Files:** nenhum (verificação).

- [ ] **Step 1: Suíte completa + typecheck + lint**

Run: `bun test && bun run typecheck && bun run lint`
Expected: toda a suíte verde, typecheck limpo, lint 0 warnings / 0 errors.

- [ ] **Step 2: Confirmar as limpezas**

Run: `grep -rn "APP_WINDOW_TITLE" src; echo "--- inline imports ---"; grep -rn "import('./types')" src/providers; echo "--- generic helpers ---"; grep -rnE "buildGenericTerminal|buildGenericTooltip" src/formatters`
Expected: `APP_WINDOW_TITLE` — nenhuma ocorrência; `import('./types')` em
`src/providers` — nenhuma ocorrência; `buildGenericTerminal`/`buildGenericTooltip` —
nenhuma ocorrência (substituídos pelo `buildGeneric` do pipeline).

- [ ] **Step 3: Confirmar histórico**

Run: `git log --oneline -6`
Expected: os 5 commits da sub-fase (`refactor: remove dead code e endurece tipos`,
`fix: evita conflito de stdout do spinner no install`,
`refactor: migra fallback generic para o pipeline`, `test: cobre o BaseProvider`,
`test: amplia fixtures de snapshot dos formatters`) acima do commit do spec.

---

## Self-Review (preenchido pelo autor do plano)

**Cobertura do spec:**
- A1 `APP_WINDOW_TITLE` → Task 1 Step 1 ✓
- A2 conflito spinner/stdout → Task 2 ✓
- A3 `isRecord` guard id=1 → Task 1 Step 2 ✓
- A4 imports inline → Task 1 Step 3 ✓
- B `generic` → pipeline → Task 3 ✓
- C1 fixture `p.account` → Task 5 Step 2 ✓
- C2 teste `BaseProvider` → Task 4 ✓
- C3 branches de builders → Task 5 Step 3 ✓
- Verificação de resíduo → Task 6 ✓

**Placeholders:** A1/A3/A4 têm instrução exata (símbolos nomeados, código mostrado).
A2/B são tarefas guiadas pela leitura do código atual + oráculo (`typecheck`/`lint`/
snapshot) — o "código completo" de um refactor protegido por snapshot é a
assinatura-alvo + o que move + o oráculo; adequado à escala. C2/C4 mostram a forma
do teste e os comportamentos a cobrir; o engenheiro escreve as asserções concretas
seguindo o padrão das suítes existentes.

**Consistência:** `buildGeneric(quota, options): Line[]`, `BuildOptions`, os
renderers `renderAnsi`/`renderPango`, e o contrato `BaseProvider` são usados de
forma consistente entre as tasks.

## Risco conhecido

- Task 3 toca os dois entry points dos formatters; a rede é o snapshot (casos
  não-`generic` não podem mudar) + `typecheck`/`lint` para órfãos. O `generic` é
  caminho de baixo tráfego.
- Task 5 adiciona entradas de snapshot; o cuidado é não colidir com casos
  existentes — o `git diff` do `.snap` deve ser só adições.
- Task 2 mexe em `install.ts`, sem teste automatizado — verificação por leitura +
  `typecheck`/`lint`.
