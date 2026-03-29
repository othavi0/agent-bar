# Design: Melhorias Abrangentes do Codebase

**Data:** 2026-03-28
**Escopo:** Qualidade de código, testes, infraestrutura local, extensibilidade
**Sequência:** Fase 1 (Housekeeping) → Fase 2 (Testes) → Fase 3 (Extensibilidade)

---

## Contexto

O agent-bar-omarchy é um CLI TypeScript/Bun com 6.400 linhas em 35 arquivos, 209 testes (~75% cobertura). O codebase é bem estruturado, mas acumulou duplicações entre providers, inconsistências no padrão de cache, e gaps de cobertura de testes. Com novos providers (Gemini, Cursor, Windsurf) planejados, a extensibilidade é prioridade.

---

## Fase 1: Housekeeping (baixo risco, alto retorno)

### 1.1 Eliminar `classifyWindow` duplicada

- **Remover** o método privado `classifyWindow()` em `src/providers/codex.ts:244-251`
- **Importar** `classifyWindow` de `src/formatters/shared.ts:39-46` (lógica idêntica)
- Impacto: 1 arquivo modificado, 0 risco funcional

### 1.2 Unificar normalização de plano

- **Remover** `normalizePlanName()` de `src/providers/codex.ts:148-165`
- **Usar** `normalizePlanLabel()` de `src/formatters/shared.ts:48-68` no Codex
- A versão da shared.ts é estritamente melhor (fallback com titlecase)
- Ajuste necessário: `normalizePlanLabel` hoje recebe `ProviderQuota` e lê `p.plan` e `p.planType`. O Codex chama `normalizePlanName(limits.plan_type)` com uma string raw.
- **Solução**: extrair a lógica do map para `normalizePlan(raw: string | undefined): string` em `src/formatters/shared.ts`. A função `normalizePlanLabel(p: ProviderQuota)` passa a chamar `normalizePlan(p.plan ?? p.planType)` internamente. O Codex importa `normalizePlan` diretamente.
- Não criar arquivo novo — manter em `src/formatters/shared.ts` que já é o lar canônico de utilidades de formatação

### 1.3 Unificar cache do Codex para `getOrFetch`

- **Substituir** o padrão manual `cache.get()` + `cache.set()` em `src/providers/codex.ts:487-506`
- **Usar** `cache.getOrFetch()` com fetcher que encapsula a cadeia app-server → session-log
- Isso ativa a deduplicação in-flight (evita múltiplos processos app-server simultâneos)

### 1.4 Remover `codexTtlMs` redundante

- **Remover** `codexTtlMs: 300_000` de `src/config.ts:39`
- **Atualizar** `src/providers/codex.ts:506` para usar `CONFIG.cache.ttlMs`
- Ambos têm o mesmo valor (300.000ms). Se no futuro Codex precisar de TTL diferente, basta adicionar de volta

### 1.5 Padronizar mensagens de erro

- O Amp já tem mensagens padronizadas (linhas 53, 60), mas o catch genérico (linha 37) diz apenas "Failed to fetch usage"
- Codex (linha 497) diz "No session data found" sem orientação
- **Padrão**: toda mensagem de "não logado" deve seguir: `Not logged in. Open \`agent-bar-omarchy menu\` and choose Provider login.`
- **Padrão**: erros de fetch devem incluir o provider: `"Failed to fetch {Provider} usage"`

### 1.6 Adicionar Biome linter

- Instalar `@biomejs/biome` como dev dependency
- Criar `biome.json` respeitando o estilo existente (ESM, indentação com tabs/spaces conforme atual)
- Scripts: `"lint": "bunx biome check"`, `"lint:fix": "bunx biome check --write"`
- Rodar `lint:fix` uma vez para alinhar formatação existente

### 1.7 Adicionar pre-commit hook

- Usar `lefthook` (single binary) ou script `.git/hooks/pre-commit` simples
- Hook executa: `bun run typecheck && bun test`
- Impede commits com erros de tipo ou testes quebrados

---

## Fase 2: Fundação de Testes

### 2.1 Criar test helpers compartilhados

