# Fase 3b — Consolidação: limpezas + testes de alto valor

**Data:** 2026-05-15
**Status:** aprovado, pronto para planejamento
**Projeto:** agent-bar — mutirão de limpeza (Fase 3, sub-fase 3b)

## Contexto

As Fases 1, 2, 3a e 3a-bis estão concluídas e publicadas. A Fase 3 foi decomposta
em quatro sub-fases: 3a (formatters), 3a-bis (`BaseProvider`), **3b (este spec)** e
3c (rewrite de docs).

A exploração de cobertura de testes (`bun test` com `coverage`) mostrou que os
módulos criados na Fase 3 já têm boa cobertura **indireta** pelas suítes de
snapshot/formatter — `extras.ts`, `view-model.ts`, `formatters/shared.ts`,
`segments.ts`, `render-ansi.ts`, `render-pango.ts`, `builders/shared.ts` estão em
100% de linha. Os gaps reais de cobertura são arquivos de lifecycle/CLI/IO-pesado
(`waybar-integration.ts`, `setup.ts`, `uninstall.ts`, `action-right.ts`, `index.ts`,
`update.ts`, `cli.ts`) — código de spawn de processo / I/O / TUI interativa, caro de
testar.

Decisão de escopo: a 3b tem **escopo focado** — as limpezas pendentes do backlog +
testes de alto valor específicos. A cobertura do lifecycle IO-pesado fica **fora do
escopo** do mutirão.

## Objetivo

Fechar o backlog de limpezas acumulado nas fases anteriores e adicionar os testes de
maior valor (os que pegariam bugs reais ou cobrem branches descobertos), sem alterar
comportamento observável — exceto a correção de um bug latente de XML-escape e a
remoção de uma exportação morta.

## Parte A — Limpezas

### A1. Remover `APP_WINDOW_TITLE` morto

`src/app-identity.ts` exporta `APP_WINDOW_TITLE = 'Agent Bar'`. Nenhum arquivo o
importa (confirmado por grep). Remover a constante. Verificar que `tsc`/`lint`
continuam limpos.

### A2. Corrigir o conflito `runInteractive` + spinner

`src/install.ts` `ensureBunGlobalPackage`: durante `bun add -g`, `runInteractive`
faz `Bun.spawn` com `stdout`/`stderr: 'inherit'`, enquanto o spinner do
`@clack/prompts` anima — o subprocesso escreve no terminal e corrompe a animação.

Correção: a chamada de instalação do `bun add -g` passa a **capturar** stdout/stderr
do subprocesso (`'pipe'`) em vez de herdar. O spinner permanece limpo durante a
instalação. A saída capturada é exibida **apenas em caso de falha** (exit code ≠ 0
ou o binário não aparece) — em caso de sucesso, é descartada. Avaliar durante a
implementação se `runInteractive` deve ganhar um modo "captura" ou se a chamada de
instalação usa um spawn próprio; o critério é: spinner não corrompido, e diagnóstico
preservado no caminho de falha.

### A3. `isRecord` guard no branch `id === 1` do Codex

`src/providers/codex.ts`, no handler `rl.on('line')` de `fetchRateLimitsViaAppServer`:
o branch `id === 2` recebeu, na Fase 1, um guard `isRecord(msg.result)` antes do
cast. O branch `id === 1` ainda faz `msg.result as CodexAppServerAccountReadResult`
sem o guard. Adicionar `isRecord(msg.result)` à condição do branch `id === 1`, por
consistência e segurança de tipo. Comportamento inalterado (o `isRecord` apenas
estreita antes do cast já existente).

### A4. Imports inline de tipo

`src/providers/amp.ts` e `src/providers/codex.ts` usam `import('./types').XxxQuotaExtra`
inline (ex.: `const extra: import('./types').CodexQuotaExtra = {}`). Mover esses
tipos para o `import type { ... } from './types'` no topo do arquivo. Puramente
estilístico, comportamento inalterado.

## Parte B — Caminho `generic` fallback → pipeline

`buildGenericTerminal` (`terminal.ts`) e `buildGenericTooltip` (`waybar.ts`) — o
fallback para um provider sem builder dedicado — **não** foram migrados para o
pipeline de builders na Fase 3a. Consequências, apontadas pelo review final da 3a:

- `terminal.ts` e `waybar.ts` ainda carregam helpers duplicados com os módulos
  novos: `TOOLTIP_BORDER`, `escapeXml`, os mapas `ANSI_BY_TOKEN`/`HEX_BY_TOKEN`,
  `renderPangoLocal`, `buildHeader`/`buildFooter` locais.
- `buildGenericTooltip` injeta `p.displayName`/`p.error`/`p.provider` no markup
  Pango **sem escape XML** — bug latente: um valor com `<`, `>`, `&` ou `'` produz
  Pango malformado.

