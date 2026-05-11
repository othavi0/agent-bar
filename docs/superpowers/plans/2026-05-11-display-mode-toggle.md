# Display Mode Toggle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Adicionar setting `waybar.displayMode: 'remaining' | 'used'` (default `'remaining'`) que inverte a apresentação de quotas em terminal e Waybar — quando `'used'`, 0% = não usei nada, 100% = esgotado, cores invertidas, barra enche conforme uso, label "Resets in" no lugar de "Full in".

**Architecture:** Domínio inalterado (`QuotaWindow.remaining` continua = restante). Conversão acontece só no formatter via helpers `toDisplay` / `toHealth` em `src/formatters/shared.ts`. Cor usa thresholds de saúde existentes em `config.ts`, sem mudança lá. `mode` propaga de `loadSettings()` em `src/index.ts` / `src/refresh.ts` / `src/action-right.ts` como parâmetro explícito para os formatters; valor default `'remaining'` mantém testes e callers atuais funcionando.

**Tech Stack:** Bun, TypeScript, bun:test, clack/prompts (TUI), biome (lint).

**Spec:** `docs/superpowers/specs/2026-05-11-display-mode-toggle-design.md`.

---

## File Structure

**Modificações:**

- `src/settings.ts` — tipo `DisplayMode`, campo `waybar.displayMode`, default, validador, persistência.
- `src/formatters/shared.ts` — exporta `toDisplay`, `toHealth`; `formatEta` ganha parâmetro opcional `mode` para mudar label "Full" → "Resets" interno.
- `src/formatters/terminal.ts` — `formatForTerminal` aceita `mode`; helpers locais `bar` / `indicator` / `getColor` recebem `mode`; todos os call sites passam `mode`. Label "Full in" → "Resets in" no bloco Amp Free Tier.
- `src/formatters/waybar.ts` — análogo a terminal.ts. Mesmo padrão.
- `src/index.ts`, `src/refresh.ts`, `src/action-right.ts` — leem `settings.waybar.displayMode` e passam para os formatters.
- `src/tui/configure-layout.ts` — novo passo: seleção de display mode antes de aplicar.

**Testes:**

- `tests/settings.test.ts` — default, validação, persistência do novo campo.
- `tests/formatters.test.ts` — `toDisplay` / `toHealth`; cor invertida; bar invertida; label "Resets in".
- `tests/formatters-snapshot.test.ts` — snapshots `*-used.snap` cobrindo terminal e Waybar para Claude (cheio/parcial/esgotado), Codex (parcial), Amp (parcial).

---

## Task 1: Setting schema + default

**Files:**
- Modify: `src/settings.ts`
- Test: `tests/settings.test.ts`

- [ ] **Step 1: Write failing test for default**

Adicionar a `tests/settings.test.ts`:

```typescript
import { describe, expect, it } from 'bun:test';
// (manter imports existentes)

describe('Settings displayMode', () => {
  it('default is "remaining" when not set', async () => {
    process.env.XDG_CONFIG_HOME = await Bun.file('/tmp').exists()
      ? '/tmp/abo-test-displaymode-default'
      : process.env.XDG_CONFIG_HOME;
    const { loadSettings } = await import('../src/settings');
    const s = await loadSettings();
    expect(s.waybar.displayMode).toBe('remaining');
  });

  it('rejects invalid value, falls back to "remaining"', async () => {
    const { loadSettings, saveSettings } = await import('../src/settings');
    const s = await loadSettings();
    // @ts-expect-error testando valor inválido
    s.waybar.displayMode = 'bogus';
    await saveSettings(s);
    const reloaded = await loadSettings();
    expect(reloaded.waybar.displayMode).toBe('remaining');
  });

  it('persists "used" value round-trip', async () => {
    const { loadSettings, saveSettings } = await import('../src/settings');
    const s = await loadSettings();
    s.waybar.displayMode = 'used';
    await saveSettings(s);
    const reloaded = await loadSettings();
    expect(reloaded.waybar.displayMode).toBe('used');
  });
});
```

- [ ] **Step 2: Run test, expect failure**

```
bun test tests/settings.test.ts -t "displayMode"
```
Esperado: FAIL — `displayMode` não existe no schema.

- [ ] **Step 3: Implementar no settings.ts**