- **Arquivo**: `tests/helpers/mocks.ts`
- Funções:
  - `fakeFile({ exists, json?, text? })` — mock unificado de `Bun.file()`
  - `mockProviderQuota(overrides)` — cria `ProviderQuota` válido com defaults
  - `mockAllQuotas(providers)` — wraps em `AllQuotas`
  - `futureUnix(hoursFromNow)` — timestamp helper (duplicado em vários testes)

### 2.2 Snapshot tests para formatters

- **Arquivo**: `tests/formatters-snapshot.test.ts`
- Cenários por provider: healthy, error, edge cases (0%, 100%, null)
- `expect(result).toMatchSnapshot()` para terminal e waybar output
- Sanitizar timestamps dinâmicos antes do snapshot

### 2.3 Testar Codex app-server protocol

- **Arquivo**: `tests/providers/codex-appserver.test.ts`
- Mockar `Bun.spawn` para fornecer stdio fake com handshake JSON-RPC
- Validar que `initialize`, `account/read`, `account/rateLimits/read` são enviados corretamente
- Testar timeout, grace period, e cleanup de processo

### 2.4 Corrigir over-mocking no Codex

- Substituir `(p as any).fetchRateLimitsViaAppServer = ...` por mocks de dependências reais (spawn, Bun.Glob, Bun.file)
- Isso testa o código real dos métodos internos

### 2.5 Expandir testes do waybar-contract

- Testar `exportWaybarCss()` para todos 6 estilos de separador
- Testar `normalizeProviderSelection()` com deduplicação e providers desconhecidos

### 2.6 Testar CLI parsing

- **Arquivo**: `tests/cli.test.ts`
- Testar `parseArgs()` com combinações de flags e comandos
- Testar did-you-mean (Levenshtein) para comandos incorretos

---

## Fase 3: Extensibilidade para Novos Providers

### 3.1 Provider Registry

- **Arquivo**: `src/providers/registry.ts`
- API: `registerProvider(provider: Provider)`, `getProviders(): Provider[]`, `getProvider(id): Provider | undefined`
- Cada provider se auto-registra no module scope: `registerProvider(new ClaudeProvider())`
- `src/providers/index.ts` re-exporta do registry em vez de ter array hardcoded

### 3.2 Tooltip Builder Registry

- Cada provider exporta opcionalmente um `buildTooltip(p: ProviderQuota, fetchedAt?: string): string`
- **Arquivo**: `src/formatters/tooltip-registry.ts` — map de `providerId → tooltipBuilder`
- Providers desconhecidos usam tooltip genérico (mostra primary/secondary com barras de progresso)
- Elimina os 3 `switch` statements em `waybar.ts` (linhas 429, 490, 518)

### 3.3 Terminal Builder Registry

- Mesmo padrão: cada provider registra um `buildTerminal(p: ProviderQuota): string[]`
- Elimina o `switch` em `terminal.ts:305`
- Fallback genérico para providers sem builder customizado

### 3.4 Atualizar waybar-contract

- `WAYBAR_PROVIDERS` no `waybar-contract.ts` deve derivar do provider registry
- Novos providers que se registram automaticamente aparecem nas opções de Waybar

---

## Arquivos Críticos

| Arquivo | Fase | Mudança |
|---------|------|---------|
| `src/providers/codex.ts` | 1, 2 | Remover duplicações, unificar cache, testar app-server |
| `src/formatters/shared.ts` | 1 | Recebe lógica unificada de plan/window |
| `src/config.ts` | 1 | Remover `codexTtlMs` |
| `src/providers/index.ts` | 3 | Migrar para registry |
| `src/formatters/waybar.ts` | 2, 3 | Snapshot tests + tooltip registry |
| `src/formatters/terminal.ts` | 2, 3 | Snapshot tests + builder registry |
| `tests/helpers/mocks.ts` | 2 | Novo: test helpers compartilhados |
| `biome.json` | 1 | Novo: configuração do linter |

---

## Verificação

### Fase 1
```bash
bun test                # todos os testes passam
bun run typecheck       # sem erros de tipo
bunx biome check        # sem lint errors
```

### Fase 2
```bash
bun test                          # cobertura total >= 85%
bun test --coverage 2>&1 | grep "codex"  # codex >= 75%
```

### Fase 3
```bash
bun test                # todos os testes passam
bun run typecheck       # sem erros (interfaces novas)
# Teste manual: adicionar provider mock e verificar que aparece no waybar output
```
