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

Regras de combinação (validadas em um **bloco pós-parse** no fim de `parseArgs`,
não dentro do `switch` sequencial — a checagem cruza flags):

- `--watch` sem `--format` → assume `json`.
- `--watch` com `--format waybar` explícito → erro stderr + exit 1
  (`--watch requires --format json`).
- `--interval` fora de `--watch` → **warning em stderr** (`--interval has no
  effect without --watch`); não-fatal.
- `--interval` não-numérico ou ≤ 0 → erro stderr + exit 1.
- `--format` com valor inválido → erro stderr + exit 1.
- Funciona com `--provider X` (single) ou sem (todos).

`--format` é **ortogonal ao `command`**: `command` continua resolvendo pra
`'waybar'` por default; o branch de output decide pela flag `options.format`,
não pelo `command`. Hoje `--format` é uma option desconhecida que cai no
`default` do parser e **degrada silenciosamente pra Waybar** — a implementação
precisa adicionar os cases explicitamente (ver CliOptions abaixo).

### CliOptions (parser)

`CliOptions` em `src/cli.ts` ganha:

```ts
format: 'waybar' | 'json'; // default 'waybar'
watch: boolean;            // default false
intervalSeconds: number;   // default 60 (lido só em --watch)
```

Novos cases no `switch`: `--format` (usa `requireNextArg`, valida `waybar|json`),
`--watch` (seta `watch=true`), `--interval` (usa `requireNextArg`, `parseInt`,
valida `> 0`). O bloco pós-parse aplica as regras de combinação acima.

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
      "extra":     { "weeklyModels": { "Sonnet": { "remaining": 89, "resetsAt": "2026-06-19T22:59:59Z" } } }
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

**Convenção de ausência:** campo **omitido** quando ausente (nunca `null`
explícito, nunca `undefined` serializado). Inclui `error` — provider sem erro
não tem a chave `error` (consumidor checa `'error' in p`, não `p.error !== null`).

**Estabilidade / `schemaVersion`:** os campos top-level e `primary`/`secondary`/
`models` (tipo `JsonWindow`) são o **contrato estável** coberto pelo
`schemaVersion`. O bloco `extra` espelha estruturas internas provider-specific
(`weeklyModels`, `modelsDetailed` com `ModelWindows`, `extraUsage`, `meta`,
`quotaSnapshots`) e é declarado **best-effort/instável**: pode mudar sem bump de
`schemaVersion`. Documentar isso em destaque no `docs/json-output.md`.
Política de bump: incrementar `schemaVersion` ao **remover** campo estável,
**renomear**, ou **mudar o tipo/semântica** de um campo estável; **adicionar**
campo opcional novo NÃO exige bump.

**`fetchedAt`:** é o instante em que o agent-bar **produziu o snapshot** (a
chamada `getAllQuotas`), não quando o dado foi buscado da rede. Em cache hit
(comum em `--watch` com intervalo < TTL de 5min) o dado subjacente pode ser até
~5min mais velho. Documentar essa semântica; expor idade real por-provider
(`CacheEntry.fetchedAt`) fica para v2 (fora de escopo).

## Componentes

| Arquivo | Mudança |
| --- | --- |
| `src/formatters/json.ts` (novo) | Puro, sem I/O. `toJsonOutput(quotas: AllQuotas): JsonOutput` e `toProviderOutput(p: ProviderQuota): JsonProvider`. **Cópia campo-a-campo explícita** (não spread), para que adicionar um campo interno NÃO vaze pro contrato sem decisão consciente. Esta é a fronteira de contrato. |
| `src/index.ts` | Branch para `--format json` one-shot e para `--watch` (loop). Reusa `getAllQuotas()` / `getQuotaFor()`. **Dois guards do caminho Waybar precisam ser desviados no modo json:** (1) o gate `if (!settings.waybar.providers.includes(options.provider))` que cospe o envelope *hidden* (atual `index.ts:152`) — pular quando `format==='json'`; (2) o filtro `if (options.command === 'waybar')` (atual `index.ts:172`) — passar a `if (options.command === 'waybar' && options.format !== 'json')`. Single-provider em json reusa o mesmo wrapping `{ providers: [quota], fetchedAt }` já feito pro Waybar (extrair helper para não duplicar). |
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
setup (uma vez):
  process.stdout.on('error', e => { if (e.code === 'EPIPE') process.exit(0) })  // [H1]
  se options.refresh → invalidar cache UMA vez antes do loop                     // [--refresh]
  let stopping = false; SIGTERM/SIGINT → stopping = true; process.exit(0)

tick():
  quotas = await (provider ? wrap(getQuotaFor(p)) : getAllQuotas())
  const line = JSON.stringify(toJsonOutput(quotas)) + '\n'   // linha ATÔMICA (1 write)
  process.stdout.write(line, () => {                          // agenda no callback → backpressure-aware [M4]
    if (!stopping) setTimeout(tick, intervalSeconds * 1000)
  })

