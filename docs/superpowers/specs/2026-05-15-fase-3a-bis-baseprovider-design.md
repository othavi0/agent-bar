# Fase 3a-bis — Extração de `BaseProvider`

**Data:** 2026-05-15
**Status:** aprovado, pronto para planejamento
**Projeto:** agent-bar — mutirão de limpeza (Fase 3, sub-fase 3a-bis)

## Contexto

As Fases 1, 2 e 3a estão concluídas e publicadas. A Fase 3 foi decomposta em
quatro sub-fases: 3a (formatters, feita), **3a-bis (este spec)**, 3b (cobertura de
testes + limpezas), 3c (rewrite de docs).

Os quatro providers — `src/providers/claude.ts`, `codex.ts`, `copilot.ts`,
`amp.ts` — implementam a interface `Provider` (`src/providers/types.ts`) com um
`getQuota()` divergente, mas todos repetem o mesmo esqueleto (~40 linhas de
boilerplate cada):

1. Montar um objeto `base` incompleto (`{ provider, displayName, available: false }`).
2. Checar disponibilidade; se indisponível, retornar `{ ...base, error: <mensagem> }`.
3. `cache.getOrFetch(cacheKey, fetcher, CONFIG.cache.ttlMs)` dentro de `try/catch`;
   no `catch`, retornar `{ ...base, error: <mensagem amigável> }`.
4. Pós-processar o resultado e retornar o `ProviderQuota` final.

## Objetivo

Extrair uma classe abstrata `BaseProvider` que possua o template-method `getQuota()`,
eliminando o boilerplate repetido e padronizando o tratamento de erro/cache, sem
alterar o comportamento observável de nenhum provider.

## Arquitetura

Novo arquivo `src/providers/base.ts` com a classe abstrata `BaseProvider`, que
`implements Provider`. O `getQuota()` é um template-method concreto:

```
async getQuota():
  const base = this.buildBase()
  if (!(await this.isAvailable())) {
    return { ...base, error: this.unavailableError() }
  }
  try {
    const raw = await cache.getOrFetch(this.cacheKey, () => this.fetchRaw(), CONFIG.cache.ttlMs)
    return this.buildQuota(raw, base)
  } catch (error) {
    return { ...base, error: this.toUserFacingError(error) }
  }
```

### Membros da `BaseProvider`

**Concretos (fornecidos pela base):**

- `getQuota()` — o template-method acima.
- `buildBase()` — monta `{ provider: this.id, displayName: this.name, available: false }`.
- `toUserFacingError(error: unknown): string` — default:
  `error instanceof Error ? error.message : 'Failed to fetch quota'`. **Sobrescrevível.**

**Abstratos (cada provider implementa):**

- `unavailableError(): string` — a mensagem de erro quando o provider está indisponível.
- `fetchRaw(): Promise<unknown>` — busca os dados (HTTP / CLI / app-server / file).
  É o `fetcher` passado a `cache.getOrFetch`.
- `buildQuota(raw: unknown, base): ProviderQuota` — converte o resultado de
  `fetchRaw` no `ProviderQuota` final.

**Já no contrato `Provider` (não duplicar):** `isAvailable()`, `id`, `name`,
`cacheKey`. O template chama `this.isAvailable()` diretamente. Cada provider
continua declarando seus `readonly id/name/cacheKey` e o
`registerProvider(new XxxProvider())` no fim do módulo.

### Tipagem

`fetchRaw()` devolve `unknown` e `buildQuota()` recebe `unknown`. Cada provider faz
o narrowing interno do seu tipo cru concreto. O `BaseProvider` é genérico apenas no
necessário; não se tenta parametrizar a classe com o tipo cru de cada provider
(YAGNI — `unknown` + narrowing local é suficiente e mais simples).

## Divergência de cache — decisão de escopo

Há uma divergência real no **nível** do que é cacheado:

- **Claude** e **Codex** cacheiam dados **crus/intermediários**
  (`ClaudeUsageResponse`, `CodexRateLimits`) e fazem o pós-processamento DEPOIS do
  `getOrFetch`.
- **Copilot** e **Amp** cacheiam o **objeto final** (`CopilotQuota`, `AmpQuota`),
  montado dentro do próprio fetcher.

