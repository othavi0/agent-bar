# Fase 1 — Correção e endurecimento — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Corrigir cinco defeitos reais do `agent-bar` (um bug que derruba o provider Amp, três pontos de baixa diagnosticabilidade/type-safety no Codex, um vazamento de timer) e remover a dependência redundante `ora`, sem alterar arquitetura.

**Architecture:** Mudanças cirúrgicas e isoladas. Cada task toca um arquivo (ou um par arquivo+teste), produz um commit focado e é verificável de forma independente. Nenhum boundary de módulo é movido — isso fica para a Fase 3.

**Tech Stack:** TypeScript strict, Bun (`bun:test`, `bun.lock`), `@clack/prompts` 1.3.0, Biome.

---

## Contexto para o engenheiro

O `agent-bar` é um monitor de quota LLM para Waybar. Providers (`src/providers/*.ts`)
buscam dados de quota e retornam `ProviderQuota`. Convenções relevantes:

- **Bun apenas.** Use `bun test`, `bun run typecheck`, `bun run lint`. Nunca `npm`/`node`.
- **stdout limpo:** o comando padrão emite JSON para o Waybar. Diagnóstico vai para
  stderr via `logger` (`src/logger.ts`), nunca `console.log`.
- **Erros de provider** são representados como campo `error` no objeto retornado,
  não lançados.
- Testes usam `bun:test` com `mock.module` e `spyOn`. Veja `tests/providers/amp.test.ts`
  e `tests/providers/codex.test.ts` como referência de padrão.
- Commits em Português, Conventional Commits, subject ≤ 50 caracteres.

Spec de origem: `docs/superpowers/specs/2026-05-15-fase-1-correcao-endurecimento-design.md`.

## Estrutura de arquivos

| Arquivo | Responsabilidade | Mudança nesta fase |
| --- | --- | --- |
| `src/providers/amp.ts` | Provider Amp: roda `amp usage`, faz parse do stdout | Guard de ETA + dedup |
| `tests/providers/amp.test.ts` | Testes do provider Amp | +1 teste de regressão |
| `src/providers/codex.ts` | Provider Codex: app-server + fallback de session log | Logging + tipos |
| `src/providers/index.ts` | Orquestração: timeout, retry, agregação | Fix de timer |
| `src/install.ts` | Instalação de CLIs externos com spinner | Usa spinner do clack |
| `src/spinner.ts` | Wrapper de `ora` (único consumidor: `install.ts`) | **Deletado** |
| `package.json` | Manifesto | Remove dependência `ora` |
| `bun.lock` | Lockfile | Regenerado por `bun install` |

---

## Task 1: Amp — guard contra ETA infinita

Quando `amp usage` reporta `replenishes +$0/hour`, `effectiveRate` vira `0`,
`hoursToFull` vira `Infinity`, e `new Date(...).toISOString()` **lança `RangeError`**.
O `catch` de `fetchUsage` engole a exceção e o provider Amp inteiro falha com
"Failed to parse usage". O fix guarda o cálculo; de quebra, elimina a chamada
duplicada de `parseAmpFreeTier`.

**Files:**
- Modify: `src/providers/amp.ts:78-85` (guard) e `src/providers/amp.ts:88-117` (dedup)
- Test: `tests/providers/amp.test.ts`

- [ ] **Step 1: Escrever o teste que falha**

Em `tests/providers/amp.test.ts`, adicionar a constante de stdout logo após
`OUTPUT_FULL_QUOTA` (perto da linha 43):

```typescript
const OUTPUT_ZERO_REPLENISH = [
  'Signed in as user@email.com',
  'Amp Free: $3.50/$5.00 remaining',
  'replenishes +$0/hour',
].join('\n');
```

E adicionar este teste dentro do `describe('fullAt ETA calculation', ...)`
(depois do teste `'returns null resetsAt when no replenish rate'`, perto da linha 380):

```typescript
    it('returns null resetsAt and stays available when replenish rate is 0', async () => {
      spawnSpy = spyOn(Bun, 'spawn').mockReturnValue(makeMockProc(OUTPUT_ZERO_REPLENISH, 0) as any);

      const result = await provider.getQuota();

      // Antes do fix: parseAmpFreeTier lança RangeError -> available:false.
      expect(result.available).toBe(true);
      expect(result.primary?.remaining).toBe(70);
      expect(result.primary?.resetsAt).toBeNull();
    });
```