Em `src/settings.ts`, perto das constantes:

```typescript
export type DisplayMode = 'remaining' | 'used';
const VALID_DISPLAY_MODES = ['remaining', 'used'] as const;

function isValidDisplayMode(value: unknown): value is DisplayMode {
  return typeof value === 'string' && (VALID_DISPLAY_MODES as readonly string[]).includes(value);
}
```

Atualizar `Settings.waybar`:

```typescript
waybar: {
  providers: string[];
  showPercentage: boolean;
  separators: SeparatorStyle;
  providerOrder: string[];
  displayMode: DisplayMode;
};
```

`DEFAULT_SETTINGS.waybar` ganha `displayMode: 'remaining'`.

Em `normalizeSettings`, após validar separators:

```typescript
if (!isValidDisplayMode(merged.waybar.displayMode)) {
  merged.waybar.displayMode = DEFAULT_SETTINGS.waybar.displayMode;
}
```

- [ ] **Step 4: Rodar testes**

```
bun test tests/settings.test.ts
```
Esperado: PASS em todos os casos novos + existentes.

- [ ] **Step 5: Commit**

```
git add src/settings.ts tests/settings.test.ts
git commit -m "feat(settings): adicionar waybar.displayMode toggle"
```

---

## Task 2: Helpers `toDisplay` / `toHealth` + label de ETA

**Files:**
- Modify: `src/formatters/shared.ts`
- Test: `tests/formatters.test.ts`

- [ ] **Step 1: Test para helpers**

Adicionar a `tests/formatters.test.ts`:

```typescript
import { toDisplay, toHealth, etaLabel } from '../src/formatters/shared';

describe('display mode helpers', () => {
  it('toDisplay: remaining passes through', () => {
    expect(toDisplay(80, 'remaining')).toBe(80);
    expect(toDisplay(0, 'remaining')).toBe(0);
    expect(toDisplay(null, 'remaining')).toBe(null);
  });

  it('toDisplay: used inverts', () => {
    expect(toDisplay(80, 'used')).toBe(20);
    expect(toDisplay(0, 'used')).toBe(100);
    expect(toDisplay(100, 'used')).toBe(0);
    expect(toDisplay(null, 'used')).toBe(null);
  });

  it('toHealth: roundtrips display value back to health', () => {
    expect(toHealth(20, 'used')).toBe(80);
    expect(toHealth(80, 'remaining')).toBe(80);
    expect(toHealth(null, 'used')).toBe(null);
  });

  it('etaLabel: "Full in" when remaining, "Resets in" when used', () => {
    expect(etaLabel('remaining')).toBe('Full in');
    expect(etaLabel('used')).toBe('Resets in');
  });
});
```

- [ ] **Step 2: Rodar, esperar FAIL**

```
bun test tests/formatters.test.ts -t "display mode helpers"
```
Esperado: FAIL — exports não existem.

- [ ] **Step 3: Implementar em shared.ts**

Adicionar em `src/formatters/shared.ts`:

```typescript
import type { DisplayMode } from '../settings';
export type { DisplayMode };

export function toDisplay(remaining: number | null, mode: DisplayMode): number | null {
  if (remaining === null) return null;
  return mode === 'used' ? 100 - remaining : remaining;
}

export function toHealth(displayValue: number | null, mode: DisplayMode): number | null {
  if (displayValue === null) return null;
  return mode === 'used' ? 100 - displayValue : displayValue;
}

export function etaLabel(mode: DisplayMode): string {
  return mode === 'used' ? 'Resets in' : 'Full in';
}
```

- [ ] **Step 4: Rodar testes**

```
bun test tests/formatters.test.ts -t "display mode helpers"
```
Esperado: PASS.

- [ ] **Step 5: Commit**

```
git add src/formatters/shared.ts tests/formatters.test.ts
git commit -m "feat(formatters): helpers toDisplay/toHealth/etaLabel"
```

---

## Task 3: terminal.ts adapta para `mode`

**Files:**
- Modify: `src/formatters/terminal.ts`
- Test: `tests/formatters.test.ts`

- [ ] **Step 1: Test de comportamento em modo `used`**

Adicionar a `tests/formatters.test.ts`:

