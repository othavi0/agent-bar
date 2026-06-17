# Output Architecture (`--format json` + `--watch`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expor um contrato JSON estável e versionado (`--format json`) e um modo stream NDJSON (`--watch`) para que bars não-Waybar (Quickshell, Eww, Ironbar) consumam o agent-bar, mantendo o Waybar como default.

**Architecture:** Um mapper puro (`src/formatters/json.ts`) converte o modelo interno `ProviderQuota`/`AllQuotas` (pré-render, sem Pango) num envelope versionado. A CLI ganha flags ortogonais ao `command`. O `index.ts` roteia one-shot json; um módulo `src/watch.ts` faz o loop NDJSON. Providers, cache, settings e render-pango não mudam.

**Tech Stack:** Bun, TypeScript strict, ESM, `bun:test`, Biome.

## Global Constraints

- **Bun only.** Sem Node/npm/pnpm/yarn/ts-node em runtime ou testes.
- **stdout limpo:** só JSON/NDJSON no stdout; todo log vai pra stderr (o `logger` já faz).
- **Nunca `!` (non-null assertion).** Estreitar com guard explícito.
- **Provider error strings são contrato** — não alterar as existentes.
- **XML/Pango escape só em `render-pango.ts`** — o json não toca render-pango.
- TypeScript strict, ESM, Biome (2 espaços, single quote, 120 cols, unused import = erro).
- Identificadores/arquivos em inglês; commits em PT, Conventional Commits, subject ≤ 50 chars.
- Verificação por task: rodar os testes da task; antes do handoff `bun test && bun run typecheck && bun run lint`.

## File Structure

- `src/formatters/json.ts` (novo) — `SCHEMA_VERSION`, tipos `JsonWindow`/`JsonProvider`/`JsonOutput`, `toJsonWindow`/`toProviderOutput`/`toJsonOutput`. Puro, sem I/O.
- `src/cli.ts` (modificar) — `CliOptions` += `format`/`watch`/`intervalSeconds`; cases no parser; validação cross-flag pós-parse; entradas no `--help`.
- `src/index.ts` (modificar) — branch watch (chama `startWatch`), branch one-shot json, desvio dos 2 guards do caminho Waybar.
- `src/watch.ts` (novo) — `buildWatchLine` (puro) e `startWatch` (loop, EPIPE, scheduling).
- `tests/formatters-json.test.ts` (novo).
- `tests/cli.test.ts` (modificar).
- `tests/watch.test.ts` (novo) — `buildWatchLine` + handler EPIPE.
- `docs/json-output.md` (novo), `README.md`/`docs/README.md` (links), `package.json` `files`, `tests/package.test.ts`.

---

### Task 1: JSON formatter (mapper puro)

**Files:**
- Create: `src/formatters/json.ts`
- Test: `tests/formatters-json.test.ts`

**Interfaces:**
- Consumes: `AllQuotas`, `ProviderQuota`, `QuotaWindow` de `src/providers/types.ts`.
- Produces: `SCHEMA_VERSION: number`, `interface JsonWindow`, `interface JsonProvider`, `interface JsonOutput`, `toJsonWindow(w: QuotaWindow): JsonWindow`, `toProviderOutput(p: ProviderQuota): JsonProvider`, `toJsonOutput(quotas: AllQuotas): JsonOutput`.

- [ ] **Step 1: Write the failing test**

Create `tests/formatters-json.test.ts`:

```ts
import { describe, expect, it } from 'bun:test';
import type { AllQuotas, ProviderQuota } from '../src/providers/types';
import { SCHEMA_VERSION, toJsonOutput, toProviderOutput } from '../src/formatters/json';

const claude: ProviderQuota = {
  provider: 'claude',
  displayName: 'Claude',
  available: true,
  plan: 'Max',
  primary: { remaining: 30, used: 70, resetsAt: '2026-06-17T20:00:00Z', windowMinutes: 300 },
  secondary: { remaining: 65, resetsAt: '2026-06-19T22:00:00Z' },
  models: { Sonnet: { remaining: 89, resetsAt: '2026-06-19T22:00:00Z' } },
  extra: { weeklyModels: { Sonnet: { remaining: 89, resetsAt: '2026-06-19T22:00:00Z' } } },
};

const ampError: ProviderQuota = {
  provider: 'amp',
  displayName: 'Amp',
  available: false,
  error: 'Not logged in.',
};

const allQuotas: AllQuotas = { providers: [claude, ampError], fetchedAt: '2026-06-17T19:00:00.000Z' };

describe('json formatter', () => {
  it('wraps providers in a versioned envelope', () => {
    const out = toJsonOutput(allQuotas);
    expect(out.schemaVersion).toBe(SCHEMA_VERSION);
    expect(out.fetchedAt).toBe('2026-06-17T19:00:00.000Z');
    expect(out.providers).toHaveLength(2);
  });

  it('maps primary/secondary/models/used and keeps extra', () => {
    const p = toProviderOutput(claude);
    expect(p.primary).toEqual({ remaining: 30, used: 70, resetsAt: '2026-06-17T20:00:00Z', windowMinutes: 300 });
    expect(p.secondary).toEqual({ remaining: 65, resetsAt: '2026-06-19T22:00:00Z' });
    expect(p.models?.Sonnet.remaining).toBe(89);
    expect(p.extra?.weeklyModels).toBeDefined();
  });

  it('omits absent optional fields (no null, no undefined key) on an available provider', () => {
    const p = toProviderOutput(claude);
    expect('error' in p).toBe(false);
    expect('account' in p).toBe(false);
  });

  it('includes error and omits primary on a failed provider', () => {
    const p = toProviderOutput(ampError);
    expect(p.available).toBe(false);
    expect(p.error).toBe('Not logged in.');
    expect('primary' in p).toBe(false);
  });

  it('never contains Pango markup', () => {
    expect(JSON.stringify(toJsonOutput(allQuotas))).not.toContain('<span');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bun test tests/formatters-json.test.ts`
Expected: FAIL — `Cannot find module '../src/formatters/json'`.

- [ ] **Step 3: Write minimal implementation**

Create `src/formatters/json.ts`:

