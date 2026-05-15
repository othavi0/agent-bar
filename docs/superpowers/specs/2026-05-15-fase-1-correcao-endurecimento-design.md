# Fase 1 — Correção e endurecimento

**Data:** 2026-05-15
**Status:** aprovado, pronto para planejamento
**Projeto:** agent-bar — mutirão de limpeza (Fase 1 de 3)

## Contexto

O `agent-bar` (monitor de quota LLM para Waybar) passou por uma auditoria completa.
As migrações de identidade e schema estão completas e testadas — não há migração
pela metade. A dívida real está em três frentes, que foram decompostas em fases:

- **Fase 1 (este spec):** correção de bugs e endurecimento, sem mudança de arquitetura.
- **Fase 2:** remoção total da camada de compatibilidade legacy (`qbar` + `agent-bar-omarchy`).
- **Fase 3:** refatoração estrutural, docs finais e cobertura de testes.

Esta fase é deliberadamente cirúrgica: corrige defeitos reais e remove uma dependência
redundante, sem tocar em boundaries de módulo. Isso garante uma base correta antes da
remoção de legacy (Fase 2) e da refatoração (Fase 3).

## Escopo

Seis itens, todos localizados em `src/providers/*`, `src/providers/index.ts`,
`src/install.ts` e `src/spinner.ts`.

### 1. Amp — `Invalid Date` derruba o provider (`src/providers/amp.ts:70-86`)

**Causa raiz:** em `parseAmpFreeTier`, quando `effectiveRate` é `0`, o cálculo
`hoursToFull = (total - remaining) / effectiveRate` produz `Infinity`. Em seguida,
`new Date(Date.now() + Infinity)` gera um `Invalid Date`, e `.toISOString()` **lança
`RangeError`**. A exceção é capturada pelo `catch` de `fetchUsage`, que retorna o erro
genérico `Failed to parse usage` — o provider Amp inteiro falha por causa de um cálculo
de ETA.

**Correção:**
- Só construir a data quando `effectiveRate > 0` e `Number.isFinite(hoursToFull)`;
  caso contrário, `fullAt = null`.
- `parseAmpFreeTier` é chamado duas vezes com os mesmos argumentos (linhas 91 e 104),
  recomputando `parseFloat`. Computar `remaining`/`total`/resultado uma vez só e
  reaproveitar nos dois pontos de uso.
- Não introduzir `try/catch` novo; a correção é no cálculo (causa raiz).

### 2. Codex — `catch {}` silencioso em três pontos (`src/providers/codex.ts`)

**Causa raiz:** três blocos `catch {}` engolem erros sem rastro:
- `codex.ts:132` — `JSON.parse` de linha `.jsonl` corrompida em `extractRateLimits`.
- `codex.ts:388-390` — `proc.stdin.write` falhando (processo app-server morto).
- `codex.ts:427-429` — `JSON.parse` de linha não-JSON vinda do app-server.

Não escondem *o* erro real, mas tornam impossível diagnosticar "app-server quebrado"
vs "sessão corrompida" vs "linha de log do app-server".

**Correção:** substituir cada `catch {}` por `catch (error) { logger.debug(...) }` com
contexto relevante (caminho do arquivo, trecho da linha truncado). `logger.debug` vai
para stderr, não polui o JSON do Waybar em stdout, e só aparece em modo verboso —
satisfaz o princípio "sem catch silencioso" sem virar ruído em uso normal.

### 3. Codex — `as any` no parse JSON-RPC (`src/providers/codex.ts:398`)

**Causa raiz:** `const msg = JSON.parse(line) as any` desabilita checagem de tipo em
todos os acessos subsequentes (`msg.id`, `msg.result`, `msg.result.rateLimits`).

**Correção:** definir uma interface local `JsonRpcResponse { id?: number | string;
result?: unknown; error?: { message?: string } }` e estreitar `msg.result` com type
guards antes de cada cast específico. O `copilot.ts` já tem exatamente esse padrão
(`JsonRpcMessage`) — seguir o mesmo estilo para consistência.

### 4. Codex — `!` non-null em credits (`src/providers/codex.ts:486-490`)

**Causa raiz:** após o guard `if (limits.credits?.has_credits || ...)`, o bloco usa
`limits.credits!.balance` / `limits.credits!.unlimited` três vezes. O `!` é seguro
(o guard garante que `credits` existe), mas é frágil e ilegível.

**Correção:** extrair `const credits = limits.credits` antes do guard, checar
`if (credits && (credits.has_credits || parseFloat(credits.balance || '0') > 0))` e
usar `credits.balance` / `credits.unlimited` sem `!`. Comportamento idêntico.

### 5. `withTimeout()` vaza timer (`src/providers/index.ts:22-27`)

