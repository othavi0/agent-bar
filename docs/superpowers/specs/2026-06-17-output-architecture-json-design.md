# Design — Arquitetura de output: `--format json` + `--watch`

- **Data:** 2026-06-17
- **Status:** aprovado (design); aguardando revisão da spec
- **Escopo:** Tier 3, sub-projeto A (arquitetura de output) da auditoria de 2026-06-17
- **Branch:** `analise`

## Contexto e objetivo

O agent-bar hoje só emite o envelope específico do Waybar (`{text, tooltip, class}`
com Pango markup). O Omarchy está migrando para Quickshell (DHH PR
basecamp/omarchy#5856), que renderiza nativamente em QML e consome **JSON cru**
(Pango apareceria como texto literal). O bar Quickshell do Omarchy mantém um
protocolo command-module compatível com Waybar, então o agent-bar **continua
funcionando sem mudança** após a migração — mas para cards nativos ricos (e para
Eww/Ironbar/qualquer bar não-Waybar) é preciso uma saída estruturada estável.

**Objetivo:** expor um contrato JSON estável, versionado e documentado, mantendo
o Waybar como default e sem quebrar nada. Driver = **prontidão** (não há
consumidor hoje); o plugin QML nativo do Omarchy é passo futuro (quando v4 sair).

Não-objetivos (YAGNI agora): plugin QML nativo do Omarchy, socket IPC/push,
formatos não-JSON, qualquer mudança em providers ou cache.

## Decisões (locked no brainstorming)

1. **Superfície CLI = flag**, não comando novo (encaixa no parser atual).
2. **Modelo completo** na saída: espelha `ProviderQuota` (pré-render, zero Pango),
   incluindo `used` (Copilot), weekly/models, extra usage, account, plan.
3. **Reuso do modelo interno** via um mapper explícito (fronteira de contrato),
   não dump cru do tipo interno.
4. **`--watch`** incluído agora: stream NDJSON (padrão eficiente do Quickshell
   `Process` + `SplitParser`).

## Superfície CLI

| Flag | Efeito |
| --- | --- |
| `--format <waybar\|json>` | Default `waybar` (comportamento atual intacto). |
| `--watch` | **Implica `--format json`.** Processo longo; emite 1 envelope por linha (NDJSON) a cada intervalo. |
| `--interval <segundos>` | Só com `--watch`; default **60**. Inteiro positivo. |

Regras de combinação:

- `--watch` sem `--format` → assume `json`.
- `--watch` com `--format waybar` explícito → erro stderr + exit 1
  (`--watch requires --format json`).
- `--interval` fora de `--watch` → ignorado (sem efeito; não é erro).
- `--interval` não-numérico ou ≤ 0 → erro stderr + exit 1.
- `--format` com valor inválido → erro stderr + exit 1.
- Funciona com `--provider X` (single) ou sem (todos).

Exemplos:

```bash
agent-bar --format json                    # snapshot, todos os providers
agent-bar --format json --provider claude  # snapshot, só Claude
agent-bar --watch                          # stream NDJSON, intervalo 60s
agent-bar --watch --interval 30 --provider codex
```

`--help` ganha entradas para `--format`, `--watch`, `--interval`.

## Schema do contrato (versionado)

Envelope **sempre** o mesmo shape — mesmo com 1 provider (array de 1 elemento):

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
      "extra":     { "weeklyModels": { "Sonnet": { "remaining": 89, "resetsAt": "…" } } },
      "error": null
    },
    {
      "provider": "amp",
      "displayName": "Amp",
      "available": false,
      "error": "Amp CLI not installed. Right-click to install and log in."
    }
  ]
}
```

Em `--watch`, cada tick imprime **um** desses envelopes numa única linha
terminada por `\n` (NDJSON).

### Tipos de saída (declarados no mapper)

```ts
interface JsonWindow {
  remaining: number;
  used?: number | null;
  resetsAt: string | null;
  windowMinutes?: number | null;
}

interface JsonProvider {
  provider: string;
  displayName: string;
  available: boolean;
  account?: string;
  plan?: string;
  planType?: string;
  primary?: JsonWindow;
  secondary?: JsonWindow;
  models?: Record<string, JsonWindow>;
  /** Provider-specific bloco de dados pré-render (weeklyModels, modelsDetailed, extraUsage, meta, quotaSnapshots). */
  extra?: Record<string, unknown>;
  error?: string;
}

interface JsonOutput {
  schemaVersion: number;
  fetchedAt: string;
  providers: JsonProvider[];
}
```

Campos omitidos quando ausentes (não emitir `undefined`). `error: null` é
aceitável no exemplo, mas a implementação **omite** `error` quando não há erro
(consistente com o resto: campo ausente = sem valor). Documentar isso.

## Componentes

| Arquivo | Mudança |
| --- | --- |
| `src/formatters/json.ts` (novo) | Puro, sem I/O. `toJsonOutput(quotas: AllQuotas): JsonOutput` e `toProviderOutput(p: ProviderQuota): JsonProvider`. **Cópia campo-a-campo explícita** (não spread), para que adicionar um campo interno NÃO vaze pro contrato sem decisão consciente. Esta é a fronteira de contrato. |
| `src/index.ts` | Branch para `--format json` one-shot e para `--watch` (loop). Reusa `getAllQuotas()` / `getQuotaFor()`. |
| `src/cli.ts` | Parse de `--format`, `--watch`, `--interval`; validação; entradas no `--help`. Estende `CliOptions`. |
| Providers, cache, settings, render-pango, builders | **Sem mudança.** |

### Constante de versão

`SCHEMA_VERSION = 1` definida em `src/formatters/json.ts` e exportada para os
testes assertarem. Bump manual e consciente em mudança incompatível de schema.

## Data flow

**One-shot json:**

```
parseArgs → format=json
  → (provider? getQuotaFor(p) : getAllQuotas())
  → toJsonOutput(allQuotas)
  → console.log(JSON.stringify(output))
  → exit 0