- [ ] **Step 2: Rodar o teste e confirmar que falha**

Run: `bun test tests/providers/amp.test.ts -t "replenish rate is 0"`
Expected: FAIL — `expect(result.available).toBe(true)` recebe `false` (o provider
caiu no `catch` de "Failed to parse usage").

- [ ] **Step 3: Adicionar o guard em `parseAmpFreeTier`**

Em `src/providers/amp.ts`, dentro de `parseAmpFreeTier`, substituir o bloco
(linhas ~78-85):

```typescript
        let fullAt: string | null = null;
        if (replenish && remaining < total) {
          const ratePerHour = parseFloat(replenish[1]);
          const effectiveRate = bonusM ? ratePerHour * (1 + parseInt(bonusM[1], 10) / 100) : ratePerHour;
          const hoursToFull = (total - remaining) / effectiveRate;
          fullAt = new Date(Date.now() + hoursToFull * 3_600_000).toISOString();
        }
        return { pct, fullAt };
```

por:

```typescript
        let fullAt: string | null = null;
        if (replenish && remaining < total) {
          const ratePerHour = parseFloat(replenish[1]);
          const effectiveRate = bonusM ? ratePerHour * (1 + parseInt(bonusM[1], 10) / 100) : ratePerHour;
          const hoursToFull = (total - remaining) / effectiveRate;
          if (effectiveRate > 0 && Number.isFinite(hoursToFull)) {
            fullAt = new Date(Date.now() + hoursToFull * 3_600_000).toISOString();
          }
        }
        return { pct, fullAt };
```

- [ ] **Step 4: Eliminar a chamada duplicada de `parseAmpFreeTier`**

Em `src/providers/amp.ts`, substituir o bloco (linhas ~88-117):

```typescript
      let primary: QuotaWindow | undefined;

      if (freeMatch) {
        const { pct, fullAt } = parseAmpFreeTier(freeMatch, replenishMatch, bonusMatch);
        primary = { remaining: pct, resetsAt: fullAt };
      }

      const creditsMatch = stdout.match(/Individual credits:\s*\$([0-9.]+)\s*remaining/);

      const models: Record<string, QuotaWindow> = {};
      const meta: Record<string, string> = {};
      const extra: import('./types').AmpQuotaExtra = {};

      if (freeMatch) {
        const remaining = parseFloat(freeMatch[1]);
        const total = parseFloat(freeMatch[2]);
        const { pct, fullAt } = parseAmpFreeTier(freeMatch, replenishMatch, bonusMatch);

        models['Free Tier'] = { remaining: pct, resetsAt: fullAt };
        meta.freeRemaining = `$${remaining}`;
        meta.freeTotal = `$${total}`;
        if (replenishRate) meta.replenishRate = replenishRate;
        if (bonus) meta.bonus = bonus;
      }

      if (creditsMatch) {
        const balance = parseFloat(creditsMatch[1]);
        models.Credits = { remaining: balance > 0 ? 100 : 0, resetsAt: null };
        meta.creditsBalance = `$${balance}`;
      }
```

por:

```typescript
      const creditsMatch = stdout.match(/Individual credits:\s*\$([0-9.]+)\s*remaining/);

      const models: Record<string, QuotaWindow> = {};
      const meta: Record<string, string> = {};
      const extra: import('./types').AmpQuotaExtra = {};
      let primary: QuotaWindow | undefined;

      if (freeMatch) {
        const remaining = parseFloat(freeMatch[1]);
        const total = parseFloat(freeMatch[2]);
        const { pct, fullAt } = parseAmpFreeTier(freeMatch, replenishMatch, bonusMatch);

        primary = { remaining: pct, resetsAt: fullAt };
        models['Free Tier'] = { remaining: pct, resetsAt: fullAt };
        meta.freeRemaining = `$${remaining}`;
        meta.freeTotal = `$${total}`;
        if (replenishRate) meta.replenishRate = replenishRate;
        if (bonus) meta.bonus = bonus;
      }

      if (creditsMatch) {
        const balance = parseFloat(creditsMatch[1]);
        models.Credits = { remaining: balance > 0 ? 100 : 0, resetsAt: null };
        meta.creditsBalance = `$${balance}`;
      }
```