```typescript
describe('formatForTerminal displayMode=used', () => {
  it('shows used percentage (100 - remaining)', () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80)]); // 20% usado
    const result = formatForTerminal(quotas, 'used');
    expect(result).toContain('20%');
    expect(result).not.toMatch(/\b80%\b/);
  });

  it('colors used=90 as red (low health) not green', () => {
    const quotas = mockAllQuotas([mockClaudeQuota(10)]); // 90% usado
    const result = formatForTerminal(quotas, 'used');
    // health=10 → red threshold in CONFIG. ANSI.red presente.
    if (!process.env.NO_COLOR) {
      expect(result).toContain(ANSI.red);
    }
  });

  it('default arg keeps remaining behavior', () => {
    const quotas = mockAllQuotas([mockClaudeQuota(80)]);
    const result = formatForTerminal(quotas);
    expect(result).toContain('80%');
  });
});
```

- [ ] **Step 2: Rodar, esperar FAIL**

```
bun test tests/formatters.test.ts -t "formatForTerminal displayMode"
```
Esperado: FAIL — `formatForTerminal` não aceita 2º arg.

- [ ] **Step 3: Refatorar terminal.ts**

Em `src/formatters/terminal.ts`:

3a — atualizar imports:

```typescript
import { CONFIG } from '../config';
import type { AllQuotas, ProviderQuota, QuotaWindow } from '../providers/types';
import { loadSettingsSync, type WindowPolicy, type DisplayMode } from '../settings';
import { ANSI, BOX, PROVIDER_ANSI } from '../theme';
import {
  applyCodexModelFilter,
  codexModelsFromQuota,
  etaLabel,
  formatEta,
  formatPercent,
  formatResetTime,
  normalizePlanLabel,
  toDisplay,
  toHealth,
} from './shared';
```

3b — `getColor`, `bar`, `indicator` agora recebem valor de display + mode:

```typescript
function getColor(display: number | null, mode: DisplayMode): string {
  const health = toHealth(display, mode);
  if (health === null) return ANSI.text;
  if (health >= CONFIG.thresholds.green) return ANSI.green;
  if (health >= CONFIG.thresholds.yellow) return ANSI.yellow;
  if (health >= CONFIG.thresholds.orange) return ANSI.orange;
  return ANSI.red;
}

function bar(display: number | null, mode: DisplayMode): string {
  if (display === null) return `${ANSI.comment}${'░'.repeat(20)}${ANSI.reset}`;
  const filled = Math.floor(display / 5);
  const color = getColor(display, mode);
  return `${color}${'█'.repeat(filled)}${ANSI.comment}${'░'.repeat(20 - filled)}${ANSI.reset}`;
}

function indicator(display: number | null, mode: DisplayMode): string {
  if (display === null) return `${ANSI.comment}${BOX.dotO}${ANSI.reset}`;
  const color = getColor(display, mode);
  return `${color}${BOX.dot}${ANSI.reset}`;
}
```

3c — atualizar `modelLine` e `codexModelLine`:

```typescript
function modelLine(name: string, window: QuotaWindow | undefined, maxLen: number, vColor: string, mode: DisplayMode): string {
  const rem = window?.remaining ?? null;
  const reset = window?.resetsAt ?? null;
  const disp = toDisplay(rem, mode);
  const nameS = `${ANSI.textBright}${name.padEnd(maxLen)}${ANSI.reset}`;
  const barS = bar(disp, mode);
  const pctS = `${getColor(disp, mode)}${formatPercent(disp).padStart(4)}${ANSI.reset}`;
  const etaS = `${ANSI.cyan}→ ${formatEta(reset, rem)} ${formatResetTime(reset, rem)}${ANSI.reset}`;
  return `${v(vColor)}  ${indicator(disp, mode)} ${nameS} ${barS} ${pctS} ${etaS}`;
}

function codexModelLine(name: string, window: QuotaWindow | undefined, maxLen: number, vColor: string, mode: DisplayMode): string {
  const rem = window?.remaining ?? null;
  const disp = toDisplay(rem, mode);
  const nameS = `${ANSI.textBright}${name.padEnd(maxLen)}${ANSI.reset}`;
  const barS = bar(disp, mode);
  const pctS = `${getColor(disp, mode)}${formatPercent(disp).padStart(4)}${ANSI.reset}`;
  const etaS = window?.resetsAt
    ? `${ANSI.cyan}→ ${formatEta(window.resetsAt, rem)} ${formatResetTime(window.resetsAt, rem)}${ANSI.reset}`
    : `${ANSI.cyan}→ N/A${ANSI.reset}`;
  return `${v(vColor)}  ${indicator(disp, mode)} ${nameS} ${barS} ${pctS} ${etaS}`;
}
```

