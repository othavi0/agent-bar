# Fase 3a-bis — Extração de `BaseProvider` — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extrair uma classe abstrata `BaseProvider` com o template-method `getQuota()` e migrar Codex, Copilot e Amp para herdar dela, eliminando o boilerplate repetido de base/availability/cache/erro.

**Architecture:** `BaseProvider implements Provider` possui o `getQuota()` concreto (base → gate de disponibilidade → `cache.getOrFetch` em try/catch → pós-processamento) e delega o que difere a 3 métodos abstratos (`unavailableError`, `fetchRaw`, `buildQuota`) + 1 sobrescrevível (`toUserFacingError`). Claude **não** é migrado — seu fluxo de credenciais em estágios não cabe no template sem mudar comportamento.

**Tech Stack:** TypeScript strict, Bun (`bun:test`), Biome.

---

## Contexto para o engenheiro

`agent-bar` é um monitor de quota LLM para Waybar. Os providers vivem em
`src/providers/*.ts` e implementam a interface `Provider` de `src/providers/types.ts`.
Spec de origem: `docs/superpowers/specs/2026-05-15-fase-3a-bis-baseprovider-design.md`.

Convenções: **Bun apenas** (`bun test`, `bun run typecheck`, `bun run lint`).
TypeScript strict. Biome (2 espaços, aspas simples, 120 colunas, import não usado =
erro). Commits em Português, Conventional Commits, subject ≤ 50 chars.

### O oráculo de correção

As suítes `tests/providers/{claude,codex,copilot,amp}.test.ts` e
`tests/providers/codex-appserver.test.ts` afirmam strings de erro e comportamento
**exatos** (o `AGENTS.md` exige estabilidade dessas strings). Esta refatoração é
byte-idêntica em comportamento — todas devem permanecer verdes **sem nenhuma
alteração nos arquivos de teste**. Hoje a suíte tem 345 testes, 0 falhas.

### Por que Claude fica de fora

O `getQuota()` do `ClaudeProvider` faz checagem de credencial em três estágios, cada
um com mensagem própria (`Not logged in…` / `Invalid credentials file` / `No access
token`), e propaga o campo `plan` (lido das credenciais) nos retornos de erro
pós-fetch. O template do `BaseProvider` — gate único `isAvailable()` +
`unavailableError()` de uma só mensagem, e `buildQuota(raw)` sem acesso às
credenciais — não reproduz isso sem alterar mensagens/campos. Forçar seria o
"force-fit" que o spec proíbe. `claude.ts` permanece como `implements Provider`
standalone, intacto.

## Estrutura de arquivos

| Arquivo | Ação |
| --- | --- |
| `src/providers/base.ts` | **Criar** — classe abstrata `BaseProvider` |
| `src/providers/amp.ts` | Migrar para `extends BaseProvider` |
| `src/providers/copilot.ts` | Migrar para `extends BaseProvider` |
| `src/providers/codex.ts` | Migrar para `extends BaseProvider` |
| `src/providers/claude.ts` | **Não tocar** |

---

## Task 1: Criar `BaseProvider`

Cria a classe abstrata. Nenhum provider a usa ainda — uma classe abstrata sem
subclasse compila e não muda comportamento.

**Files:**
- Create: `src/providers/base.ts`

- [ ] **Step 1: Criar `src/providers/base.ts`**

```typescript
import { cache } from '../cache';
import { CONFIG } from '../config';
import type { Provider, ProviderQuota } from './types';

/** The minimal quota shape available before a successful fetch. */
export interface QuotaBase {
  provider: string;
  displayName: string;
  available: false;
}

/**
 * Abstract base for quota providers. Owns the getQuota() orchestration —
 * base object, availability gate, cache wrapper, error handling — so each
 * concrete provider implements only the parts that genuinely differ.
 */
export abstract class BaseProvider implements Provider {
  abstract readonly id: string;
  abstract readonly name: string;
  abstract readonly cacheKey: string;

  abstract isAvailable(): Promise<boolean>;

  /** Fetch raw provider data. The result is cached under `cacheKey`. */
  protected abstract fetchRaw(): Promise<unknown>;

  /** Convert the (cached) raw data from `fetchRaw` into the final quota. */
  protected abstract buildQuota(raw: unknown, base: QuotaBase): ProviderQuota;

  /** Error message shown when the provider is unavailable. */
  protected abstract unavailableError(): string;

  /** Map a thrown fetch error to a user-facing message. Override as needed. */
  protected toUserFacingError(error: unknown): string {
    return error instanceof Error ? error.message : 'Failed to fetch quota';
  }

  protected buildBase(): QuotaBase {
    return { provider: this.id, displayName: this.name, available: false };
  }

  async getQuota(): Promise<ProviderQuota> {
    const base = this.buildBase();

    if (!(await this.isAvailable())) {
      return { ...base, error: this.unavailableError() };
    }

    try {
      const raw = await cache.getOrFetch(this.cacheKey, () => this.fetchRaw(), CONFIG.cache.ttlMs);
      return this.buildQuota(raw, base);
    } catch (error) {
      return { ...base, error: this.toUserFacingError(error) };
    }
  }
}
```