- [ ] **Step 5: Rodar testes e verificar que passam**

Run: `bun test tests/providers/amp.test.ts`
Expected: PASS — todos os testes do Amp, incluindo o novo. A suíte cobre ETA com
bônus, sem bônus, quota cheia e sem replenish; todos devem continuar verdes
(prova que o dedup não mudou comportamento).

- [ ] **Step 6: Typecheck e lint**

Run: `bun run typecheck && bun run lint`
Expected: PASS, sem erros.

- [ ] **Step 7: Commit**

```bash
git add src/providers/amp.ts tests/providers/amp.test.ts
git commit -m "fix: corrige ETA infinita no provider Amp"
```

---

## Task 2: Codex — registrar erros antes silenciados

`src/providers/codex.ts` tem três blocos `catch {}` que engolem erros sem rastro:
parse de linha `.jsonl` corrompida, falha de escrita em stdin do app-server, e
linha não-JSON do app-server. Não escondem o erro real, mas impedem diagnóstico.
A correção troca cada um por `logger.debug` — vai para stderr, não polui o JSON do
Waybar, e só aparece em modo verboso. `logger` já está importado em `codex.ts:7`.

Não há teste novo: os testes existentes mockam `logger` com no-ops e cobrem os
caminhos de sucesso/erro. A mudança é de diagnosticabilidade, verificada por
`typecheck`/`lint` e pela suíte continuar verde.

**Files:**
- Modify: `src/providers/codex.ts:132`, `src/providers/codex.ts:386-390`, `src/providers/codex.ts:427-429`

- [ ] **Step 1: Logar linha de session corrompida (`extractRateLimits`)**

Em `src/providers/codex.ts`, dentro de `extractRateLimits`, substituir:

```typescript
        } catch {}
```

por:

```typescript
        } catch (error) {
          logger.debug('Skipped unparseable Codex session line', { error, filePath });
        }
```

- [ ] **Step 2: Logar falha de escrita no stdin do app-server (`send`)**

Em `src/providers/codex.ts`, dentro da função `send` de `fetchRateLimitsViaAppServer`,
substituir:

```typescript
        try {
          proc.stdin.write(`${JSON.stringify(msg)}\n`);
        } catch {
          // ignore
        }
```

por:

```typescript
        try {
          proc.stdin.write(`${JSON.stringify(msg)}\n`);
        } catch (error) {
          logger.debug('Codex app-server stdin write failed', { error });
        }
```

- [ ] **Step 3: Logar linha não-JSON do app-server (`rl.on('line')`)**

Em `src/providers/codex.ts`, no handler `rl.on('line', ...)`, substituir:

```typescript
        } catch {
          // ignore non-json / unrelated messages
        }
```

por:

```typescript
        } catch (error) {
          logger.debug('Skipped non-JSON Codex app-server line', { error });
        }
```

- [ ] **Step 4: Typecheck, lint e testes do Codex**

Run: `bun test tests/providers/codex.test.ts tests/providers/codex-appserver.test.ts && bun run typecheck && bun run lint`
Expected: PASS — comportamento inalterado, nenhum teste quebra.

- [ ] **Step 5: Commit**

```bash
git add src/providers/codex.ts
git commit -m "refactor: loga falhas antes silenciadas no Codex"
```

---

## Task 3: Codex — endurecer tipos do parse JSON-RPC

`src/providers/codex.ts:398` usa `JSON.parse(line) as any`, desabilitando checagem
de tipo. E `codex.ts:486-490` usa `limits.credits!` três vezes (non-null assertion).
Ambas são mudanças de type-safety sem alteração de comportamento, verificadas por
`typecheck` e pela suíte existente (`codex.test.ts` cobre todos os casos de credits;
`codex-appserver.test.ts` cobre o parse do app-server).

**Files:**
- Modify: `src/providers/codex.ts` — adicionar helper/interface no escopo de módulo;
  trocar `as any` no handler `rl.on('line')`; reescrever o bloco de credits em `getQuota`

