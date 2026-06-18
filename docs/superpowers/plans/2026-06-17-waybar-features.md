# Waybar Features Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Emit `percentage`/`alt` on the single-provider Waybar output (unlock `format-icons`) and support an opt-in `signal` on the generated module, fully additively.

**Architecture:** `formatProviderForWaybar` gains two optional output fields. A new optional `waybar.signal` setting threads through `exportWaybarModules`/`moduleDefinition` into both the export command and setup. Aggregate output and the `--format json` envelope are untouched.

**Tech Stack:** Bun + TypeScript, `bun:test`, Biome.

## Global Constraints

- **Bun only.** No Node/npm in runtime or tests.
- **stdout limpo** — Waybar parses stdout as JSON; logs go to stderr.
- **Provider error strings are contract** — assert verbatim.
- **Never `!` non-null assertion** — narrow with explicit guards.
- **Identity constants** from `src/app-identity.ts` (`APP_BASE_CLASS`), never hardcoded strings.
- **`alt` is a fixed internal enum** (`ok`/`low`/`warn`/`critical`/`disconnected`), never free provider text — no escape needed.
- **Settings schema stays v2** — `waybar.signal` is optional, normalized on load, no migration.
- Verification per change area (CLAUDE.md §2): settings → `tests/settings.test.ts`; module export → `tests/waybar-contract.test.ts`; waybar formatter → formatters/contract tests; broad → `bun test && bun run typecheck && bun run lint`.

---

### Task 1: `waybar.signal` setting + validation

**Files:**
- Modify: `src/settings.ts` (interface `Settings.waybar`, `normalizeSettings`)
- Test: `tests/settings.test.ts`

**Interfaces:**
- Produces: `Settings.waybar.signal?: number` — a positive integer in `1..30`, else absent.