```ts
import type { AllQuotas, ProviderQuota, QuotaWindow } from '../providers/types';

/** Bump on incompatible schema change (remove/rename/retype a stable field). Adding optional fields does not require a bump. */
export const SCHEMA_VERSION = 1;

export interface JsonWindow {
  remaining: number;
  used?: number | null;
  resetsAt: string | null;
  windowMinutes?: number | null;
}

export interface JsonProvider {
  provider: string;
  displayName: string;
  available: boolean;
  account?: string;
  plan?: string;
  planType?: string;
  primary?: JsonWindow;
  secondary?: JsonWindow;
  models?: Record<string, JsonWindow>;
  /** Provider-specific, pre-render data (weeklyModels, modelsDetailed, extraUsage, meta, quotaSnapshots). NOT covered by schemaVersion stability. */
  extra?: Record<string, unknown>;
  error?: string;
}

export interface JsonOutput {
  schemaVersion: number;
  fetchedAt: string;
  providers: JsonProvider[];
}

export function toJsonWindow(w: QuotaWindow): JsonWindow {
  const out: JsonWindow = { remaining: w.remaining, resetsAt: w.resetsAt };
  if (w.used !== undefined) out.used = w.used;
  if (w.windowMinutes !== undefined) out.windowMinutes = w.windowMinutes;
  return out;
}

export function toProviderOutput(p: ProviderQuota): JsonProvider {
  const out: JsonProvider = {
    provider: p.provider,
    displayName: p.displayName,
    available: p.available,
  };
  if (p.account !== undefined) out.account = p.account;
  if (p.plan !== undefined) out.plan = p.plan;
  if (p.planType !== undefined) out.planType = p.planType;
  if (p.primary) out.primary = toJsonWindow(p.primary);
  if (p.secondary) out.secondary = toJsonWindow(p.secondary);
  if (p.models) {
    const models: Record<string, JsonWindow> = {};
    for (const [name, w] of Object.entries(p.models)) {
      models[name] = toJsonWindow(w);
    }
    out.models = models;
  }
  if (p.extra && Object.keys(p.extra).length > 0) {
    out.extra = p.extra as Record<string, unknown>;
  }
  if (p.error !== undefined) out.error = p.error;
  return out;
}

export function toJsonOutput(quotas: AllQuotas): JsonOutput {
  return {
    schemaVersion: SCHEMA_VERSION,
    fetchedAt: quotas.fetchedAt,
    providers: quotas.providers.map(toProviderOutput),
  };
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `bun test tests/formatters-json.test.ts`
Expected: PASS (5 tests).

- [ ] **Step 5: Typecheck and commit**

Run: `bun run typecheck`
Expected: no output (sucesso).

```bash
git add src/formatters/json.ts tests/formatters-json.test.ts
git commit -m "feat: mapper json versionado (contrato de output)"
```

---

### Task 2: CLI flags (`--format`, `--watch`, `--interval`)

**Files:**
- Modify: `src/cli.ts`
- Test: `tests/cli.test.ts`

**Interfaces:**
- Produces: `CliOptions.format: 'waybar' | 'json'`, `CliOptions.watch: boolean`, `CliOptions.intervalSeconds: number`. `parseArgs` aplica: `--watch` implica `format='json'`; `--watch` + `--format waybar` explícito → exit 1; `--interval` sem `--watch` → warning stderr; `--format`/`--interval` inválidos → exit 1.

- [ ] **Step 1: Write the failing test**

Add to `tests/cli.test.ts` inside `describe('parseArgs')`, after the `--version` test in the `commands` block (or a new `describe`):

```ts
describe('output format flags', () => {
  it('defaults format to waybar, watch false, interval 60', () => {
    const o = parseArgs([]);
    expect(o.format).toBe('waybar');
    expect(o.watch).toBe(false);
    expect(o.intervalSeconds).toBe(60);
  });

  it('parses --format json', () => {
    expect(parseArgs(['--format', 'json']).format).toBe('json');
  });

  it('--watch implies json', () => {
    const o = parseArgs(['--watch']);
    expect(o.watch).toBe(true);
    expect(o.format).toBe('json');
  });

  it('parses --interval', () => {
    expect(parseArgs(['--watch', '--interval', '30']).intervalSeconds).toBe(30);
  });

  function expectExit1(args: string[], needle: string) {
    const origExit = process.exit;
    const origErr = console.error;
    const codes: number[] = [];
    const errs: string[] = [];
    process.exit = ((c?: number) => {
      codes.push(c ?? 0);
      throw new Error('__exit__');
    }) as typeof process.exit;
    console.error = (...a: unknown[]) => {
      errs.push(a.join(' '));
    };
    try {
      expect(() => parseArgs(args)).toThrow('__exit__');
      expect(codes).toEqual([1]);
      expect(errs.join('\n')).toContain(needle);
    } finally {
      process.exit = origExit;
      console.error = origErr;
    }
  }

  it('exits 1 on invalid --format', () => {
    expectExit1(['--format', 'xml'], "--format must be");
  });

  it('exits 1 on invalid --interval', () => {
    expectExit1(['--watch', '--interval', 'abc'], '--interval must be');
  });

  it('exits 1 on --watch with explicit --format waybar', () => {
    expectExit1(['--watch', '--format', 'waybar'], '--watch requires --format json');
  });

  it('warns (no exit) on --interval without --watch', () => {
    const origErr = console.error;
    const errs: string[] = [];
    console.error = (...a: unknown[]) => {
      errs.push(a.join(' '));
    };
    try {
      const o = parseArgs(['--interval', '30']);
      expect(o.watch).toBe(false);
      expect(errs.join('\n')).toContain('--interval has no effect without --watch');
    } finally {
      console.error = origErr;
    }
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bun test tests/cli.test.ts`
Expected: FAIL — `o.format` undefined / cases não existem.

- [ ] **Step 3: Extend `CliOptions` and defaults**

In `src/cli.ts`, add to the `CliOptions` interface (after `verbose: boolean;`):

```ts
  format: 'waybar' | 'json';
  watch: boolean;
  intervalSeconds: number;
```

In `parseArgs`, change the initial `options` object to:

```ts
  const options: CliOptions = {
    command: 'waybar',
    refresh: false,
    verbose: false,
    format: 'waybar',
    watch: false,
    intervalSeconds: 60,
  };
```

- [ ] **Step 4: Add parser cases and post-parse validation**

In `src/cli.ts`, declare two trackers at the top of `parseArgs` (right after the `options` object):

```ts
  let formatGiven = false;
  let intervalGiven = false;
```

Add these cases to the `switch (arg)` (place near `--verbose`):

```ts
      case '--format': {
        const val = requireNextArg(args, i, '--format');
        if (val !== 'waybar' && val !== 'json') {
          console.error(`Error: --format must be 'waybar' or 'json' (got '${val}')`);
          process.exit(1);
        }
        options.format = val;
        formatGiven = true;
        i++;
        break;
      }
      case '--watch':
        options.watch = true;
        break;
      case '--interval': {
        const val = requireNextArg(args, i, '--interval');
        const n = Number.parseInt(val, 10);
        if (!Number.isInteger(n) || n <= 0) {
          console.error(`Error: --interval must be a positive integer (got '${val}')`);
          process.exit(1);
        }
        options.intervalSeconds = n;
        intervalGiven = true;
        i++;
        break;
      }
```

Add the post-parse validation block immediately before `return options;`:

```ts
  if (options.watch) {
    if (formatGiven && options.format === 'waybar') {
      console.error('Error: --watch requires --format json');
      process.exit(1);
    }
    options.format = 'json';
  }
  if (intervalGiven && !options.watch) {
    console.error('[agent-bar] --interval has no effect without --watch');
  }
```

- [ ] **Step 5: Run test to verify it passes**

Run: `bun test tests/cli.test.ts`
Expected: PASS (todos, incl. os novos).

- [ ] **Step 6: Add `--help` entries**

In `showHelp()`, inside the `Flags` section (after the `--version` line added previously), insert:

```ts
  console.log(optLine('--format <fmt>', 'Output format: waybar (default) | json'));
  console.log(optLine('--watch', 'Stream NDJSON (implies --format json)'));
  console.log(optLine('--interval <s>', 'Watch poll floor in seconds (default 60)'));
```

- [ ] **Step 7: Typecheck and commit**

Run: `bun run typecheck && bun test tests/cli.test.ts`
Expected: typecheck limpo; testes PASS.

```bash
git add src/cli.ts tests/cli.test.ts
git commit -m "feat: flags --format/--watch/--interval na CLI"
```

---

### Task 3: One-shot `--format json` wiring (`index.ts`)

**Files:**
- Modify: `src/index.ts`

**Interfaces:**
- Consumes: `toJsonOutput` (Task 1), `options.format` (Task 2), `getAllQuotas`/`getQuotaFor` (existentes).
- Produces: comportamento — `agent-bar --format json [--provider X]` emite o envelope JSON e sai 0; ignora a lista `settings.waybar.providers`.

- [ ] **Step 1: Bypass the provider-disabled guard in json mode**

In `src/index.ts`, find the single-provider guard (atual ~linha 152-158):

```ts
    if (!settings.waybar.providers.includes(options.provider)) {
      console.log(JSON.stringify({ text: '', tooltip: '', class: APP_HIDDEN_CLASS }));
      process.exit(0);
    }
```

Replace the condition so json never emits the hidden Waybar envelope:

```ts
    if (options.format !== 'json' && !settings.waybar.providers.includes(options.provider)) {
      console.log(JSON.stringify({ text: '', tooltip: '', class: APP_HIDDEN_CLASS }));
      process.exit(0);
    }
```

- [ ] **Step 2: Bypass the Waybar settings filter in json mode**

Find (atual ~linha 172):

```ts
    if (options.command === 'waybar') {
      quotas.providers = quotas.providers.filter((p) => settings.waybar.providers.includes(p.provider));
    }
```

Replace the condition:

```ts
    if (options.command === 'waybar' && options.format !== 'json') {
      quotas.providers = quotas.providers.filter((p) => settings.waybar.providers.includes(p.provider));
    }
```

- [ ] **Step 3: Add the json one-shot branch before the output switch**

Immediately before `const mode = settings.waybar.displayMode;` (ou antes do `switch (options.command)`), insert:

```ts
  if (options.format === 'json') {
    const { toJsonOutput } = await import('./formatters/json');
    console.log(JSON.stringify(toJsonOutput(quotas)));
    process.exit(0);
  }
```

- [ ] **Step 4: Verify manually (one-shot)**

Run: `bun run src/index.ts --format json --provider claude | bun -e 'const j=JSON.parse(await Bun.stdin.text()); console.log("schemaVersion=", j.schemaVersion, "providers=", j.providers.length)'`
Expected: imprime `schemaVersion= 1 providers= 1` (claude presente mesmo se desabilitado no Waybar).

Run: `bun run src/index.ts --format json | bun -e 'const j=JSON.parse(await Bun.stdin.text()); console.log(j.providers.map(p=>p.provider).join(","))'`
Expected: lista de TODOS os providers registrados (`claude,codex,copilot,amp`), independente do settings.

Run: `bun run src/index.ts --format json | grep -c "<span"`
Expected: `0` (sem Pango).

- [ ] **Step 5: Regression + commit**

Run: `bun test tests/waybar-contract.test.ts tests/cli.test.ts && bun run typecheck`
Expected: PASS; typecheck limpo. (Confirma que o caminho Waybar default não regrediu.)

```bash
git add src/index.ts
git commit -m "feat: roteia --format json (one-shot)"
```

---

### Task 4: `--watch` stream (`src/watch.ts` + wiring)

**Files:**
- Create: `src/watch.ts`
- Modify: `src/index.ts`
- Test: `tests/watch.test.ts`

**Interfaces:**
- Consumes: `getAllQuotas`/`getQuotaFor` (`src/providers`), `toJsonOutput` (Task 1).
- Produces: `buildWatchLine(quotas: AllQuotas): string` (linha NDJSON com `\n`), `startWatch(opts: { provider?: string; intervalMs: number }): Promise<void>` (nunca resolve; sai via signal/EPIPE).

- [ ] **Step 1: Write the failing test**

Create `tests/watch.test.ts`:

```ts
import { describe, expect, it } from 'bun:test';
import type { AllQuotas } from '../src/providers/types';
import { buildWatchLine } from '../src/watch';

const quotas: AllQuotas = {
  providers: [{ provider: 'claude', displayName: 'Claude', available: true, primary: { remaining: 50, resetsAt: null } }],
  fetchedAt: '2026-06-17T19:00:00.000Z',
};

describe('buildWatchLine', () => {
  it('produces one valid NDJSON line ending in newline', () => {
    const line = buildWatchLine(quotas);
    expect(line.endsWith('\n')).toBe(true);
    expect(line.includes('\n')).toBe(true);
    expect(line.trim().split('\n')).toHaveLength(1);
    const parsed = JSON.parse(line);
    expect(parsed.schemaVersion).toBe(1);
    expect(parsed.providers[0].provider).toBe('claude');
  });

  it('contains no Pango markup', () => {
    expect(buildWatchLine(quotas)).not.toContain('<span');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bun test tests/watch.test.ts`
Expected: FAIL — `Cannot find module '../src/watch'`.

- [ ] **Step 3: Write `src/watch.ts`**

Create `src/watch.ts`:

```ts
import { toJsonOutput } from './formatters/json';
import { logger } from './logger';
import { getAllQuotas, getQuotaFor } from './providers';
import type { AllQuotas } from './providers/types';

export interface StartWatchOptions {
  provider?: string;
  intervalMs: number;
}

async function fetchQuotas(provider?: string): Promise<AllQuotas> {
  if (provider) {
    const quota = await getQuotaFor(provider);
    return {
      providers: quota ? [quota] : [],
      fetchedAt: new Date().toISOString(),
    };
  }
  return getAllQuotas();
}

/** Serialize one quota snapshot as a single NDJSON line. */
export function buildWatchLine(quotas: AllQuotas): string {
  return `${JSON.stringify(toJsonOutput(quotas))}\n`;
}

/**
 * Long-running NDJSON emitter. Emits immediately, then every `intervalMs` after
 * the previous tick's write completes (backpressure-aware, no overlap/drift).
 * Exits 0 on EPIPE (consumer closed the pipe) or SIGTERM/SIGINT.
 * Never resolves on its own.
 */
export async function startWatch(opts: StartWatchOptions): Promise<void> {
  process.stdout.on('error', (err: Error & { code?: string }) => {
    if (err.code === 'EPIPE') process.exit(0);
  });

  if (process.stdout.isTTY) {
    process.stderr.write('[agent-bar] watch mode: output is NDJSON — pipe to a consumer\n');
  }

  let stopping = false;
  const stop = () => {
    stopping = true;
    process.exit(0);
  };
  process.on('SIGTERM', stop);
  process.on('SIGINT', stop);

  const tick = async (): Promise<void> => {
    if (stopping) return;
    try {
      const quotas = await fetchQuotas(opts.provider);
      process.stdout.write(buildWatchLine(quotas), () => {
        if (!stopping) setTimeout(tick, opts.intervalMs);
      });
    } catch (error) {
      logger.error('watch tick failed', { error });
      if (!stopping) setTimeout(tick, opts.intervalMs);
    }
  };

  await tick();
  // Keep the process alive; it exits only via signal or EPIPE.
  await new Promise<void>(() => {});
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `bun test tests/watch.test.ts`
Expected: PASS (2 tests).

- [ ] **Step 5: Wire `--watch` into `index.ts`**

In `src/index.ts`, find the existing refresh block:

```ts
  if (options.refresh) {
    const toInvalidate = options.provider ? [options.provider] : getRegisteredProviderIds();

    for (const id of toInvalidate) {
      const prov = getProvider(id);
      if (prov) await cache.invalidate(prov.cacheKey);
    }
    logger.info('Cache invalidated');
  }
```

Immediately AFTER that block (so `--refresh` invalidates once before the loop), insert:

```ts
  if (options.watch) {
    const { startWatch } = await import('./watch');
    await startWatch({ provider: options.provider, intervalMs: options.intervalSeconds * 1000 });
    return;
  }
```

- [ ] **Step 6: Verify manually (stream, EPIPE, interval)**

Run: `timeout 5 bun run src/index.ts --watch --interval 2 | head -2`
Expected: 2 linhas, cada uma um JSON válido (`schemaVersion:1`); o processo termina sem stack trace (EPIPE tratado quando `head` fecha o pipe).

Run: `bun run src/index.ts --watch --interval 2 2>&1 1>/dev/null | head -1` (TTY/dica)
Expected (se rodado num terminal): a dica `[agent-bar] watch mode: output is NDJSON` em stderr.

- [ ] **Step 7: Regression + commit**

Run: `bun test && bun run typecheck`
Expected: tudo PASS; typecheck limpo.

```bash
git add src/watch.ts src/index.ts tests/watch.test.ts
git commit -m "feat: modo --watch (stream NDJSON)"
```

---

### Task 5: Docs + packaging

**Files:**
- Create: `docs/json-output.md`
- Modify: `README.md`, `docs/README.md`, `package.json`, `tests/package.test.ts`

**Interfaces:**
- Consumes: o schema/flags das tasks 1-4.
- Produces: doc do contrato publicada no npm; `package.test.ts` reflete o novo doc no `files`.

- [ ] **Step 1: Write the failing test (package contract)**

In `tests/package.test.ts`, add `'docs/json-output.md'` to the expected `pkg.files` array — insira logo após `'docs/new-provider.md',`:

```ts
      'docs/new-provider.md',
      'docs/json-output.md',
      'docs/assets/agent-bar-banner.png',
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bun test tests/package.test.ts`
Expected: FAIL — `pkg.files` não contém `docs/json-output.md`.

- [ ] **Step 3: Add the doc to `package.json` `files`**

In `package.json`, add `"docs/json-output.md"` to `files`, logo após `"docs/new-provider.md",`:

```json
    "docs/new-provider.md",
    "docs/json-output.md",
    "docs/assets/agent-bar-banner.png"
```

- [ ] **Step 4: Run test to verify it passes**

Run: `bun test tests/package.test.ts`
Expected: PASS.

- [ ] **Step 5: Write `docs/json-output.md`**

Create `docs/json-output.md`:

````markdown
# JSON output (`--format json` + `--watch`)

For non-Waybar bars (Quickshell, Eww, Ironbar) that render natively and want raw
structured data instead of the Waybar Pango envelope.

## Modes

```bash
agent-bar --format json                 # one-shot snapshot, all registered providers
agent-bar --format json --provider claude  # one-shot, single provider
agent-bar --watch                       # stream: one JSON object per line (NDJSON), default 60s
agent-bar --watch --interval 30         # custom poll floor
```

- `--watch` implies `--format json`.
- `--interval` is a **floor**, not a strict period: each provider has a ~10s
  fetch timeout, so a slow tick can take longer than the interval. Use ≥ 30s.
- json/watch emit **all registered providers**, independent of the Waybar
  enabled-providers setting. `--provider X` emits a single-provider envelope.
- stdout is pure JSON/NDJSON; logs go to stderr.

## Envelope

```json
{
  "schemaVersion": 1,
  "fetchedAt": "2026-06-17T19:00:00.000Z",
  "providers": [
    {
      "provider": "claude",
      "displayName": "Claude",
      "available": true,
      "plan": "Max",
      "primary":   { "remaining": 30, "used": 70, "resetsAt": "2026-06-17T20:09:59Z", "windowMinutes": 300 },
      "secondary": { "remaining": 65, "resetsAt": "2026-06-19T22:59:59Z" },
      "models":    { "Sonnet": { "remaining": 89, "resetsAt": "2026-06-19T22:59:59Z" } },
      "extra":     { "weeklyModels": { "Sonnet": { "remaining": 89, "resetsAt": "2026-06-19T22:59:59Z" } } }
    },
    { "provider": "amp", "displayName": "Amp", "available": false, "error": "Not logged in." }
  ]
}
```

## Fields

| Field | Type | Notes |
| --- | --- | --- |
| `schemaVersion` | number | Contract version. See stability below. |
| `fetchedAt` | string (ISO) | When agent-bar produced the snapshot — **not** the network fetch time. On a cache hit the underlying data can be up to the cache TTL (~5min) older. |
| `providers[]` | array | One entry per provider. |
| `provider` | string | `claude` / `codex` / `copilot` / `amp`. |
| `displayName` | string | Human label. |
| `available` | boolean | Authenticated and fetched OK. |
| `account` / `plan` / `planType` | string | Optional; omitted when absent. |
| `primary` / `secondary` | `Window` | Optional quota windows. |
| `models` | map of `Window` | Optional per-model/bucket windows. |
| `extra` | object | **Unstable** — see below. |
| `error` | string | Present only on failure (key omitted when OK — check `'error' in p`). |

`Window`: `{ remaining: number, used?: number|null, resetsAt: string|null, windowMinutes?: number|null }`.
`remaining`/`used` are percentages (0-100; Copilot `used` can exceed 100 with overage).

## Stability

The top-level fields and `primary`/`secondary`/`models` (`Window`) are the
**stable contract** covered by `schemaVersion`. `extra` mirrors internal
provider-specific structures and is **best-effort/unstable** — it may change
without a `schemaVersion` bump. Don't depend on `extra` shapes long-term.

Bump policy: `schemaVersion` increments when a stable field is removed, renamed,
or changes type/meaning. Adding a new optional field does **not** bump.

Absence convention: optional fields are **omitted** when absent (never `null`,
never a serialized `undefined`).

## Quickshell example

One-shot (poll with a Timer):

```qml
import Quickshell.Io

Process {
  id: proc
  command: ["agent-bar", "--format", "json", "--provider", "claude"]
  running: true
  stdout: StdioCollector {
    onStreamFinished: {
      const data = JSON.parse(this.text);
      const p = data.providers[0];
      label.text = p.error ?? (p.primary.remaining + "%");
    }
  }
}
```

Stream (one long-lived process, NDJSON via SplitParser):

```qml
import Quickshell.Io

Process {
  command: ["agent-bar", "--watch", "--interval", "60"]
  running: true
  stdout: SplitParser {           // splits on "\n" by default
    onRead: (line) => {
      const data = JSON.parse(line);
      label.text = data.providers[0].primary.remaining + "%";
    }
  }
}
```

`Process.command` is an argv array (no shell) — keep each argument separate.

## Future

A native Omarchy Quickshell bar-widget plugin
(`~/.config/omarchy/plugins/agent-bar/`) is a future step once Omarchy ships its
Quickshell release (v4).
````

- [ ] **Step 6: Add doc links**

In `README.md`, under the `## Docs` list, add after the new-provider line:

```markdown
- [JSON output (Quickshell/Eww)](docs/json-output.md)
```

In `docs/README.md`, add an equivalent bullet linking `json-output.md` (match the existing list style in that file).

- [ ] **Step 7: Verify and commit**

Run: `bun test tests/package.test.ts && git diff --check`
Expected: PASS; `git diff --check` sem erros de whitespace.

```bash
git add docs/json-output.md README.md docs/README.md package.json tests/package.test.ts
git commit -m "docs: contrato json-output + links"
```

---

## Final verification (antes do handoff)

- [ ] Run: `bun test && bun run typecheck && bun run lint`
  Expected: todos os testes PASS; typecheck limpo; lint exit 0.
- [ ] Run: `bun run src/index.ts --format json | bun -e 'JSON.parse(await Bun.stdin.text()); console.log("ok")'` → `ok`.
- [ ] Run: `timeout 5 bun run src/index.ts --watch --interval 2 | head -2` → 2 linhas JSON, sem stack trace.
- [ ] Run: `bun run src/index.ts` (sem flags, num pipe) → ainda emite o envelope Waybar `{text,tooltip,class}` (não-regressão).

## Self-review (preenchido)

- **Spec coverage:** CLI (Task 2), schema/mapper (Task 1), one-shot wiring + guard bypasses (Task 3), watch/EPIPE/backpressure/TTY (Task 4), docs/estabilidade/QML/packaging (Task 5). `--refresh` once = via bloco existente antes do branch watch (Task 4 Step 5). `--verbose` em watch = stderr pelo logger (sem mudança). Coberto.
- **Placeholders:** nenhum — todo passo tem código/comando real.
- **Type consistency:** `JsonOutput`/`JsonProvider`/`JsonWindow`/`SCHEMA_VERSION`/`toJsonOutput` (Task 1) usados em Tasks 3-4; `buildWatchLine`/`startWatch`/`StartWatchOptions` (Task 4); `CliOptions.format/watch/intervalSeconds` (Task 2) usados em Task 3-4. Nomes batem.