- [ ] **Step 1: Adicionar helper `isRecord` e interface `CodexAppServerResponse`**

Em `src/providers/codex.ts`, no escopo de módulo, logo após a interface
`CodexSessionEvent` (perto da linha 68, antes de `export class CodexProvider`),
adicionar:

```typescript
interface CodexAppServerResponse {
  id?: number | string;
  result?: unknown;
  error?: { message?: string } | null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}
```

- [ ] **Step 2: Tipar o parse no handler `rl.on('line')`**

Em `src/providers/codex.ts`, no handler `rl.on('line', (line: string) => { ... })`,
substituir o bloco:

```typescript
        try {
          const msg = JSON.parse(line) as any;

          if (msg?.id === 0 && msg?.result) {
            send({ method: 'initialized', params: {} });
            send({ method: 'account/read', id: 1, params: { refreshToken: false } });
            send({ method: 'account/rateLimits/read', id: 2, params: {} });
            return;
          }

          if (msg?.id === 1 && msg?.result) {
            const accountResult = msg.result as CodexAppServerAccountReadResult;
            accountPlanType = accountResult.account?.planType ?? null;
            if (rateLimitsResult) tryResolve();
            return;
          }

          if (msg?.id === 2 && msg?.result && (msg.result.rateLimits || msg.result.rateLimitsByLimitId)) {
            rateLimitsResult = msg.result as CodexAppServerRateLimitsReadResult;
```

por:

```typescript
        try {
          const msg = JSON.parse(line) as CodexAppServerResponse;

          if (msg?.id === 0 && msg.result) {
            send({ method: 'initialized', params: {} });
            send({ method: 'account/read', id: 1, params: { refreshToken: false } });
            send({ method: 'account/rateLimits/read', id: 2, params: {} });
            return;
          }

          if (msg?.id === 1 && msg.result) {
            const accountResult = msg.result as CodexAppServerAccountReadResult;
            accountPlanType = accountResult.account?.planType ?? null;
            if (rateLimitsResult) tryResolve();
            return;
          }

          if (msg?.id === 2 && isRecord(msg.result) && (msg.result.rateLimits || msg.result.rateLimitsByLimitId)) {
            rateLimitsResult = msg.result as CodexAppServerRateLimitsReadResult;
```

(O resto do bloco — `if (accountPlanType !== undefined) { ... }` etc. — fica inalterado.)

- [ ] **Step 3: Remover os `!` non-null do bloco de credits**

Em `src/providers/codex.ts`, dentro de `getQuota`, substituir o bloco
(perto das linhas 484-493):

```typescript
    let codexCredits: import('./types').CodexQuotaExtra['extraUsage'] | undefined;
    if (limits.credits?.has_credits || parseFloat(limits.credits?.balance || '0') > 0) {
      const balance = parseFloat(limits.credits!.balance);
      codexCredits = {
        enabled: true,
        remaining: limits.credits!.unlimited ? 100 : Math.min(100, Math.round(balance)),
        limit: limits.credits!.unlimited ? -1 : 0,
        used: 0,
      };
    }
```

por:

```typescript
    let codexCredits: import('./types').CodexQuotaExtra['extraUsage'] | undefined;
    const credits = limits.credits;
    if (credits && (credits.has_credits || parseFloat(credits.balance || '0') > 0)) {
      const balance = parseFloat(credits.balance);
      codexCredits = {
        enabled: true,
        remaining: credits.unlimited ? 100 : Math.min(100, Math.round(balance)),
        limit: credits.unlimited ? -1 : 0,
        used: 0,
      };
    }
```

A condição é logicamente idêntica: quando `credits` é `undefined`, a original
avaliava `undefined || parseFloat('0') > 0` → `false`, e a nova avalia
`undefined && ...` → `false`.

- [ ] **Step 4: Typecheck, lint e testes do Codex**

Run: `bun test tests/providers/codex.test.ts tests/providers/codex-appserver.test.ts && bun run typecheck && bun run lint`
Expected: PASS — em especial os testes `getQuota() credits handling` (que cobrem
`has_credits` true/false, `balance` zero/positivo, `unlimited`) devem todos passar,
provando que a remoção dos `!` não mudou comportamento.