3d — atualizar `buildClaude`, `buildCodex`, `buildAmp`, `buildGenericTerminal` para receber `mode` e propagar. Em cada call de `bar`/`indicator`/`getColor`/`formatPercent`/`modelLine`/`codexModelLine`, calcular `disp = toDisplay(rem, mode)` e usar.

Exemplo de adaptação em `buildClaude` (bloco extraUsage, linhas ~102–109):

```typescript
if (p.extraUsage?.enabled && p.extraUsage.limit > 0) {
  const { remaining, used, limit } = p.extraUsage;
  const disp = toDisplay(remaining, mode);
  lines.push(v(vc));
  lines.push(label('Extra Usage', vc));
  const nameS = `${ANSI.textBright}${'Budget'.padEnd(maxLen)}${ANSI.reset}`;
  const barS = bar(disp, mode);
  const pctS = `${getColor(disp, mode)}${formatPercent(disp).padStart(4)}${ANSI.reset}`;
  const usedS = `${ANSI.cyan}$${(used / 100).toFixed(2)}/$${(limit / 100).toFixed(2)}${ANSI.reset}`;
  lines.push(`${v(vc)}  ${indicator(disp, mode)} ${nameS} ${barS} ${pctS} ${usedS}`);
}
```

Bloco Amp Free Tier (linhas ~217–220) — trocar label hardcoded:

```typescript
if (free.resetsAt && free.remaining !== 100) {
  subs.push(
    `${ANSI.cyan}${etaLabel(mode)} ${formatEta(free.resetsAt, free.remaining)}  ${formatResetTime(free.resetsAt, free.remaining)}${ANSI.reset}`,
  );
}
```

Bloco `buildGenericTerminal` (linha ~297):

```typescript
} else if (p.primary) {
  const rem = p.primary.remaining;
  const disp = toDisplay(rem, mode);
  const color = getColor(disp, mode);
  const suffix = mode === 'used' ? 'used' : 'remaining';
  lines.push(`${vi(vc)}  ${color}${formatPercent(disp)} ${suffix}${ANSI.reset}`);
}
```

3e — atualizar exports:

```typescript
export function formatForTerminal(quotas: AllQuotas, mode: DisplayMode = 'remaining'): string {
  const sections: string[][] = [];
  for (const p of quotas.providers) {
    if (!p.available && !p.error) continue;
    const builder = terminalBuilders.get(p.provider);
    sections.push(builder ? builder(p, mode) : buildGenericTerminal(p, mode));
  }
  if (sections.length === 0) {
    return `${ANSI.comment}No providers connected${ANSI.reset}`;
  }
  return sections.map((s) => s.join('\n')).join('\n\n');
}

export function outputTerminal(quotas: AllQuotas, mode: DisplayMode = 'remaining'): void {
  console.log(formatForTerminal(quotas, mode));
}
```

Tipo do registry:

```typescript
type TerminalBuilder = (p: ProviderQuota, mode: DisplayMode) => string[];
```

Atualizar a assinatura de `registerTerminalBuilder` para refletir o novo tipo.

- [ ] **Step 4: Rodar testes**

```
bun test tests/formatters.test.ts
bun run typecheck
```
Esperado: PASS, tipos OK.

- [ ] **Step 5: Commit**

```
git add src/formatters/terminal.ts tests/formatters.test.ts
git commit -m "feat(terminal): suportar displayMode used/remaining"
```

---

## Task 4: waybar.ts adapta para `mode`

**Files:**
- Modify: `src/formatters/waybar.ts`
- Test: `tests/formatters.test.ts`

- [ ] **Step 1: Test em modo `used`**

Adicionar a `tests/formatters.test.ts`:

```typescript
describe('formatForWaybar displayMode=used', () => {
  it('text shows used (100 - remaining)', () => {
    const result = formatForWaybar(mockAllQuotas([mockClaudeQuota(80)]), 'used');
    expect(result.text).toContain('20%');
  });

  it('class still uses health thresholds', () => {
    const result = formatForWaybar(mockAllQuotas([mockClaudeQuota(5)]), 'used');
    // health=5 -> critical
    expect(result.class).toContain('claude-critical');
  });

  it('formatProviderForWaybar respects mode', () => {
    const result = formatProviderForWaybar(mockClaudeQuota(80), 'used');
    expect(result.text).toContain('20%');
  });

  it('default arg keeps remaining behavior', () => {
    const result = formatForWaybar(mockAllQuotas([mockClaudeQuota(80)]));
    expect(result.text).toContain('80%');
  });
});
```

- [ ] **Step 2: Rodar, esperar FAIL**

```
bun test tests/formatters.test.ts -t "formatForWaybar displayMode"
```
Esperado: FAIL.

- [ ] **Step 3: Refatorar waybar.ts**

3a — imports:

```typescript
import { APP_BASE_CLASS } from '../app-identity';
import { getColorForPercent } from '../config';
import type { AllQuotas, ProviderQuota, QuotaWindow } from '../providers/types';
import { loadSettingsSync, type WindowPolicy, type DisplayMode } from '../settings';
import { BOX, ONE_DARK, PROVIDER_HEX } from '../theme';
import {
  applyCodexModelFilter,
  codexModelsFromQuota,
  etaLabel,
  formatEta,
  formatPercent,
  formatResetTime,
  normalizePlanLabel,
  toDisplay,
  toHealth,
} from './shared';
```

3b — wrapper de cor que respeita mode:

```typescript
function colorFor(display: number | null, mode: DisplayMode): string {
  return getColorForPercent(toHealth(display, mode));
}
```

3c — refatorar helpers locais:

```typescript
function pctColored(display: number | null, mode: DisplayMode): string {
  return s(colorFor(display, mode), formatPercent(display));
}

function bar(display: number | null, mode: DisplayMode): string {
  if (display === null) return s(ONE_DARK.comment, '░'.repeat(20));
  const filled = Math.floor(display / 5);
  return s(colorFor(display, mode), '█'.repeat(filled)) + s(ONE_DARK.comment, '░'.repeat(20 - filled));
}

function indicator(display: number | null, mode: DisplayMode): string {
  const health = toHealth(display, mode);
  if (health === null) return s(ONE_DARK.comment, BOX.dotO);
  if (health < 10) return s(ONE_DARK.red, BOX.dot);
  if (health < 30) return s(ONE_DARK.orange, BOX.dot);
  if (health < 60) return s(ONE_DARK.yellow, BOX.dot);
  return s(ONE_DARK.green, BOX.dot);
}
```

3d — `codexModelLine` recebe `mode`:

```typescript
function codexModelLine(name: string, window: QuotaWindow | undefined, maxLen: number, v: string, mode: DisplayMode): string {
  const rem = window?.remaining ?? null;
  const disp = toDisplay(rem, mode);
  const nameS = s(ONE_DARK.textBright, name.padEnd(maxLen));
  const b = bar(disp, mode);
  const pctS = s(colorFor(disp, mode), formatPercent(disp).padStart(4));
  const etaS = window?.resetsAt
    ? s(ONE_DARK.cyan, `→ ${formatEta(window.resetsAt, rem)} ${formatResetTime(window.resetsAt, rem)}`)
    : s(ONE_DARK.cyan, '→ N/A');
  return `${v}  ${indicator(disp, mode)} ${nameS} ${b} ${pctS} ${etaS}`;
}
```

3e — atualizar `buildClaudeTooltip`, `buildCodexTooltip`, `buildAmpTooltip`, `buildGenericTooltip` para receber `mode` e propagar em todos os call sites de `bar`/`indicator`/`colorFor`/`formatPercent`. Em todos os pontos onde hoje aparece `bar(window.remaining)` etc, computar `disp = toDisplay(window.remaining, mode)` primeiro.

Exemplo Amp Free Tier (linhas ~277–293) — trocar label:

```typescript
if (free.resetsAt && free.remaining !== 100) {
  etaParts.push(
    s(
      ONE_DARK.cyan,
      `→ ${etaLabel(mode)} ${formatEta(free.resetsAt, free.remaining)} ${formatResetTime(free.resetsAt, free.remaining)}`,
    ),
  );
}
```

3f — registry e funções de topo:

```typescript
type TooltipBuilder = (p: ProviderQuota, fetchedAt: string | undefined, mode: DisplayMode) => string;

const tooltipBuilders = new Map<string, TooltipBuilder>([
  ['claude', buildClaudeTooltip],
  ['codex', buildCodexTooltip],
  ['amp', buildAmpTooltip],
]);

export function registerTooltipBuilder(providerId: string, builder: TooltipBuilder): void {
  tooltipBuilders.set(providerId, builder);
}

function buildProviderTooltip(p: ProviderQuota, fetchedAt: string | undefined, mode: DisplayMode): string {
  const builder = tooltipBuilders.get(p.provider);
  if (builder) return builder(p, fetchedAt, mode);
  return buildGenericTooltip(p, fetchedAt, mode);
}

function buildTooltip(quotas: AllQuotas, mode: DisplayMode): string {
  const sections: string[] = [];
  const fetchedAt = quotas.fetchedAt;
  for (const p of quotas.providers) {
    if (!p.available && !p.error) continue;
    sections.push(buildProviderTooltip(p, fetchedAt, mode));
  }
  return sections.join('\n\n');
}

function buildText(quotas: AllQuotas, mode: DisplayMode): string {
  const parts: string[] = [];
  for (const p of quotas.providers) {
    if (!p.available) continue;
    const disp = toDisplay(p.primary?.remaining ?? null, mode);
    parts.push(pctColored(disp, mode));
  }
  if (parts.length === 0) return s(ONE_DARK.comment, 'No Providers');
  return parts.join(` ${s(ONE_DARK.comment, '│')} `);
}

function getClass(quotas: AllQuotas): string {
  // class continua baseado em health, não display — sem mudança
  // (mantém código existente)
  ...
}

export function formatForWaybar(quotas: AllQuotas, mode: DisplayMode = 'remaining'): WaybarOutput {
  return {
    text: buildText(quotas, mode),
    tooltip: buildTooltip(quotas, mode),
    class: getClass(quotas),
  };
}

export function outputWaybar(quotas: AllQuotas, mode: DisplayMode = 'remaining'): void {
  console.log(JSON.stringify(formatForWaybar(quotas, mode)));
}

export function formatProviderForWaybar(quota: ProviderQuota, mode: DisplayMode = 'remaining'): WaybarOutput {
  if (!quota.available || quota.error) {
    return {
      text: `<span foreground='${ONE_DARK.red}'>󱘖</span>`,
      tooltip: buildProviderTooltip(quota, undefined, mode),
      class: `${APP_BASE_CLASS}-${quota.provider} disconnected`,
    };
  }
  const disp = toDisplay(quota.primary?.remaining ?? null, mode);
  const health = quota.primary?.remaining ?? 100; // class continua usando health
  let status = 'ok';
  if (health < 10) status = 'critical';
  else if (health < 30) status = 'warn';
  else if (health < 60) status = 'low';
  return {
    text: pctColored(disp, mode),
    tooltip: buildProviderTooltip(quota, undefined, mode),
    class: `${APP_BASE_CLASS}-${quota.provider} ${status}`,
  };
}
```

- [ ] **Step 4: Rodar testes**

```
bun test tests/formatters.test.ts
bun run typecheck
```
Esperado: PASS.

- [ ] **Step 5: Commit**

```
git add src/formatters/waybar.ts tests/formatters.test.ts
git commit -m "feat(waybar): suportar displayMode used/remaining"
```

---

## Task 5: Callers passam `mode` real

**Files:**
- Modify: `src/index.ts`, `src/refresh.ts`, `src/action-right.ts`

- [ ] **Step 1: Atualizar `src/index.ts`**

Carregar settings já existe via fluxo normal. Onde `outputTerminal(quotas)` / `outputWaybar(quotas)` / `formatProviderForWaybar(...)` são chamados (linhas 179, 190, 192), passar `settings.waybar.displayMode`. Se `settings` não estiver no escopo, importar `loadSettingsSync` ou usar `loadSettings()` async conforme padrão do arquivo.