- [ ] **Step 2: Verificar**

Run: `bun test && bun run typecheck && bun run lint`
Expected: PASS. 345 testes, 0 falhas (nada usa `BaseProvider` ainda — comportamento
inalterado). Typecheck e lint limpos. Se o `{ ...base, error }` ou o retorno de
`getQuota` exigir um `as ProviderQuota` para o `tsc` aceitar (o `base` tem
`provider: string`, casando com `GenericQuota`), o cast é aceitável e esperado.

- [ ] **Step 3: Commit**

```bash
git add src/providers/base.ts
git commit -m "refactor: adiciona classe abstrata BaseProvider"
```

---

## Task 2: Migrar o Amp para `BaseProvider`

Amp é o mais simples: um único `AMP_MISSING_ERROR` de indisponibilidade, e o
fetcher já monta o objeto final (`AmpQuota`) — `buildQuota` é quase identidade.

**Files:**
- Modify: `src/providers/amp.ts`

- [ ] **Step 1: Migrar `AmpProvider`**

Reescrever `AmpProvider` para `extends BaseProvider`. Mapear o `getQuota()` atual
nos métodos do template:
- A classe passa a `extends BaseProvider`; remover `implements Provider` (a base já
  o faz). Manter `readonly id`/`name`/`cacheKey` e `isAvailable()`.
- Remover o método `getQuota()` — herdado da base.
- `unavailableError(): string` — retorna `AMP_MISSING_ERROR`.
- `fetchRaw(): Promise<unknown>` — o trabalho que hoje é o `fetcher` passado a
  `cache.getOrFetch` (chamar `findAmpBin()`, `fetchUsage(...)`, montar o `AmpQuota`).
  O `fetchUsage` atual recebe `base`/`bin`; adaptar para que `fetchRaw` obtenha o
  `bin` por conta própria (`findAmpBin()`) e construa o `base` localmente
  (`this.buildBase()` ou um literal equivalente) — o objetivo é que `fetchRaw`
  devolva exatamente o mesmo `AmpQuota` que hoje é cacheado.
- `buildQuota(raw, base): ProviderQuota` — como Amp cacheia o objeto final,
  `buildQuota` devolve `raw as AmpQuota` (identidade). Se o `raw` puder vir sem o
  `base` aplicado, garantir `{ ...base, ...(raw as AmpQuota) }` — o critério é
  comportamento idêntico ao atual.
- `registerProvider(new AmpProvider())` permanece no fim do arquivo.

Atenção ao detalhe de comportamento: hoje o `fetchUsage` do Amp pode **retornar**
(não lançar) um `AmpQuota` com `error` (ex.: exit code ≠ 0, "Not logged in"). Esse
objeto é devolvido por `fetchRaw`, cacheado por `getOrFetch`, e passa por
`buildQuota` sem mudança — exatamente como hoje. NÃO transformar esses retornos de
erro em exceções.

- [ ] **Step 2: Verificar**

Run: `bun test tests/providers/amp.test.ts && bun test && bun run typecheck && bun run lint`
Expected: PASS. `tests/providers/amp.test.ts` 100% verde **sem editar o teste** —
ele é o oráculo: toda string de erro e todo campo do `AmpQuota` deve sair idêntico.
Suíte completa 345/0. Typecheck e lint limpos. Se algum teste de Amp falhar, o
comportamento divergiu — corrigir o provider, NUNCA o teste.

- [ ] **Step 3: Commit**