- [ ] **Step 5: Commit**

```bash
git add src/providers/codex.ts
git commit -m "refactor: endurece tipos do parse JSON-RPC Codex"
```

---

## Task 4: Corrigir vazamento de timer em `withTimeout`

`src/providers/index.ts:22-27` — em `Promise.race([promise, timeoutPromise])`,
quando `promise` vence, o `setTimeout` interno continua pendente. Um timer pendente
mantém o event loop vivo, atrasando a saída do processo em até 10s
(`PROVIDER_TIMEOUT_MS`) após o trabalho terminar. O fix captura o handle e chama
`clearTimeout` quando a corrida resolve.

`withTimeout` é uma função privada (não exportada) sem arquivo de teste dedicado;
a correção é verificada por `typecheck`/`lint` e pela suíte completa continuar verde.

**Files:**
- Modify: `src/providers/index.ts:22-27`

- [ ] **Step 1: Reescrever `withTimeout` limpando o timer**

Em `src/providers/index.ts`, substituir:

```typescript
function withTimeout<T>(promise: Promise<T>, ms: number, label: string): Promise<T> {
  return Promise.race([
    promise,
    new Promise<T>((_, reject) => setTimeout(() => reject(new Error(`${label} timed out after ${ms}ms`)), ms)),
  ]);
}
```

por:

```typescript
function withTimeout<T>(promise: Promise<T>, ms: number, label: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<T>((_, reject) => {
    timer = setTimeout(() => reject(new Error(`${label} timed out after ${ms}ms`)), ms);
  });
  return Promise.race([promise, timeout]).finally(() => {
    if (timer) clearTimeout(timer);
  });
}
```

- [ ] **Step 2: Typecheck, lint e suíte completa**

Run: `bun test && bun run typecheck && bun run lint`
Expected: PASS — toda a suíte. `withTimeout` mantém assinatura e comportamento
observável (resolve com o valor, ou rejeita com "timed out after Nms").

- [ ] **Step 3: Commit**

```bash
git add src/providers/index.ts
git commit -m "fix: limpa timer pendente em withTimeout"
```

---

## Task 5: Trocar `ora` pelo spinner do `@clack/prompts`

`ora` é redundante: `@clack/prompts` (já dependência do projeto) oferece `spinner()`
nativo. `src/spinner.ts` é um wrapper de 12 linhas com um único consumidor,
`src/install.ts`, que já importa `* as p from '@clack/prompts'`. Deletamos
`src/spinner.ts`, usamos `p.spinner()` direto, e removemos `ora` do `package.json`.

API do clack (validada na doc da versão 1.3.0): `s.start(msg)`, `s.message(text)`,
`s.stop(msg, code)` — `code` 0 = sucesso, ≠ 0 = erro.

**Files:**
- Delete: `src/spinner.ts`
- Modify: `src/install.ts` (remover import, trocar uso do spinner)
- Modify: `package.json` (remover `ora`)
- Regenerate: `bun.lock`

- [ ] **Step 1: Confirmar que `install.ts` é o único consumidor**

Run: `grep -rn "createSpinner\|from './spinner'\|from \"ora\"\|from 'ora'" src tests`
Expected: somente ocorrências em `src/install.ts` e `src/spinner.ts`. Se aparecer
qualquer outro arquivo, PARE e reporte — o escopo desta task muda.

- [ ] **Step 2: Atualizar `src/install.ts`**

Remover a linha de import (linha 4):

```typescript
import { createSpinner } from './spinner';
```

E substituir, dentro de `ensureBunGlobalPackage`, o bloco:

```typescript
  const spinner = createSpinner(`Installing ${label ?? pkg}...`);
  spinner.start();

  try {
    const code = await runInteractive('bun', ['add', '-g', pkg]);
    if (code === 0 && (await hasCmd(bin))) {
      spinner.succeed(`${label ?? pkg} ready`);
      return true;
    }

    spinner.fail(`Failed to install ${label ?? pkg}`);
    return false;
  } catch {
    spinner.fail(`Failed to install ${label ?? pkg}`);
    return false;
  }
```