```

**watch:**

```
emite imediatamente (tick 0), depois a cada `interval` segundos:
  tick():
    quotas = await (provider? getQuotaFor : getAllQuotas)
    process.stdout.write(JSON.stringify(toJsonOutput(quotas)) + '\n')
  loop com setTimeout reagendado APÓS cada tick terminar (evita overlap/drift
  quando um fetch demora mais que o intervalo).
  SIGTERM/SIGINT → para o timer e exit 0 (handlers já existem em index.ts).
```

- **Cache-aware** em ambos: `getAllQuotas`/`getQuotaFor` respeitam o cache (TTL
  5min), então em watch com intervalo 60s a rede é tocada ~a cada 5min e os
  ticks intermediários emitem dado cacheado. Sem hammer de API.
- **Seleção de providers:** json/watch emitem **todos os providers registrados**
  (desacoplado de `settings.waybar.providers` — consumidor não-Waybar decide o
  que mostrar). `--provider X` → envelope com 1 provider.
- `fetchedAt` = momento do `getAllQuotas` daquele tick (já setado lá).

## Tratamento de erro

- **Erro por-provider** vai no campo `error` do `JsonProvider` (não-fatal).
  `getAllQuotas` já captura por provider e nunca rejeita → cada tick sempre
  produz um envelope válido.
- **stdout 100% JSON/NDJSON.** Todo log vai pra stderr (o `logger` já faz; em
  modo waybar/json o logger fica silent salvo `--verbose`). Em watch, garantir
  que nada além das linhas JSON chegue ao stdout.
- **Erros de uso** (`--format` inválido, `--interval` inválido, `--watch` +
  `--format waybar`) → mensagem clara em stderr + exit 1.
- Em watch, se um tick lançar (improvável, dado que getAllQuotas captura),
  logar em stderr e continuar o loop (não derrubar o stream).

## Testes

- **`tests/formatters-json.test.ts`** (novo):
  - shape do envelope (`schemaVersion`, `fetchedAt`, `providers`);
  - `SCHEMA_VERSION` exportado e refletido na saída;
  - **ausência de Pango**: `expect(JSON.stringify(out)).not.toContain('<span')`;
  - mapeia primary/secondary/models/extra/used corretamente;
  - provider com erro: `available:false` + `error` presente, sem `primary`;
  - campos ausentes são omitidos (não `undefined` serializado);
  - single (`--provider`) vs all.
- **`tests/cli.test.ts`** (estende): parse de `--format json`, `--watch`
  (implica json), `--interval N`; erros (`--format xpto`, `--interval abc`,
  `--watch --format waybar`).
- **watch**: extrair o corpo do tick numa função pura (`buildWatchLine` ou
  `runTick`) e testá-la isolada; não testar `setTimeout`.
- Regressão: `bun test && bun run typecheck && bun run lint` continuam verdes
  (matriz §2 do CLAUDE.md: contrato Waybar, formatters, cli).

## Docs

- **`docs/json-output.md`** (novo): descrição do contrato; tabela de campos com
  tipos e semântica; nota de estabilidade (`schemaVersion`, política de bump);
  os modos (`--format json` one-shot, `--watch` NDJSON, `--interval`); **exemplo
  QML Quickshell** — one-shot via `StdioCollector.onStreamFinished` + `JSON.parse`,
  e stream via `Process` + `SplitParser` (delimitador `\n`) + `JSON.parse`,
  lembrando que `command` é array (sem shell). Nota: plugin QML nativo do Omarchy
  = passo futuro quando o v4 (Quickshell) sair.
- Linkar em `README.md` (seção Docs) e `docs/README.md`.
- Adicionar `docs/json-output.md` ao `files` do `package.json` (publicado no npm,
  como os outros docs) e ao teste `tests/package.test.ts`.

## Riscos e mitigações

- **Acoplamento do contrato ao tipo interno** → mapper com cópia campo-a-campo
  explícita + `schemaVersion` + doc de estabilidade.
- **Drift de intervalo / overlap em watch** → reagendar `setTimeout` após o tick
  terminar (não `setInterval`).
- **Vazamento de log no stdout em watch** → logger silent por default; teste de
  ausência de Pango/ruído onde viável.
- **Pango vazar pro JSON** → o mapper parte de `ProviderQuota` (pré-render); teste
  explícito `not.toContain('<span')`.

## Fora de escopo (futuro)

- Plugin QML nativo `~/.config/omarchy/plugins/agent-bar/` (espera Omarchy v4).
- Socket Unix / push IPC.
- Modo de seleção de providers configurável especificamente para json.