start: tick()  // emite imediatamente (tick 0), depois reagenda
```

- **Linha atômica:** cada tick faz **um** `write` da linha completa. Logo um
  SIGTERM no meio do `await` só causa "falta a última linha", nunca uma linha
  NDJSON parcial — não precisa esperar tick in-flight. [L5]
- **EPIPE:** consumidor fechando o pipe (reload do bar, lock, logout) → handler
  sai com 0 em vez de crash. [H1]
- **Agendamento no callback do `write`** (não `setInterval`): serializa emissão
  e scheduling e é sensível a backpressure; evita overlap/drift. [M4]
- **`--interval` é um piso, não período exato:** cada provider tem timeout de
  `PROVIDER_TIMEOUT_MS` (10s) + 1 retry; um tick lento pode levar ~21s, então o
  período efetivo é `max(interval, duração do tick)`. Documentar no `--help` e na
  doc; intervalos < ~10s não dão a cadência ingênua. [H3]
- **`--refresh`** invalida o cache **uma vez** antes do loop (não por tick). [missing]
- **`--verbose`** em watch emite debug por tick em **stderr** (stdout segue puro). [missing]

Comum a one-shot e watch:

- **Cache-aware:** `getAllQuotas`/`getQuotaFor` respeitam o cache (TTL 5min);
  em watch com intervalo 60s a rede é tocada ~a cada 5min e os ticks
  intermediários emitem dado cacheado. Sem hammer de API. O `inflight` Map do
  `Cache` se limpa no `.then`/`.catch` → **sem leak** em processo longo. [missing]
- **Seleção de providers:** json/watch emitem **todos os providers registrados**
  (desacoplado de `settings.waybar.providers`). `--provider X` → envelope com 1
  provider (via `getQuotaFor` embrulhado em `AllQuotas`).
- **TTY interativo:** rodar `agent-bar --watch` num terminal cospe NDJSON cru;
  quando `process.stdout.isTTY`, emitir **uma** dica em stderr ("watch mode:
  output is NDJSON — pipe to a consumer"). [missing]

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
  - provider OK: **chave `error` ausente** (`expect('error' in p).toBe(false)`),
    não `null`; idem demais campos opcionais ausentes;
  - single (`--provider`) vs all.
- **`tests/cli.test.ts`** (estende): parse de `--format json`, `--watch`
  (implica json), `--interval N`; `--interval` sem `--watch` (warning, não erro);
  erros (`--format xpto`, `--interval abc`, `--interval 0`, `--watch --format
  waybar`).
- **watch** (extrair `runTick()` puro que retorna a linha; não testar timers):
  - `runTick` produz uma linha NDJSON válida terminada em `\n`;
  - tick 0 emite imediatamente (chamar `runTick` uma vez sem timer);
  - **não-overlap**: com `getAllQuotas` mockado com delay, garantir que o próximo
    tick só agenda após o `write` callback (testar a função de agendamento, ou
    documentar como limitação se inviável sem fake timers);
  - **EPIPE**: simular `stdout` emitindo `error` com `code:'EPIPE'` → handler
    chama `process.exit(0)` (spy em `process.exit`).
- Regressão: `bun test && bun run typecheck && bun run lint` continuam verdes
  (matriz §2 do CLAUDE.md: contrato Waybar, formatters, cli).

## Docs

- **`docs/json-output.md`** (novo): descrição do contrato; tabela de campos com
  tipos e semântica; **nota de estabilidade** (campos top-level + `JsonWindow`
  são estáveis sob `schemaVersion`; `extra` é best-effort/instável — em destaque)
  + política de bump (remover/renomear/mudar tipo de campo estável = bump;
  adicionar opcional = não); semântica de `fetchedAt` (snapshot, não fetch);
  os modos (`--format json` one-shot, `--watch` NDJSON, `--interval` como **piso**
  com nota dos timeouts de provider); **exemplo QML Quickshell** — one-shot via
  `StdioCollector.onStreamFinished` + `JSON.parse`, e stream via `Process` +
  `SplitParser` (delimitador `\n`) + `JSON.parse`, lembrando que `command` é array
  (sem shell). Nota: plugin QML nativo do Omarchy = passo futuro quando o v4
  (Quickshell) sair.
- Linkar em `README.md` (seção Docs) e `docs/README.md`.
- Adicionar `docs/json-output.md` ao `files` do `package.json` (publicado no npm,
  como os outros docs) e ao teste `tests/package.test.ts`.

## Riscos e mitigações

- **EPIPE no watch** (consumidor fecha o pipe) → `process.stdout.on('error')`
  com saída 0 em EPIPE. [H1]
- **`--format` degradar silenciosamente pra Waybar** (option desconhecida cai no
  `default` do parser) → adicionar cases explícitos + bloco de validação. [H4]
- **`--provider X` + json bater no gate de `waybar.providers`** (cospe envelope
  hidden) → desviar os guards do `index.ts` no modo json. [H2/M7]
- **Acoplamento do contrato ao tipo interno** → mapper com cópia campo-a-campo
  explícita + `schemaVersion` + `extra` marcado instável na doc.
- **Drift/overlap/backpressure em watch** → agendar próximo tick no callback do
  `write` (não `setInterval`); `--interval` é piso, documentado. [M4/H3]
- **`fetchedAt` enganar em cache hit** → documentar que é hora do snapshot, não do
  fetch de rede. [M3]
- **Vazamento de log no stdout em watch** → logger silent por default; debug só
  com `--verbose`, sempre em stderr.
- **Pango vazar pro JSON** → o mapper parte de `ProviderQuota` (pré-render); teste
  explícito `not.toContain('<span')`.
- **Divergência de seleção de providers** (json mostra desabilitados no Waybar) →
  decisão consciente (consumidor não-Waybar é independente); documentar.

## Fora de escopo (futuro)

- Plugin QML nativo `~/.config/omarchy/plugins/agent-bar/` (espera Omarchy v4).
- Socket Unix / push IPC.
- Modo de seleção de providers configurável especificamente para json.