por:

```typescript
  const spinner = p.spinner();
  spinner.start(`Installing ${label ?? pkg}...`);

  try {
    const code = await runInteractive('bun', ['add', '-g', pkg]);
    if (code === 0 && (await hasCmd(bin))) {
      spinner.stop(`${label ?? pkg} ready`, 0);
      return true;
    }

    spinner.stop(`Failed to install ${label ?? pkg}`, 1);
    return false;
  } catch {
    spinner.stop(`Failed to install ${label ?? pkg}`, 1);
    return false;
  }
```

- [ ] **Step 3: Deletar `src/spinner.ts`**

Run: `rm src/spinner.ts`

- [ ] **Step 4: Remover `ora` do `package.json`**

Em `package.json`, no bloco `"dependencies"`, remover a linha:

```json
    "ora": "9.4.0"
```

Atenção à vírgula: `"dependencies"` deve continuar JSON válido (a entrada anterior,
`"@clack/prompts": "1.3.0"`, não pode terminar com vírgula sobrando).

- [ ] **Step 5: Regenerar o lockfile**

Run: `bun install`
Expected: `bun.lock` atualizado, `ora` removido de `node_modules`.

- [ ] **Step 6: Typecheck, lint e suíte completa**

Run: `bun test && bun run typecheck && bun run lint`
Expected: PASS — nenhum import quebrado, nenhuma referência pendente a `ora` ou
`./spinner`.

- [ ] **Step 7: Commit**

```bash
git add src/install.ts src/spinner.ts package.json bun.lock
git commit -m "refactor: troca ora pelo spinner do clack"
```

---

## Task 6: Verificação final da fase

Confirma que toda a Fase 1 está consistente antes do handoff para a Fase 2.

**Files:** nenhum (apenas verificação).

- [ ] **Step 1: Suíte completa + typecheck + lint**

Run: `bun test && bun run typecheck && bun run lint`
Expected: PASS — todos os testes verdes, zero erro de tipo, zero issue de lint.

- [ ] **Step 2: Confirmar ausência de resíduos**

Run: `grep -rn "ora" package.json && grep -rn "as any" src/providers/codex.ts; echo "---"; ls src/spinner.ts 2>&1`
Expected: nenhuma linha de `ora` em `package.json`; nenhum `as any` em `codex.ts`;
`ls` reporta que `src/spinner.ts` não existe.

- [ ] **Step 3: Confirmar histórico**

Run: `git log --oneline -6`
Expected: os cinco commits da fase (`fix: corrige ETA infinita...`,
`refactor: loga falhas...`, `refactor: endurece tipos...`,
`fix: limpa timer pendente...`, `refactor: troca ora...`) acima do commit do spec.

---

## Self-Review (preenchido pelo autor do plano)

**Cobertura do spec:**
- Bug 1 (Amp `Invalid Date`) → Task 1 ✓
- Bug 2 (Codex 3× `catch {}`) → Task 2 ✓
- Bug 3 (Codex `as any`) → Task 3, Steps 1-2 ✓
- Bug 4 (Codex `!` non-null) → Task 3, Step 3 ✓
- Bug 5 (`withTimeout` timer leak) → Task 4 ✓
- Item 6 (`ora` → clack) → Task 5 ✓
- "Fora de escopo" (Copilot, refactor estrutural) → não há tasks, correto.

**Placeholders:** nenhum — todo passo de código mostra o código completo.

**Consistência de tipos:** `CodexAppServerResponse`, `isRecord`,
`CodexAppServerAccountReadResult` e `CodexAppServerRateLimitsReadResult` (estas duas
já existem em `codex.ts`) são usadas de forma consistente entre os steps da Task 3.
`p.spinner()` da Task 5 usa a API `start`/`stop` validada na doc do clack 1.3.0.

**Risco conhecido:** `src/install.ts` não tem teste automatizado; a troca do spinner
é coberta só por `typecheck`/`lint`. A diferença visual entre `ora` e o spinner do
clack é aceita (consistência com a TUI). Se `s.stop(msg, code)` divergir na versão
1.3.0 instalada, a alternativa documentada no spec é `s.error(msg)`.