Exemplo:

```typescript
import { loadSettingsSync } from './settings';
// ...
const mode = loadSettingsSync().waybar.displayMode;
outputTerminal(quotas, mode);
// ...
console.log(JSON.stringify(formatProviderForWaybar(quotas.providers[0], mode)));
// ...
outputWaybar(quotas, mode);
```

- [ ] **Step 2: Atualizar `src/refresh.ts` (linhas 37, 44)**

```typescript
import { loadSettingsSync } from './settings';
// ...
const mode = loadSettingsSync().waybar.displayMode;
outputTerminal({ providers: [quota], fetchedAt: new Date().toISOString() }, mode);
// ...
outputTerminal(quotas, mode);
```

- [ ] **Step 3: Atualizar `src/action-right.ts` (linha 76)**

```typescript
import { loadSettingsSync } from './settings';
// ...
const mode = loadSettingsSync().waybar.displayMode;
outputTerminal({ providers: [...], fetchedAt: ... }, mode);
```

- [ ] **Step 4: Verificar**

```
bun run typecheck
bun test
```
Esperado: PASS.

- [ ] **Step 5: Commit**

```
git add src/index.ts src/refresh.ts src/action-right.ts
git commit -m "feat: callers propagam displayMode aos formatters"
```

---

## Task 6: TUI passo de display mode

**Files:**
- Modify: `src/tui/configure-layout.ts`

- [ ] **Step 1: Adicionar passo após separator (antes do `--- Apply ---`)**

Em `src/tui/configure-layout.ts`, depois de `const newSeparator = sepResult as typeof currentSep;`:

```typescript
// --- Step 4: Display mode ---
const currentMode = settings.waybar.displayMode;
const modeResult = await p.select({
  message: colorize('Display mode', semantic.title),
  options: [
    {
      value: 'remaining' as const,
      label: colorize('Remaining', currentMode === 'remaining' ? oneDark.green : oneDark.text),
      hint: colorize('100% = quota cheia, 0% = esgotado (default)', semantic.muted),
    },
    {
      value: 'used' as const,
      label: colorize('Used', currentMode === 'used' ? oneDark.green : oneDark.text),
      hint: colorize('0% = nada usado, 100% = esgotado', semantic.muted),
    },
  ],
  initialValue: currentMode,
});

if (p.isCancel(modeResult)) return false;
const newDisplayMode = modeResult as typeof currentMode;
```

E no bloco `--- Apply ---`:

```typescript
settings.waybar.providers = selectedProviders;
settings.waybar.providerOrder = newOrder;
settings.waybar.separators = newSeparator;
settings.waybar.displayMode = newDisplayMode;
await saveSettings(settings);
```

E no summary final:

```typescript
p.log.info(`${colorize('Mode:', semantic.subtitle)} ${colorize(newDisplayMode, oneDark.green)}`);
```

- [ ] **Step 2: Typecheck**

```
bun run typecheck
```
Esperado: PASS.

- [ ] **Step 3: Smoke manual (opcional, não bloqueante)**

`bun run start` → menu → Configure Layout → verificar passo Display mode aparece, escolher Used, ver settings.json gravado. (Não rodar em live se usuário não autorizar; usar `XDG_CONFIG_HOME=/tmp/abo-smoke bun run start`.)

- [ ] **Step 4: Commit**

```
git add src/tui/configure-layout.ts
git commit -m "feat(tui): adicionar passo Display mode em configure-layout"
```

---

## Task 7: Snapshots `*-used.snap`

**Files:**
- Modify: `tests/formatters-snapshot.test.ts`

- [ ] **Step 1: Adicionar bloco de snapshots para `mode='used'`**

Replicar a estrutura existente, mas chamando `formatForTerminal(q, 'used')` e `formatForWaybar(q, 'used')`. Cobrir cenários:

- Claude cheio (remaining=100 → display 0).
- Claude parcial (remaining=45 → display 55).
- Claude esgotado (remaining=0 → display 100).
- Codex parcial (remaining=30).
- Amp Free Tier parcial (remaining=75 → display 25), incluindo verificação que tooltip contém `"Resets in"`.