O template acomoda os dois estilos: `fetchRaw()` devolve `unknown`. Para
Claude/Codex, `fetchRaw` devolve o dado cru e `buildQuota` faz o pós-processamento.
Para Copilot/Amp, `fetchRaw` devolve o `ProviderQuota` já montado e `buildQuota` é
praticamente identidade (apenas devolve o objeto, eventualmente garantindo o
`base`).

**Decisão:** NÃO unificar o nível de cache (não refatorar Copilot/Amp para cachear
cru). Isso exigiria mover a lógica de montagem para fora dos fetchers em dois
providers — risco alto para ganho marginal. O template uniforme com
`fetchRaw(): unknown` é suficiente.

## Aplicação por provider

Cada um dos 4 providers passa a `extends BaseProvider` e implementa os 3 abstratos:

| Provider | `unavailableError()` | `fetchRaw()` | `buildQuota()` | `toUserFacingError()` |
| --- | --- | --- | --- | --- |
| Claude | "Not logged in…" (+ checagens de credencial/token) | POST HTTP à API de usage | parse das 5 windows + `weeklyModels` + `extraUsage` | sobrescreve: AbortError/timeout, `Claude API error:` |
| Codex | "Not logged in…" | app-server JSON-RPC + fallback session log | `buildModelWindows`/`flatten`/`pick` | default da base |
| Copilot | `COPILOT_MISSING_ERROR` / `NOT_LOGGED_IN_ERROR` | CLI stdio JSON-RPC → `CopilotQuota` pronto | identidade | sobrescreve: `toUserFacingError` por regex (já existe) |
| Amp | `AMP_MISSING_ERROR` | `amp usage` spawn + parse → `AmpQuota` pronto | identidade | default da base |

Observações de comportamento que DEVEM ser preservadas exatamente:
- Claude faz checagens em sequência (arquivo de credencial → JSON válido → access
  token), cada uma com mensagem própria. Essas checagens são parte do
  `isAvailable()`/`unavailableError()` OU permanecem como hoje se não couberem
  limpo no par availability/error — o critério é: comportamento e mensagens
  byte-idênticos. Se o fluxo de Claude não couber no par
  `isAvailable()`/`unavailableError()` sem mudar mensagens, manter a verificação
  detalhada dentro do próprio provider (ex.: em `fetchRaw`/`buildQuota`) é
  aceitável — o template não pode forçar uma mudança de mensagem.
- Erros de fetch que hoje são retornados como valor (não lançados) e portanto
  cacheados — ex.: o `fetchUsage` do Amp devolvendo `{ ...base, error }` quando o
  exit code é ≠ 0 — continuam sendo devolvidos por `fetchRaw` e passam por
  `buildQuota` sem mudança. O `cache.getOrFetch` os cacheia como hoje.

## Plano de verificação

- **Rede de segurança:** `tests/providers/{claude,codex,copilot,amp}.test.ts` e
  `tests/providers/codex-appserver.test.ts`. Essas suítes afirmam strings de erro e
  comportamento exatos (o `AGENTS.md` exige estabilidade dessas strings). O
  refactor é byte-idêntico em comportamento — todas devem permanecer verdes **sem
  alteração**.
- `bun test` — 345 testes, 0 falhas.
- `bun run typecheck` — limpo.
- `bun run lint` — Biome limpo, 0 warnings.

## Fora de escopo

- Unificar o nível de cache (Copilot/Amp cachear dados crus).
- Alterar a lógica de fetch ou de normalização de qualquer provider.
- Cobertura de testes nova, inclusive para `base.ts` — Fase 3b.
- O caminho `generic` fallback dos formatters — Fase 3b.

## Risco conhecido

Risco baixo-médio. Os 4 providers já são classes que implementam `Provider`;
introduzir uma base abstrata é natural e não quebra a interface. O maior cuidado é
o fluxo de checagem em sequência do Claude (várias mensagens de erro distintas) —
se ele não couber limpo no par `isAvailable()`/`unavailableError()`, preserva-se a
verificação detalhada dentro do provider em vez de forçar a abstração e mudar uma
mensagem. As suítes de provider são o oráculo: qualquer string de erro alterada
falha um teste imediatamente.