- [ ] **Step 1: Write failing tests** (adapt to the file's existing describe/helpers)

```ts
it('preserves a valid waybar.signal', () => {
  const s = normalizeSettings({ waybar: { signal: 8 } } as any);
  expect(s.waybar.signal).toBe(8);
});
it('drops a non-integer / out-of-range / non-number signal', () => {
  expect(normalizeSettings({ waybar: { signal: 0 } } as any).waybar.signal).toBeUndefined();
  expect(normalizeSettings({ waybar: { signal: 3.5 } } as any).waybar.signal).toBeUndefined();
  expect(normalizeSettings({ waybar: { signal: 99 } } as any).waybar.signal).toBeUndefined();
  expect(normalizeSettings({ waybar: { signal: 'x' } } as any).waybar.signal).toBeUndefined();
});
it('leaves signal undefined by default', () => {
  expect(normalizeSettings(undefined).waybar.signal).toBeUndefined();
});
```

Note: `normalizeSettings` is not currently exported. Export it (`export function normalizeSettings`) so the test can call it directly; it is a pure function.

- [ ] **Step 2: Run, expect fail** — `bun test tests/settings.test.ts` → FAIL (signal not preserved / not exported).

- [ ] **Step 3: Implement** in `src/settings.ts`:

Add to the `Settings.waybar` interface (after `displayMode: DisplayMode;`):
```ts
    /** Waybar SIGRTMIN+N signal number for on-demand refresh (1..30). Absent = disabled. */
    signal?: number;
```
Add a validator near the other `isValid*` helpers:
```ts
function isValidWaybarSignal(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value >= 1 && value <= 30;
}
```
In `normalizeSettings`, after the displayMode validation block:
```ts
  // Validate optional Waybar refresh signal (drop invalid → disabled)
  if (merged.waybar.signal !== undefined && !isValidWaybarSignal(merged.waybar.signal)) {
    delete merged.waybar.signal;
  }
```
Export `normalizeSettings` (change `function normalizeSettings` → `export function normalizeSettings`).

- [ ] **Step 4: Run, expect pass** — `bun test tests/settings.test.ts` → PASS.

- [ ] **Step 5: Commit**
```bash
git add src/settings.ts tests/settings.test.ts
git commit -m "feat(settings): waybar.signal opcional (1..30)"
```

---

### Task 2: `signal` in the generated module export

**Files:**
- Modify: `src/waybar-contract.ts` (`WaybarModuleExportOptions`, `moduleDefinition`, `exportWaybarModules`)
- Test: `tests/waybar-contract.test.ts`

**Interfaces:**
- Consumes: `Settings.waybar.signal` (caller passes it as `options.signal`).
- Produces: `exportWaybarModules({ appBin, terminalScript, signal? }, providers)` → each module includes `signal: N` only when `signal` is a number.

- [ ] **Step 1: Write failing tests**

```ts
it('includes signal in each module when provided', () => {
  const out = exportWaybarModules({ appBin: 'bin', terminalScript: 'term', signal: 8 }, ['claude']);
  expect(out.modules['custom/agent-bar-claude'].signal).toBe(8);
});
it('omits signal when not provided', () => {
  const out = exportWaybarModules({ appBin: 'bin', terminalScript: 'term' }, ['claude']);
  expect('signal' in out.modules['custom/agent-bar-claude']).toBe(false);
});
```

- [ ] **Step 2: Run, expect fail** — `bun test tests/waybar-contract.test.ts` → FAIL.

- [ ] **Step 3: Implement** in `src/waybar-contract.ts`:

Add `signal?: number` to `WaybarModuleExportOptions`:
```ts
export interface WaybarModuleExportOptions {
  appBin: string;
  terminalScript: string;
  signal?: number;
}
```
Change `moduleDefinition` signature + body to accept and conditionally emit signal:
```ts
function moduleDefinition(provider: WaybarProviderId, appBin: string, terminalScript: string, signal?: number) {
  return {
    exec: `${appBin} --provider ${provider}`,
    'return-type': 'json',
    interval: 120,
    'exec-on-event': true,
    tooltip: true,
    'on-click': `${terminalScript} ${appBin} menu`,
    'on-click-right': `${terminalScript} ${appBin} action-right ${provider}`,
    ...(typeof signal === 'number' ? { signal } : {}),
  };
}
```
In `exportWaybarModules`, pass the signal through:
```ts
    modules[`${WAYBAR_MODULE_PREFIX}${provider}`] = moduleDefinition(
      provider,
      options.appBin,
      options.terminalScript,
      options.signal,
    );
```

- [ ] **Step 4: Run, expect pass** — `bun test tests/waybar-contract.test.ts` → PASS. Also `bun run typecheck` (the `ReturnType<typeof moduleDefinition>` in `WaybarModulesExport` now includes optional `signal` — verify no break).

- [ ] **Step 5: Commit**
```bash
git add src/waybar-contract.ts tests/waybar-contract.test.ts
git commit -m "feat(waybar): signal opcional no módulo gerado"
```

---

### Task 3: Thread `signal` through the export command and setup

**Files:**
- Modify: `src/index.ts` (`export-waybar-modules` handler, ~line 79-96)
- Modify: `src/waybar-integration.ts` (`applyWaybarIntegration`, ~line 380-394)
- Test: `tests/waybar-contract.test.ts` or `tests/cli.test.ts` (export command) if an integration test fits; otherwise covered by Task 2 unit + manual smoke.

**Interfaces:**
- Consumes: `loadSettings()/loadSettingsSync().waybar.signal`, `exportWaybarModules` from Task 2.

- [ ] **Step 1: Implement `index.ts`** — in the `export-waybar-modules` branch, add `signal` to the options object passed to `exportWaybarModules`:
```ts
        exportWaybarModules(
          {
            appBin: options.appBin ?? defaults.appBin,
            terminalScript: options.terminalScript ?? defaults.terminalScript,
            signal: settings.waybar.signal,
          },
          settings.waybar.providerOrder as WaybarProviderId[],
        ),
```
(`settings` is already loaded in that branch via `await loadSettings()`.)

- [ ] **Step 2: Implement `waybar-integration.ts`** — in `applyWaybarIntegration`, load settings before the module export and pass the signal. Move/duplicate the `loadSettingsSync()` call so it runs before `exportWaybarModules`:
```ts
  const providerOrder = resolveProviderOrder();
  const moduleIDs = getAppModuleIDs(providerOrder);
  const settings = loadSettingsSync();

  const modules = exportWaybarModules(
    {
      appBin: options.appBin ?? defaults.appBin,
      terminalScript: options.terminalScript ?? defaults.terminalScript,
      signal: settings.waybar.signal,
    },
    providerOrder,
  ).modules;
```
Then reuse `settings` for the existing CSS export (remove the later duplicate `const settings = loadSettingsSync();` to avoid redeclare).

- [ ] **Step 3: Verify** — `bun test tests/waybar-contract.test.ts tests/cli.test.ts && bun run typecheck`. Manual smoke:
```bash
XDG_CONFIG_HOME=$(mktemp -d) bun run src/index.ts export waybar-modules 2>/dev/null | python3 -c "import json,sys; m=json.load(sys.stdin)['modules']; print('signal' in list(m.values())[0])"
```
Expected: `False` (default off). Then with a settings file containing `"signal": 8`, expect `True`.

- [ ] **Step 4: Commit**
```bash
git add src/index.ts src/waybar-integration.ts
git commit -m "feat(waybar): thread waybar.signal no export e setup"
```

---

### Task 4: `percentage` + `alt` on single-provider output

**Files:**
- Modify: `src/formatters/waybar.ts` (`WaybarOutput`, `formatProviderForWaybar`)
- Test: `tests/waybar-contract.test.ts` (waybar output contract) or `tests/formatters*.test.ts`

**Interfaces:**
- Produces: `formatProviderForWaybar(quota, mode)` → `{ text, tooltip, class, alt, percentage? }`. `alt` ∈ {ok,low,warn,critical,disconnected}. `percentage` = `Math.round(toWindowDisplay(primary, mode))`, omitted when null/disconnected. `formatForWaybar` unchanged.

- [ ] **Step 1: Write failing tests**

```ts
it('emits alt (health state) and displayMode-aware percentage', () => {
  const q = { provider: 'claude', displayName: 'Claude', available: true, primary: { remaining: 70, resetsAt: null } } as any;
  const out = formatProviderForWaybar(q, 'remaining');
  expect(out.alt).toBe('ok');          // 70 >= 60
  expect(out.percentage).toBe(70);
  const used = formatProviderForWaybar(q, 'used');
  expect(used.percentage).toBe(30);    // mirrors text in used mode
});
it('maps health buckets to alt', () => {
  const mk = (rem: number) => ({ provider: 'claude', displayName: 'Claude', available: true, primary: { remaining: rem, resetsAt: null } } as any);
  expect(formatProviderForWaybar(mk(50), 'remaining').alt).toBe('low');   // 30..59
  expect(formatProviderForWaybar(mk(20), 'remaining').alt).toBe('warn');  // 10..29
  expect(formatProviderForWaybar(mk(5), 'remaining').alt).toBe('critical'); // <10
});
it('marks disconnected and omits percentage', () => {
  const q = { provider: 'claude', displayName: 'Claude', available: false, error: 'x' } as any;
  const out = formatProviderForWaybar(q, 'remaining');
  expect(out.alt).toBe('disconnected');
  expect('percentage' in out).toBe(false);
});
it('aggregate output has no alt/percentage', () => {
  const q = { providers: [{ provider: 'claude', displayName: 'Claude', available: true, primary: { remaining: 70, resetsAt: null } }], fetchedAt: 'x' } as any;
  const out = formatForWaybar(q, 'remaining');
  expect('alt' in out).toBe(false);
  expect('percentage' in out).toBe(false);
});
```

- [ ] **Step 2: Run, expect fail** — FAIL (alt/percentage undefined).

- [ ] **Step 3: Implement** in `src/formatters/waybar.ts`:

Extend `WaybarOutput`:
```ts
interface WaybarOutput {
  text: string;
  tooltip: string;
  class: string;
  alt?: string;
  percentage?: number;
}
```
In `formatProviderForWaybar`, disconnected branch — add `alt`:
```ts
  if (!quota.available || quota.error) {
    return {
      text: `<span foreground='${ONE_DARK.red}'>󱘖</span>`,
      tooltip: buildProviderTooltip(quota, undefined, mode),
      class: `${APP_BASE_CLASS}-${quota.provider} disconnected`,
      alt: 'disconnected',
    };
  }
```
Connected branch — add `alt` (the existing `status`) and `percentage` (rounded `disp` when not null):
```ts
  return {
    text: pctColored(disp, mode),
    tooltip: buildProviderTooltip(quota, undefined, mode),
    class: `${APP_BASE_CLASS}-${quota.provider} ${status}`,
    alt: status,
    ...(disp !== null ? { percentage: Math.round(disp) } : {}),
  };
```
`formatForWaybar`/`formatText`/`getClass` (aggregate) unchanged.

- [ ] **Step 4: Run, expect pass** — `bun test tests/waybar-contract.test.ts tests/formatters.test.ts` → PASS. Update any byte-for-byte Waybar snapshot that now carries `alt`/`percentage` only if the contract intentionally moved (re-run with `--update-snapshots` after confirming the diff is exactly the new fields).

- [ ] **Step 5: Commit**
```bash
git add src/formatters/waybar.ts tests/
git commit -m "feat(waybar): percentage + alt na saída single-provider"
```

---

### Task 5: Document the new fields, `signal`, and refresh recipe

**Files:**
- Modify: `docs/waybar-contract.md`

- [ ] **Step 1: Add a "Single-provider output fields" subsection** documenting `text`/`tooltip`/`class` plus the new `alt` (state enum) and `percentage` (displayMode-aware, omitted when disconnected). Show a `format-icons` example by `alt` (object) and by `percentage` (array).

- [ ] **Step 2: Add a "Signal (on-demand refresh)" subsection**: enable via `"signal": <N>` under `waybar` in settings; the generated module then carries `signal: N`. Refresh recipe (fresh data needs cache invalidation first):
```bash
agent-bar -p claude -r && pkill -RTMIN+8 waybar
```
Plus a Claude Code Stop-hook example that runs the recipe. Note `exec` is unchanged, so a bare signal re-renders cached data.

- [ ] **Step 3: Verify** — `git diff --check` (no whitespace errors).

- [ ] **Step 4: Commit**
```bash
git add docs/waybar-contract.md
git commit -m "docs: percentage/alt + signal no waybar-contract"
```

---

### Final verification (after all tasks)

- [ ] `bun test && bun run typecheck && bun run lint` — all green.
- [ ] Smoke: `bun run src/index.ts --provider claude 2>/dev/null | python3 -c "import json,sys; d=json.load(sys.stdin); print('alt=',d.get('alt'),'pct=',d.get('percentage'))"` → real `alt`/`percentage`.
- [ ] `/code-review master`.
