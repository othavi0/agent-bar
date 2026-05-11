# Cleanup + Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminar duplicação real (thresholds, helpers visuais), remover provider-specific leak de `shared.ts`, deletar registry sem consumidor, e introduzir discriminated union em `ProviderQuota` movendo campos provider-specific pra `extra` tipado.

**Architecture:** Sequência de refactors independentes commit-a-commit. Cada task entrega valor isolado, deixa o build verde, e prepara o próximo. Ordem escolhida pra minimizar conflitos: thresholds centralizados primeiro (foundation), Codex helpers movidos (low risk), registry deletado (low risk), helpers atômicos compartilhados (depende de thresholds), discriminated union por último (toca mais arquivos), comment no caching (trivial).

**Tech Stack:** Bun, TypeScript, bun:test, biome. Mesmo do repo.

**Análise base:** sessão prévia gerou recomendações 1–6. Items 2, 5, 6, 1, 4 são ajustes reais; item 3 é confirmação de design intencional (no-op com comment).

---

## File Structure

**Modificações por task:**

| Task | Arquivos |
| --- | --- |
| 1 (#2 thresholds) | `src/config.ts`, `src/formatters/waybar.ts` |
| 2 (#5 codex helpers) | `src/formatters/shared.ts`, `src/formatters/codex-helpers.ts` (novo), `src/formatters/terminal.ts`, `src/formatters/waybar.ts`, `tests/formatters.test.ts` |
| 3 (#6 deletar registry) | `src/formatters/terminal.ts`, `src/formatters/waybar.ts`, `tests/formatters.test.ts` |
| 4 (#1 helpers atômicos) | `src/formatters/segments.ts` (novo), `src/formatters/terminal.ts`, `src/formatters/waybar.ts`, `tests/formatters-segments.test.ts` (novo) |
| 5 (#4 discriminated union) | `src/providers/types.ts`, `src/providers/claude.ts`, `src/providers/codex.ts`, `src/providers/amp.ts`, `src/formatters/terminal.ts`, `src/formatters/waybar.ts`, `src/formatters/shared.ts`, `tests/formatters.test.ts`, `tests/providers/*.test.ts` |
| 6 (#3 no-op comment) | `src/formatters/waybar.ts` |

Tasks sempre verificam: `bun test`, `bun run typecheck`, `bun run lint` antes do commit.

---

## Task 1: Centralizar thresholds de status (item #2)

**Files:**
- Modify: `src/config.ts`
- Modify: `src/formatters/waybar.ts`

Hoje `CONFIG.thresholds` (60/30/10) é usado pelo terminal formatter via `getColor` e `getColorForPercent`. Waybar.ts redeclara a mesma escala hardcoded em 3 lugares (`indicator`, `getClass`, `formatProviderForWaybar`). Não é bug visual — escalas estão alinhadas — mas é DRY-violation.

- [ ] **Step 1: Adicionar helpers semânticos em `config.ts`**

Em `src/config.ts`, abaixo de `getColorForPercent`:

```typescript
/** Status bucket derived from health percentage (0-100). Matches `CONFIG.thresholds`. */
export type HealthStatus = 'ok' | 'low' | 'warn' | 'critical';

export function getStatusForPercent(pct: number | null): HealthStatus {
  if (pct === null) return 'ok';
  if (pct < CONFIG.thresholds.orange) return 'critical';
  if (pct < CONFIG.thresholds.yellow) return 'warn';
  if (pct < CONFIG.thresholds.green) return 'low';
  return 'ok';
}
```

A função usa as constantes existentes (`thresholds.green=60`, `yellow=30`, `orange=10`) — sem números mágicos novos.

- [ ] **Step 2: Atualizar `indicator` em `src/formatters/waybar.ts`**

Substituir corpo de `indicator` (linhas ~73–81):

```typescript
function indicator(display: number | null, mode: DisplayMode): string {
  const health = toHealth(display, mode);
  if (health === null) return s(ONE_DARK.comment, BOX.dotO);
  const status = getStatusForPercent(health);
  const colorByStatus: Record<HealthStatus, string> = {
    critical: ONE_DARK.red,
    warn: ONE_DARK.orange,
    low: ONE_DARK.yellow,
    ok: ONE_DARK.green,
  };
  return s(colorByStatus[status], BOX.dot);
}
```

Importar `HealthStatus`, `getStatusForPercent` de `'../config'`.

- [ ] **Step 3: Atualizar `getClass` e `formatProviderForWaybar` no mesmo arquivo**

Substituir os blocos `if (val < 10) status = 'critical'; ...` (linhas ~427–434 e ~464–470) por:

```typescript
const status = getStatusForPercent(val);
// ...
const status = getStatusForPercent(health);
```

Onde `val`/`health` é `quota.primary?.remaining ?? 100` (já existente).

- [ ] **Step 4: Verificar**

```
bun test
bun run typecheck
bun run lint
```
Todos passam. Snapshots não devem mudar (mesma escala, mesmo output).

- [ ] **Step 5: Commit**

```
git add src/config.ts src/formatters/waybar.ts
git commit -m "refactor(config): centralizar getStatusForPercent + eliminar thresholds duplicados em waybar"
```

---

## Task 2: Mover Codex helpers para fora de `shared.ts` (item #5)

**Files:**
- Create: `src/formatters/codex-helpers.ts`
- Modify: `src/formatters/shared.ts` (remover `codexModelsFromQuota`, `applyCodexModelFilter`, `CodexModelEntry`)
- Modify: `src/formatters/terminal.ts` (atualizar import)
- Modify: `src/formatters/waybar.ts` (atualizar import)
- Modify: `tests/formatters.test.ts` se houver imports diretos

Objetivo: `shared.ts` fica apenas com helpers genuinamente shared entre providers. Lógica Codex-específica (conhece `modelsDetailed`, `primary`, `secondary`, `models` shape) vai pra arquivo dedicado.

- [ ] **Step 1: Criar `src/formatters/codex-helpers.ts`**

Move literalmente o bloco `CodexModelEntry` + `codexModelsFromQuota` + `applyCodexModelFilter` de `shared.ts` (linhas ~75–139 atuais) pra arquivo novo. Imports necessários:

```typescript
import type { ModelWindows, ProviderQuota, QuotaWindow } from '../providers/types';
import { classifyWindow } from './shared';

export interface CodexModelEntry {
  name: string;
  windows: ModelWindows;
  severity: number;
}

export function codexModelsFromQuota(p: ProviderQuota): CodexModelEntry[] { /* corpo movido */ }

export function applyCodexModelFilter(models: CodexModelEntry[], allowed?: string[]): CodexModelEntry[] { /* corpo movido */ }
```

`classifyWindow` é genérico (window-duration → bucket), fica em `shared.ts`.

- [ ] **Step 2: Atualizar imports**

`src/formatters/terminal.ts` e `src/formatters/waybar.ts` trocam:

```typescript
import { applyCodexModelFilter, codexModelsFromQuota, ... } from './shared';
```

por (duas linhas):

```typescript
import { applyCodexModelFilter, codexModelsFromQuota } from './codex-helpers';
import { /* resto */ } from './shared';
```

Manter `etaLabel`, `formatEta`, `formatPercent`, `formatResetTime`, `normalizePlanLabel`, `toDisplay`, `toHealth`, `DisplayMode`, `classifyWindow` em `shared.ts`.

Se algum teste importa `codexModelsFromQuota` direto, atualizar.

- [ ] **Step 3: Verificar**

```
bun test
bun run typecheck
bun run lint
```

Comportamento idêntico. Snapshots inalterados.

- [ ] **Step 4: Commit**

```
git add src/formatters/codex-helpers.ts src/formatters/shared.ts src/formatters/terminal.ts src/formatters/waybar.ts tests/
git commit -m "refactor(formatters): isolar helpers Codex em codex-helpers.ts"
```

---

## Task 3: Deletar registry sem consumidor (item #6)

**Files:**
- Modify: `src/formatters/terminal.ts`
- Modify: `src/formatters/waybar.ts`
- Modify: `tests/formatters.test.ts` se assertar registry exports

Hoje `registerTerminalBuilder` e `registerTooltipBuilder` são exportados mas só built-in providers (claude/codex/amp) registram, via side-effect dentro do mesmo arquivo. Sem plano concreto de plugin externo. Trocar `Map` mutável + função register por `Record` literal com fallback explícito.

- [ ] **Step 1: Refatorar `src/formatters/terminal.ts`**

Substituir:

```typescript
type TerminalBuilder = (p: ProviderQuota, mode: DisplayMode) => string[];

const terminalBuilders = new Map<string, TerminalBuilder>([
  ['claude', buildClaude],
  ['codex', buildCodex],
  ['amp', buildAmp],
]);

export function registerTerminalBuilder(providerId: string, builder: TerminalBuilder): void {
  terminalBuilders.set(providerId, builder);
}
```

por:

```typescript
type TerminalBuilder = (p: ProviderQuota, mode: DisplayMode) => string[];

const TERMINAL_BUILDERS: Record<string, TerminalBuilder> = {
  claude: buildClaude,
  codex: buildCodex,
  amp: buildAmp,
};
```

E `terminalBuilders.get(p.provider)` vira `TERMINAL_BUILDERS[p.provider]`.

- [ ] **Step 2: Refatorar `src/formatters/waybar.ts`**

Mesma mudança para `tooltipBuilders` / `registerTooltipBuilder`. Resultado:

```typescript
type TooltipBuilder = (p: ProviderQuota, fetchedAt: string | undefined, mode: DisplayMode) => string;

const TOOLTIP_BUILDERS: Record<string, TooltipBuilder> = {
  claude: buildClaudeTooltip,
  codex: buildCodexTooltip,
  amp: buildAmpTooltip,
};
```

`buildProviderTooltip` consulta `TOOLTIP_BUILDERS[p.provider] ?? buildGenericTooltip`.

- [ ] **Step 3: Verificar imports externos**

```
grep -rn "registerTerminalBuilder\|registerTooltipBuilder" src/ tests/
```

Espera-se zero resultado fora dos próprios formatters. Se aparecer em algum lugar, escalar como `NEEDS_CONTEXT` antes de seguir.

- [ ] **Step 4: Verificar**

```
bun test
bun run typecheck
bun run lint
```

- [ ] **Step 5: Commit**

```
git add src/formatters/terminal.ts src/formatters/waybar.ts
git commit -m "refactor(formatters): substituir registry mutável por Record estático (YAGNI)"
```

---

## Task 4: Helpers atômicos compartilhados via segments (item #1)

**Files:**
- Create: `src/formatters/segments.ts`
- Modify: `src/formatters/terminal.ts`
- Modify: `src/formatters/waybar.ts`
- Test: `tests/formatters-segments.test.ts` (novo)

Hoje `bar()`, `indicator()`, color-aware percent têm corpo equivalente em terminal.ts e waybar.ts — só muda o renderer (ANSI escape vs Pango `<span>`). Solução: helpers retornam estrutura neutra `Segment[]`, renderers ANSI/Pango finalizam.

- [ ] **Step 1: Criar `src/formatters/segments.ts`**

```typescript
import { CONFIG, getStatusForPercent, type HealthStatus } from '../config';
import { type DisplayMode, toHealth } from './shared';

/** Theme-neutral color token. Renderers map this to ANSI or hex. */
export type ColorToken = 'green' | 'yellow' | 'orange' | 'red' | 'comment' | 'text';

export interface Segment {
  text: string;
  color: ColorToken;
  bold?: boolean;
}

const STATUS_TO_COLOR: Record<HealthStatus, ColorToken> = {
  ok: 'green',
  low: 'yellow',
  warn: 'orange',
  critical: 'red',
};

export function colorForDisplay(display: number | null, mode: DisplayMode): ColorToken {
  const health = toHealth(display, mode);
  if (health === null) return 'text';
  return STATUS_TO_COLOR[getStatusForPercent(health)];
}

/** Build 20-wide quota bar segments. Empty when value is null. */
export function barSegments(display: number | null, mode: DisplayMode): Segment[] {
  if (display === null) return [{ text: '░'.repeat(20), color: 'comment' }];
  const filled = Math.floor(display / 5);
  return [
    { text: '█'.repeat(filled), color: colorForDisplay(display, mode) },
    { text: '░'.repeat(20 - filled), color: 'comment' },
  ];
}

/** Build single-dot indicator segments. Open dot when value is null. */
export function indicatorSegments(display: number | null, mode: DisplayMode): Segment[] {
  if (display === null) return [{ text: '○', color: 'comment' }];
  return [{ text: '●', color: colorForDisplay(display, mode) }];
}
```

Nota: os caracteres `○` / `●` são `BOX.dotO` / `BOX.dot`. Importar de `../theme` em vez de inline se preferir consistência.

- [ ] **Step 2: Adicionar testes em `tests/formatters-segments.test.ts`**

```typescript
import { describe, expect, it } from 'bun:test';
import { barSegments, colorForDisplay, indicatorSegments } from '../src/formatters/segments';

describe('segments helpers', () => {
  it('colorForDisplay: ok green when health >= 60', () => {
    expect(colorForDisplay(80, 'remaining')).toBe('green');
  });

  it('colorForDisplay: critical red when health < 10', () => {
    expect(colorForDisplay(5, 'remaining')).toBe('red');
  });

  it('colorForDisplay: respects used mode (display=95 -> health=5 -> red)', () => {
    expect(colorForDisplay(95, 'used')).toBe('red');
  });

  it('colorForDisplay: null -> text', () => {
    expect(colorForDisplay(null, 'remaining')).toBe('text');
  });

  it('barSegments: 20 chars total, filled proportional', () => {
    const segs = barSegments(50, 'remaining');
    const total = segs.map((s) => s.text.length).reduce((a, b) => a + b, 0);
    expect(total).toBe(20);
    expect(segs[0].text).toBe('█'.repeat(10));
  });

  it('barSegments: null -> all dimmed', () => {
    const segs = barSegments(null, 'remaining');
    expect(segs).toEqual([{ text: '░'.repeat(20), color: 'comment' }]);
  });

  it('indicatorSegments: filled dot uses health color', () => {
    expect(indicatorSegments(80, 'remaining')).toEqual([{ text: '●', color: 'green' }]);
  });

  it('indicatorSegments: null -> open dot', () => {
    expect(indicatorSegments(null, 'remaining')).toEqual([{ text: '○', color: 'comment' }]);
  });
});
```

Rodar: `bun test tests/formatters-segments.test.ts`. Esperado: FAIL (módulo ainda não existe quando testa antes do Step 1 — fazer em ordem: Step 1 cria, Step 2 testa direto). Pass após Step 1.

- [ ] **Step 3: Adaptar `src/formatters/terminal.ts`**

Adicionar renderer ANSI no topo:

```typescript
import { type ColorToken, type Segment, barSegments, colorForDisplay, indicatorSegments } from './segments';

const ANSI_BY_TOKEN: Record<ColorToken, string> = {
  green: ANSI.green,
  yellow: ANSI.yellow,
  orange: ANSI.orange,
  red: ANSI.red,
  comment: ANSI.comment,
  text: ANSI.text,
};

function renderAnsi(segs: Segment[]): string {
  return segs.map((s) => `${ANSI_BY_TOKEN[s.color]}${s.bold ? ANSI.bold : ''}${s.text}${ANSI.reset}`).join('');
}
```

Substituir helpers locais:

```typescript
function bar(display: number | null, mode: DisplayMode): string {
  return renderAnsi(barSegments(display, mode));
}

function indicator(display: number | null, mode: DisplayMode): string {
  return renderAnsi(indicatorSegments(display, mode));
}

function getColor(display: number | null, mode: DisplayMode): string {
  return ANSI_BY_TOKEN[colorForDisplay(display, mode)];
}
```

Builders (`buildClaude`, `buildCodex`, `buildAmp`, `buildGenericTerminal`) NÃO mudam — chamam mesmas funções.

- [ ] **Step 4: Adaptar `src/formatters/waybar.ts`**

Mesma estrutura:

```typescript
import { type ColorToken, type Segment, barSegments, colorForDisplay, indicatorSegments } from './segments';

const HEX_BY_TOKEN: Record<ColorToken, string> = {
  green: ONE_DARK.green,
  yellow: ONE_DARK.yellow,
  orange: ONE_DARK.orange,
  red: ONE_DARK.red,
  comment: ONE_DARK.comment,
  text: ONE_DARK.text,
};

function renderPango(segs: Segment[]): string {
  return segs.map((seg) => s(HEX_BY_TOKEN[seg.color], seg.text, seg.bold ?? false)).join('');
}
```

Substituir helpers locais:

```typescript
function bar(display: number | null, mode: DisplayMode): string {
  return renderPango(barSegments(display, mode));
}

function indicator(display: number | null, mode: DisplayMode): string {
  return renderPango(indicatorSegments(display, mode));
}

function colorFor(display: number | null, mode: DisplayMode): string {
  return HEX_BY_TOKEN[colorForDisplay(display, mode)];
}

function pctColored(display: number | null, mode: DisplayMode): string {
  return s(colorFor(display, mode), formatPercent(display));
}
```

- [ ] **Step 5: Verificar**

```
bun test
bun run typecheck
bun run lint
```

Snapshots de formatters não devem mudar — output visual idêntico (mesmas cores hex/ANSI, mesmos chars).

- [ ] **Step 6: Commit**

```
git add src/formatters/segments.ts src/formatters/terminal.ts src/formatters/waybar.ts tests/formatters-segments.test.ts
git commit -m "refactor(formatters): segments compartilhados + renderers ANSI/Pango"
```

---

## Task 5: Discriminated union em `ProviderQuota` com `extra` tipado (item #4)

**Files:**
- Modify: `src/providers/types.ts`
- Modify: `src/providers/claude.ts`
- Modify: `src/providers/codex.ts`
- Modify: `src/providers/amp.ts`
- Modify: `src/formatters/terminal.ts`
- Modify: `src/formatters/waybar.ts`
- Modify: `src/formatters/codex-helpers.ts` (criada em Task 2)
- Test: `tests/formatters.test.ts`, `tests/providers/*.test.ts`

Objetivo: campos provider-específicos (`weeklyModels`, `extraUsage`, `modelsDetailed`) saem da raiz de `ProviderQuota` e viram parte de um campo `extra` tipado por provider via discriminated union. Core fica mínimo.

- [ ] **Step 1: Definir tipos novos em `src/providers/types.ts`**

Substituir interface `ProviderQuota` por:

```typescript
interface QuotaCore {
  displayName: string;
  available: boolean;
  account?: string;
  plan?: string;
  planType?: string;
  error?: string;
  primary?: QuotaWindow;
  secondary?: QuotaWindow;
  models?: Record<string, QuotaWindow>;
}

export interface ClaudeQuotaExtra {
  weeklyModels?: Record<string, QuotaWindow>;
  extraUsage?: {
    enabled: boolean;
    remaining: number;
    limit: number;
    used: number;
  };
}

export interface CodexQuotaExtra {
  modelsDetailed?: Record<string, ModelWindows>;
  extraUsage?: {
    enabled: boolean;
    remaining: number;
    limit: number;
    used: number;
  };
}

export interface AmpQuotaExtra {
  meta?: Record<string, string>;
}

export interface ClaudeQuota extends QuotaCore { provider: 'claude'; extra?: ClaudeQuotaExtra; }
export interface CodexQuota  extends QuotaCore { provider: 'codex';  extra?: CodexQuotaExtra;  }
export interface AmpQuota    extends QuotaCore { provider: 'amp';    extra?: AmpQuotaExtra;    }
export interface GenericQuota extends QuotaCore { provider: string;  extra?: Record<string, unknown>; }

export type ProviderQuota = ClaudeQuota | CodexQuota | AmpQuota | GenericQuota;
```

Type narrowing: `if (p.provider === 'claude') { p.extra?.weeklyModels // typed }`.

Nota: `GenericQuota` cobre providers novos / mock no fallback. Manter `Provider` interface inalterado.

- [ ] **Step 2: Atualizar `src/providers/claude.ts`**

Onde antes setava `quota.weeklyModels = ...` e `quota.extraUsage = ...`, mover para `quota.extra = { weeklyModels, extraUsage }`. Garantir `provider: 'claude'` literal (não `this.id`) ou cast adequado:

```typescript
const result: ClaudeQuota = {
  provider: 'claude',
  displayName: this.name,
  available: true,
  primary: ...,
  secondary: ...,
  extra: {
    weeklyModels: ...,
    extraUsage: ...,
  },
};
return result;
```

Idem para Codex e Amp.

- [ ] **Step 3: Atualizar `src/providers/codex.ts`**

`modelsDetailed` e `extraUsage` movem para `extra`. `provider: 'codex'`. Mesma estrutura do Claude.

- [ ] **Step 4: Atualizar `src/providers/amp.ts`**

`meta` move para `extra.meta`. `provider: 'amp'`.

- [ ] **Step 5: Atualizar formatters**

`src/formatters/terminal.ts` `buildClaude`:
- `p.weeklyModels` → `p.extra?.weeklyModels`
- `p.extraUsage` → `p.extra?.extraUsage`

`buildCodex`:
- `p.extraUsage` → `p.extra?.extraUsage`

`buildAmp`:
- `p.meta ?? {}` → `p.extra?.meta ?? {}`

Mesma adaptação em `src/formatters/waybar.ts` para os três tooltip builders.

`src/formatters/codex-helpers.ts`:
- `p.modelsDetailed` → `p.extra?.modelsDetailed` (assinatura de função recebe `CodexQuota | ProviderQuota` — usar type guard inline ou ajustar).

Em todos os pontos, type narrowing por `if (p.provider === 'claude')` é suficiente. Onde builder já assume provider correto (ex: `buildClaude` só chamado quando `p.provider === 'claude'`), TypeScript narrow automático.

- [ ] **Step 6: Atualizar testes**

`tests/formatters.test.ts` mocks usam shape antiga. Atualizar `mockClaudeQuota` e similares:

```typescript
function mockClaudeQuota(remaining: number): ClaudeQuota {
  return {
    provider: 'claude',
    displayName: 'Claude',
    available: true,
    primary: { remaining, ... },
  };
}
```

Idem `tests/formatters-snapshot.test.ts` fixtures.

Tests de provider (`tests/providers/*.test.ts`): adaptar assertions que olham `.weeklyModels` etc para `.extra?.weeklyModels`.

- [ ] **Step 7: Verificar**

```
bun test
bun run typecheck
bun run lint
```

Snapshots podem precisar de regen se algum builder mudou ordem de output (ex: condição `p.extra?.foo` vs `p.foo` retornando `undefined` em ramos diferentes). Rodar `bun test tests/formatters-snapshot.test.ts -u` se necessário, inspecionar diff para garantir que mudança é só estrutural.

- [ ] **Step 8: Commit**

```
git add src/providers/ src/formatters/ tests/
git commit -m "refactor(types): discriminated union em ProviderQuota com extra tipado"
```

---

## Task 6: Documentar caching intencional (item #3)

**Files:**
- Modify: `src/formatters/waybar.ts`

Item #3 é no-op funcional: caching só em waybar é correto (waybar polled por waybar binary, outros são one-shot). Adicionar comment para evitar refactor errado no futuro.

- [ ] **Step 1: Adicionar JSDoc em `loadSettingsCached`**

Acima da função (~linha 22), trocar/adicionar:

```typescript
/**
 * Cached settings loader for the Waybar hot path.
 *
 * Waybar invokes `agent-bar-omarchy` on a tight polling interval (default a few
 * seconds), so reading settings.json from disk every call adds up. Cache TTL
 * (`SETTINGS_CACHE_TTL_MS`) makes hot runs O(1).
 *
 * Other entry points (refresh, action-right, index) are one-shot per invocation
 * and intentionally use `loadSettingsSync` directly — caching there is YAGNI.
 */
function loadSettingsCached(): ReturnType<typeof loadSettingsSync> { ... }
```

- [ ] **Step 2: Commit**

```
git add src/formatters/waybar.ts
git commit -m "docs(waybar): documentar caching intencional apenas no hot path"
```

---

## Self-Review

**Spec coverage:** Items 1–6 da análise prévia mapeiam 1:1 para Tasks 1–6 (em ordem otimizada por risco). Item #3 incluído como Task 6 (documentação no-op).

**Placeholder scan:** Nenhum TBD/TODO. Cada step tem código completo ou comando concreto.

**Type consistency:** `DisplayMode`, `Segment`, `ColorToken`, `HealthStatus`, `ClaudeQuota`/`CodexQuota`/`AmpQuota`/`GenericQuota` são introduzidos uma vez e referenciados consistentemente. `colorForDisplay` (segments.ts) usa `getStatusForPercent` (config.ts) introduzido na Task 1 — ordem de tasks respeita dependência.

**Decomposição:** cada task entrega valor isolado e deixa o build verde. Pode-se parar entre tasks sem deixar trabalho meio-feito. Task 5 é a mais arriscada (toca providers + formatters + tests + cache shape no disco? — não, cache armazena objeto serializado mas formato é compatível desde que readers se adaptem; verificar comportamento de leitura de cache legacy é parte do step 7).

**Não-objetivos:**
- Sem mudança em `Provider` interface, `cache.ts`, `settings.ts` core, TUI.
- Sem novos features visuais.
- Sem mudança em `getColorForPercent` semântica (escala continua 60/30/10).
- Sem plugin system (registry deletado).

## Verification antes do merge

Para handoff amplo: `bun test && bun run typecheck && bun run lint`. Por área: ver tabela em `AGENTS.md`.

## Riscos e mitigations

- **Task 5 cache compat:** se usuário tem `~/.cache/agent-bar-omarchy/*.json` com `weeklyModels` na raiz, próxima leitura ignora (não é mais reconhecido). Cache TTL 5min — usuário expira rápido. Documentar no commit msg de Task 5. Se quiser robustez extra, adicionar normalização no cache reader que migra shape antiga → nova on read (não está no plano por YAGNI).
- **Task 4 visual regression:** segments helpers devem produzir bytes idênticos pros mesmos inputs. Snapshots existentes são a rede de segurança.
- **Order ofTasks:** Tasks 1 e 4 têm dependência (Task 4 usa `getStatusForPercent` da Task 1). Tasks 2, 3, 5, 6 são independentes mas a ordem proposta minimiza conflicts (Task 5 por último, pq mexe nos mocks usados pelas outras).