**Causa raiz:** no `Promise.race([promise, timeoutPromise])`, quando `promise` vence,
o `setTimeout` interno continua pendente. Um timer pendente mantém o event loop vivo,
atrasando a saída do processo em até `PROVIDER_TIMEOUT_MS` (10 s) após o trabalho
terminar.

**Correção (mínima, sem reescrever com `AbortController`):** capturar o handle do
`setTimeout` e chamar `clearTimeout` quando a corrida resolve — via `.finally()` na
promise de trabalho ou estrutura equivalente. Manter a assinatura e o comportamento
observável de `withTimeout`.

### 6. Trocar `ora` por `@clack/prompts` spinner

**Motivação:** `ora` (9.4.0) é redundante — `@clack/prompts` (1.3.0) já oferece
`p.spinner()` nativo. A API do clack cobre o uso atual:
- `s.start(message)` — inicia
- `s.stop(message, code)` — encerra (`code` 0 = sucesso, ≠ 0 = erro)
- `s.message(text)` — atualiza

**Correção:**
- `src/spinner.ts` é um wrapper de 12 linhas de `ora` com um único consumidor
  (`src/install.ts`, que já importa `* as p from '@clack/prompts'`). **Deletar
  `src/spinner.ts` por completo.**
- Em `src/install.ts`, substituir o uso de `createSpinner` por `p.spinner()` direto:
  - `createSpinner('Installing ...')` + `.start()` → `s.start('Installing ...')`
  - `.succeed('... ready')` → `s.stop('... ready', 0)`
  - `.fail('Failed ...')` → `s.stop('Failed ...', 1)`
- Remover `ora` de `dependencies` no `package.json`.
- Rodar `bun install` para atualizar `bun.lock`.

A UI fica visualmente consistente com o restante da TUI clack.

## Fora de escopo

- **`copilot.ts:344` — alegado loop infinito por `NaN`:** verificado e descartado. O
  regex `/content-length:\s*(\d+)/i` garante dígitos em `match[1]`, então
  `Number.parseInt` nunca retorna `NaN`. Existe apenas uma inconsistência cosmética: o
  caminho de recuperação procura `'Content-Length:'` case-sensitive enquanto o parse é
  case-insensitive. Pode ser corrigido com 1 linha como hardening opcional, sem
  prioridade.
- **Refatoração estrutural** (`BaseProvider`, builders compartilhados, quebra de
  arquivos grandes, mover filtro de modelos para fora dos formatters) — Fase 3.
- **Remoção de legacy** (`qbar` / `agent-bar-omarchy`) — Fase 2.
- **Cobertura de testes para `install.ts`/`setup.ts`/`update.ts`** — Fase 3.

## Plano de verificação

- **TDD:** escrever testes que falham antes da correção, para:
  - Amp: stdout com `replenishes +$0/hour` (rate zero) não deve lançar nem produzir
    `resetsAt` inválido — deve resultar em `resetsAt: null`.
  - Codex: `limits.credits` com `has_credits: false` e `balance: '0'` não deve produzir
    `extraUsage`; com credits válidos, deve produzir sem depender de `!`.
- **Testes focados:**
  `bun test tests/providers/amp.test.ts tests/providers/codex.test.ts tests/providers/codex-appserver.test.ts tests/providers/copilot.test.ts`
- **Contratos compartilhados:** `bun run typecheck`
- **Lint:** `bun run lint`
- **Antes do handoff da fase:** `bun test && bun run typecheck && bun run lint`

## Arquivos afetados

| Arquivo | Mudança |
| --- | --- |
| `src/providers/amp.ts` | Guard de `effectiveRate`/`hoursToFull`; dedup de `parseAmpFreeTier` |
| `src/providers/codex.ts` | 3× `catch {}` → `logger.debug`; interface `JsonRpcResponse`; remover `!` de credits |
| `src/providers/index.ts` | `clearTimeout` em `withTimeout()` |
| `src/install.ts` | Usar `p.spinner()` direto no lugar de `createSpinner` |
| `src/spinner.ts` | **Deletado** |
| `package.json` | Remover dependência `ora` |
| `bun.lock` | Atualizado por `bun install` |
| `tests/providers/amp.test.ts` | Teste do guard de ETA do Amp |
| `tests/providers/codex.test.ts` | Teste do guard de credits do Codex |

## Riscos conhecidos

- `src/install.ts` não tem teste automatizado; a troca de spinner é verificada apenas
  por `typecheck`/`lint` nesta fase. A diferença visual entre `ora` e o spinner do
  clack é aceita e desejada (consistência com a TUI).
- O comportamento de `s.stop(msg, code)` do clack para `code ≠ 0` deve ser confirmado
  contra a versão 1.3.0 instalada durante a implementação; se a API divergir, usar
  `s.error()` como alternativa documentada.