```bash
git add src/providers/amp.ts
git commit -m "refactor: Amp estende BaseProvider"
```

---

## Task 3: Migrar o Copilot para `BaseProvider`

Copilot, como o Amp, cacheia o objeto final (`CopilotQuota`) — `buildQuota` é
identidade. Tem dois erros de indisponibilidade (`COPILOT_MISSING_ERROR` quando o
bin não existe, `NOT_LOGGED_IN_ERROR` quando não autenticado) e já possui um
`toUserFacingError` por regex.

**Files:**
- Modify: `src/providers/copilot.ts`

- [ ] **Step 1: Migrar `CopilotProvider`**

Reescrever `CopilotProvider` para `extends BaseProvider`:
- `extends BaseProvider`; remover `implements Provider`. Manter `id`/`name`/`cacheKey`.
- Manter `isAvailable()` como está.
- Remover `getQuota()` — herdado.
- `unavailableError(): string` — hoje o `getQuota` distingue dois casos: bin
  ausente (`COPILOT_MISSING_ERROR`) e não-logado (`NOT_LOGGED_IN_ERROR`). O gate da
  base usa um único `isAvailable()`; portanto `unavailableError()` deve reproduzir a
  distinção: checar `findCopilotBin()` — se ausente, devolver `COPILOT_MISSING_ERROR`;
  senão devolver `NOT_LOGGED_IN_ERROR`. Confirmar contra `copilot.test.ts` que as
  duas mensagens saem nos casos certos.
- `fetchRaw(): Promise<unknown>` — o trabalho do fetcher atual (`fetchUsage`),
  devolvendo o `CopilotQuota` final.
- `buildQuota(raw, base)` — identidade (`raw as CopilotQuota`), garantindo o `base`
  se necessário.
- `toUserFacingError(error)` — sobrescrever com o mapeamento por regex que já existe
  no provider hoje (o `toUserFacingError` atual de `copilot.ts`).
- `registerProvider(new CopilotProvider())` permanece.

- [ ] **Step 2: Verificar**

Run: `bun test tests/providers/copilot.test.ts && bun test && bun run typecheck && bun run lint`
Expected: PASS. `tests/providers/copilot.test.ts` verde sem edição (oráculo —
inclui os dois caminhos de erro de indisponibilidade e o mapeamento de
`toUserFacingError`). Suíte completa 345/0. Typecheck e lint limpos.

- [ ] **Step 3: Commit**

```bash
git add src/providers/copilot.ts
git commit -m "refactor: Copilot estende BaseProvider"
```

---

## Task 4: Migrar o Codex para `BaseProvider`

Codex cacheia dados **crus** (`CodexRateLimits`) e tem pós-processamento extenso
(`buildModelWindows`/`flattenModels`/`pickPrimary`/`pickSecondary`) — esse
pós-processamento é o corpo do `buildQuota`.

**Files:**
- Modify: `src/providers/codex.ts`

- [ ] **Step 1: Migrar `CodexProvider`**

Reescrever `CodexProvider` para `extends BaseProvider`:
- `extends BaseProvider`; remover `implements Provider`. Manter `id`/`name`/`cacheKey`.
- Manter `isAvailable()`.
- Remover `getQuota()` — herdado.
- `unavailableError(): string` — a mensagem atual de não-logado do Codex
  (`Not logged in. Open \`agent-bar menu\` and choose Provider login.`).
- `fetchRaw(): Promise<unknown>` — o fetcher atual: tenta `fetchRateLimitsViaAppServer`,
  com fallback para a session log; devolve o `CodexRateLimits`. Lança nos casos em
  que hoje lança (`No session data found`, `No rate limit data found …`).
- `buildQuota(raw, base): ProviderQuota` — o pós-processamento atual de `getQuota`
  após o `getOrFetch`: `buildModelWindows`, `flattenModels`, `pickPrimary`,
  `pickSecondary`, montagem de `extra.modelsDetailed`/`extra.extraUsage`, `planType`,
  `plan`, e o caso `'No quota windows found'`. Recebe `raw` (o `CodexRateLimits`) e
  monta o `CodexQuota` final. Todos os métodos auxiliares privados do Codex
  (`buildModelWindows`, etc.) permanecem na classe.