Usar `toMatchSnapshot()` com chave distinta (sufixo `-used`) para não colidir com snapshots atuais.

Exemplo:

```typescript
describe('snapshot displayMode=used', () => {
  it('claude full → 0% used', () => {
    const q = mockAllQuotas([{ ...mockClaudeQuota(100) }]);
    expect(formatForTerminal(q, 'used')).toMatchSnapshot('claude-full-used.terminal');
    expect(formatForWaybar(q, 'used').tooltip).toMatchSnapshot('claude-full-used.tooltip');
  });

  it('amp shows "Resets in" label when mode=used', () => {
    const q = mockAllQuotas([mockAmpQuota()]);
    const out = formatForWaybar(q, 'used');
    expect(out.tooltip).toContain('Resets in');
    expect(out.tooltip).not.toContain('Full in');
  });
});
```

- [ ] **Step 2: Rodar snapshot, aceitar novos**

```
bun test tests/formatters-snapshot.test.ts -u
```
Inspecionar visual o `tests/__snapshots__/formatters-snapshot.test.ts.snap` — confirmar que percentuais foram invertidos e labels mostram "Resets in" onde aplicável.

- [ ] **Step 3: Rodar sem `-u`**

```
bun test tests/formatters-snapshot.test.ts
```
Esperado: PASS.

- [ ] **Step 4: Commit**

```
git add tests/formatters-snapshot.test.ts tests/__snapshots__/formatters-snapshot.test.ts.snap
git commit -m "test: snapshots p/ displayMode=used"
```

---

## Task 8: Verificação final + handoff

- [ ] **Step 1: Full verification**

```
bun test
bun run typecheck
bun run lint
```
Esperado: tudo PASS, sem erros de lint/tipos.

- [ ] **Step 2: Diff review**

```
git log --oneline origin/master..HEAD
git diff origin/master..HEAD --stat
```
Conferir que mudanças cobrem: settings (1), shared (2), terminal (3), waybar (4), callers (5), TUI (6), snapshots (7).

- [ ] **Step 3: Update CHANGELOG (se padrão do repo exigir)**

Inspecionar `CHANGELOG.md` topo; adicionar entry sob a próxima versão:

```
- Adicionado: setting `waybar.displayMode` (`remaining` | `used`) com toggle via TUI Configure Layout.
```

Commit:

```
git add CHANGELOG.md
git commit -m "docs: changelog p/ displayMode toggle"
```

- [ ] **Step 4: Handoff**

Reportar conclusão ao user com summary de commits e arquivos tocados. Pedir aprovação antes de `git push`.

---

## Self-Review

**Spec coverage:**
- Setting `waybar.displayMode` default `'remaining'` → Task 1. ✓
- `toDisplay` / `toHealth` helpers → Task 2. ✓
- Cor via thresholds de saúde (sem mudar config.ts) → Tasks 3 e 4 (via `toHealth` + `getColor`/`getColorForPercent`). ✓
- Barra enche conforme usa → Tasks 3 e 4 (`bar(display)` usa display direto, então em `used` enche). ✓
- "Resets in" vs "Full in" → Task 2 (`etaLabel`) + Task 3 (Amp Free Tier terminal) + Task 4 (Amp Free Tier waybar). ✓
- Toggle aplica em terminal + waybar → Tasks 3, 4, 5. ✓
- TUI menu → Task 6. ✓
- Testes (settings, formatters, snapshots) → Tasks 1, 2, 3, 4, 7. ✓
- Verificação `bun test && bun run typecheck && bun run lint` → Task 8. ✓

**Placeholder scan:** Sem TBD/TODO. Todos os steps de código têm bloco completo.

**Type consistency:** `DisplayMode` re-exportado de `shared.ts` (Task 2) usado consistentemente em terminal/waybar (Tasks 3/4) e callers (Task 5). `toDisplay` / `toHealth` / `etaLabel` definidos uma vez. Default `'remaining'` consistente em todas as assinaturas para preservar callers/testes existentes.

**Não-objetivos respeitados:** Sem renomear `remaining` no domínio. Sem mudar `config.ts`. Sem flag CLI. Class CSS continua baseada em health (decisão implícita pra não quebrar styling existente — class `claude-critical` mantém semântica de "quota baixa" independente do display).