Correção: criar `src/formatters/builders/generic.ts` — um builder puro
`buildGeneric(quota, options): Line[]`, o mais simples de todos (header + uma linha
de percentual/erro + footer) — e rotear o fallback dos dois entry points pelos
renderers existentes (`renderAnsi`/`renderPango`). Isso:

- elimina a duplicação residual: os helpers locais (`TOOLTIP_BORDER`, `escapeXml`,
  os mapas de cor, `renderPangoLocal`, `buildHeader`/`buildFooter` locais) que só
  existiam para o fallback são removidos quando deixam de ter consumidor;
- corrige o XML-escape de graça — `render-pango.ts` escapa o conteúdo dos segments
  automaticamente.

Após a migração, `terminal.ts` e `waybar.ts` ficam dispatchers ainda mais finos.
Qualquer helper local que sobrar com consumidor legítimo (ex.: a lógica do texto da
pill do Waybar — `buildText`/`formatProviderForWaybar`) permanece.

## Parte C — Testes de alto valor

### C1. Fixture com `p.account` no snapshot

`tests/formatters-snapshot.test.ts` hoje não tem nenhuma fixture com `p.account`
definido. Esse gap escondeu o bug de account duplicado no Waybar (corrigido na 3a).
Adicionar fixtures de Amp e de Copilot **com `account`** e os respectivos casos de
snapshot nas três superfícies. Gera **novas** entradas no arquivo `.snap` — geradas
de propósito, parte desta task. Os snapshots **existentes** não mudam.

### C2. Teste direto do `BaseProvider`

Criar `tests/providers/base.test.ts`. Com uma subclasse fake mínima de
`BaseProvider`, exercitar o template `getQuota()`:
- gate de disponibilidade — `isAvailable()` false → retorna `{ ...base, error: unavailableError() }`;
- caminho de sucesso — `fetchRaw` resolve → `buildQuota` é chamado, resultado retornado;
- caminho de erro — `fetchRaw` lança → `catch` loga (`logger.error`) e retorna
  `{ ...base, error: toUserFacingError(error) }`;
- `toUserFacingError` — default e sobrescrito.
Mockar `cache.getOrFetch` para executar o fetcher diretamente, no padrão das suítes
de provider existentes.

### C3. Cobrir branches de erro/edge dos builders

`builders/claude.ts` (33% de funções) e `builders/amp.ts` (50%) têm branches que o
caminho feliz dos snapshots não exercita. Adicionar fixtures mais ricas em
`formatters-snapshot.test.ts`:
- Claude com `extra.weeklyModels` e `extra.extraUsage` habilitados (cobre
  `extraUsageLine` e a seção de modelos semanais);
- Amp com estados de credits / fallback de modelos desconhecidos.
Cada fixture nova gera entradas de snapshot novas (intencionais). O alvo é elevar a
cobertura de funções desses dois builders exercitando os branches reais.

## Plano de verificação

- `bun test` — toda a suíte verde. Os snapshots **existentes** de terminal/Waybar
  ficam byte/texto-idênticos; há **apenas adições** de snapshot (das fixtures novas
  de C1 e C3), geradas de propósito.
- `bun run typecheck` — limpo.
- `bun run lint` — Biome limpo, 0 warnings.
- Após a Parte B: confirmar que `terminal.ts`/`waybar.ts` não têm mais helpers
  órfãos; `grep` por `escapeXml`/`renderPangoLocal`/`TOOLTIP_BORDER` em `waybar.ts`
  só deve achar o que tiver consumidor legítimo restante.

## Fora de escopo

- Cobertura de testes para o lifecycle IO-pesado (`waybar-integration.ts`,
  `setup.ts`, `uninstall.ts`, `action-right.ts`, `update.ts`, `cli.ts`,
  `install.ts`, `index.ts`, `menu.ts`, `tui/*`).
- Rewrite de `AGENTS.md`/`CLAUDE.md` — Fase 3c.

## Risco conhecido

- A migração do `generic` (Parte B) toca os dois entry points dos formatters. A
  rede de segurança é o snapshot do caso `generic` (se existir) e o
  `typecheck`/`lint` para helpers órfãos. O `generic` é caminho de baixo tráfego
  (só renderiza para um provider sem builder dedicado — hoje inexistente), então o
  risco de regressão visível é baixo; ainda assim, a saída do `generic` deve sair
  equivalente à atual (exceto o XML-escape, que passa a estar correto).
- A correção A2 (`runInteractive`) afeta um caminho sem teste automatizado
  (`install.ts`); a verificação é por `typecheck`/`lint` e raciocínio. A mudança de
  `inherit` para `pipe` na chamada de `bun add -g` é localizada e de baixo risco.