- `toUserFacingError(error)` — o Codex hoje devolve `error.message ?? 'Failed to fetch
  Codex usage'` no catch. O default da base é `error.message ?? 'Failed to fetch
  quota'`. Para manter a mensagem genérica byte-idêntica (`Failed to fetch Codex
  usage`), sobrescrever `toUserFacingError` no Codex para usar esse fallback.
- `registerProvider(new CodexProvider())` permanece.

- [ ] **Step 2: Verificar**

Run: `bun test tests/providers/codex.test.ts tests/providers/codex-appserver.test.ts && bun test && bun run typecheck && bun run lint`
Expected: PASS. As duas suítes de Codex verdes sem edição (oráculo — cobrem o
app-server, o fallback, o pós-processamento de buckets/janelas, créditos, e as
mensagens de erro). Suíte completa 345/0. Typecheck e lint limpos.

- [ ] **Step 3: Commit**

```bash
git add src/providers/codex.ts
git commit -m "refactor: Codex estende BaseProvider"
```

---

## Task 5: Verificação final da sub-fase

**Files:** nenhum (verificação).

- [ ] **Step 1: Suíte completa + typecheck + lint**

Run: `bun test && bun run typecheck && bun run lint`
Expected: 345 pass / 0 fail; typecheck limpo; lint 0 warnings / 0 errors.

- [ ] **Step 2: Confirmar a herança e o boilerplate eliminado**

Run: `grep -rn "extends BaseProvider" src/providers; echo "--- getQuota ---"; grep -rn "getQuota" src/providers/*.ts`
Expected: `amp.ts`, `copilot.ts`, `codex.ts` mostram `extends BaseProvider`;
`base.ts` é o único `*.ts` de provider com um corpo de `getQuota()` (mais
`claude.ts`, que mantém o seu); `amp.ts`/`copilot.ts`/`codex.ts` não têm mais
`getQuota()` próprio.

- [ ] **Step 3: Confirmar histórico**

Run: `git log --oneline -5`
Expected: os 4 commits da sub-fase (`refactor: adiciona classe abstrata BaseProvider`,
`refactor: Amp estende BaseProvider`, `refactor: Copilot estende BaseProvider`,
`refactor: Codex estende BaseProvider`) acima do commit do spec.

---

## Self-Review (preenchido pelo autor do plano)

**Cobertura do spec:**
- Classe abstrata `BaseProvider` com template-method `getQuota()` → Task 1 ✓
- Concretos `buildBase`/`toUserFacingError` + abstratos `unavailableError`/`fetchRaw`/`buildQuota` → Task 1 ✓
- Divergência de cache acomodada (`fetchRaw(): unknown`; `buildQuota` identidade para Copilot/Amp) → Tasks 2-4 ✓
- Migração de Codex/Copilot/Amp → Tasks 2-4 ✓
- Claude deliberadamente fora → documentado no contexto + "Por que Claude fica de fora"
- Comportamento byte-idêntico verificado pelas suítes de provider → Steps de verificação de cada task ✓

**Placeholders:** o `base.ts` (Task 1) tem código completo. As Tasks 2-4 são
migrações guiadas pelo oráculo de testes — o plano nomeia exatamente qual lógica
atual vira qual método do template; o corpo exato de `fetchRaw`/`buildQuota` é o
código já existente em cada `getQuota()`, movido sem alteração de comportamento, e
as suítes de provider verificam byte-a-byte. Isto é adequado a um refactor mecânico
com oráculo objetivo.

**Consistência de tipos:** `QuotaBase`, `BaseProvider`, e as assinaturas
`fetchRaw(): Promise<unknown>` / `buildQuota(raw: unknown, base: QuotaBase): ProviderQuota`
/ `unavailableError(): string` / `toUserFacingError(error: unknown): string` são
usadas de forma consistente entre as tasks.

## Risco conhecido

- A migração move código existente entre métodos; o `tsc` (assinaturas) e as suítes
  de provider (comportamento/strings) são a rede dupla. Qualquer string de erro
  alterada falha um teste imediatamente.
- O `getQuota()` herdado tem retorno `Promise<ProviderQuota>` em vez do tipo
  estreito (`Promise<AmpQuota>` etc.) que os providers declaravam. Isso é compatível
  com a interface `Provider` (que já declara `Promise<ProviderQuota>`) e nenhum
  consumidor depende do tipo estreito — aceitável.
