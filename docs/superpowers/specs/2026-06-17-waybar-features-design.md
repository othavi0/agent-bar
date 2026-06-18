# Design — Waybar features: `percentage`/`alt` (format-icons) + `signal`

- **Data:** 2026-06-17
- **Status:** aprovado (design); aguardando revisão da spec
- **Escopo:** Tier 3 — features de integração Waybar subutilizadas
- **Branch:** `master`

## Contexto e objetivo

A saída Waybar do agent-bar hoje emite só `{text, tooltip, class}`. O custom
module do Waybar suporta mais dois campos no JSON — `percentage` e `alt` — que
desbloqueiam `format-icons` (ícones por estado de quota), e um config key
`signal` que permite refresh sob demanda sem esperar o `interval`. Nenhum dos
dois está exposto.

**Objetivo:** emitir `percentage`/`alt` na saída single-provider (desbloqueando
`format-icons`) e suportar `signal` opt-in no módulo gerado (com recipe de
refresh fresco documentado), de forma 100% aditiva — sem quebrar o contrato
atual nem mudar o comportamento de quem não opta.

**Não-objetivos (YAGNI agora):** `percentage`/`alt` no módulo agregado;
`format` template custom; `signal` ligado por default; mudar o `exec` do módulo
para `--refresh` (mataria o cache de 5 min em todo poll de `interval`).

## Contrato Waybar (verificado na wiki oficial)

- Campos JSON do custom module com `return-type: json`: `text`, `alt`,
  `tooltip`, `class`, `percentage`.
- `format-icons`: **array** → indexado por `percentage` (low→high); **objeto**
  → keyed por `alt`; **nested** → `alt` primeiro, depois `percentage`. Emitir os
  dois dá flexibilidade máxima ao usuário.
- `signal`: `SIGRTMIN+N`, N válido de 1 a (SIGRTMAX − SIGRTMIN). Re-executa o
  **mesmo** `exec` do módulo. Detalhe load-bearing: o re-exec lê o cache de
  quota (5 min), então refresh via signal só busca dados frescos se o cache for
  invalidado antes (`agent-bar -p X -r`).

## Feature A — `percentage` + `alt` (single-provider)

`formatProviderForWaybar` (`src/formatters/waybar.ts`) passa a incluir, além de
`{text, tooltip, class}`:

- **`percentage`** (`number`): o display value displayMode-aware, idêntico ao
  número que o `text` mostra — `toWindowDisplay(quota.primary, mode)`
  arredondado. **Omitido** quando não há dado (`null`) e no estado disconnected.
- **`alt`** (`string`): o health state — `ok` / `low` / `warn` / `critical`
  (de `getStatusForPercent(remaining)`, baseado no `remaining` cru) ou
  `disconnected` quando `!available || error`. É **a mesma string já presente no
  `class`**, portanto sempre um dos 5 tokens fixos.

`WaybarOutput` ganha `percentage?: number` e `alt?: string` (opcionais). O
campo `alt` é sempre emitido (inclusive disconnected); `percentage` é emitido só
quando há valor.

`formatForWaybar` (agregado) **permanece inalterado** — sem `percentage`/`alt`.

### Decisões e justificativas

- **percentage displayMode-aware** (vs sempre-remaining): mantém a saída
  internamente consistente — `text`, `percentage` e cor refletem o mesmo modo.
  O caso dominante de ícones-por-estado é resolvido pelo `alt` (estável), então
  o `percentage` serve pro `{percentage}` no `format` e ramps numéricos, onde
  bater com o `text` é o menos surpreendente.
- **alt = health states existentes**: zero vocabulário novo; o `alt` carrega o
  mesmo estado que o `class` já expõe, num campo que `format-icons` consome
  direto.
- **Sem escape/Pango**: `alt` é um enum interno fixo (nunca dado livre de
  provider) e `percentage` é número — fora da fronteira de escape do
  `render-pango.ts`.

## Feature B — `signal` opt-in no módulo gerado

- **Settings** (`src/settings.ts`): novo `waybar.signal?: number`, default
  **ausente** (off). Não entra em `DEFAULT_SETTINGS` (off por omissão).
  Normalização em `normalizeSettings`: aceita só inteiro positivo numa faixa
  segura do Waybar; valor inválido (não-inteiro, ≤ 0, ou fora da faixa) é
  **dropado** (campo removido → off). Faixa: `1..30` (cobre a janela real
  `SIGRTMIN+N` na maioria das libcs; conservador).
- **`waybar-contract.ts`**: `WaybarModuleExportOptions` ganha `signal?: number`;
  `moduleDefinition` inclui `signal: N` **só quando presente**;
  `exportWaybarModules` repassa o valor.
- **Threading**: `index.ts` (comando `export waybar-modules`) e
  `waybar-integration.ts` (`applyWaybarIntegration`, usado pelo `setup`) leem
  `settings.waybar.signal` e passam adiante. Em `applyWaybarIntegration`, o
  `loadSettingsSync()` (hoje na linha ~396, depois do export de módulos) passa a
  ser lido antes, para alimentar o export.
- **`exec` do módulo não muda** (`agent-bar --provider X`): refresh fresco é via
  recipe documentado, não por alterar o comando que roda a cada `interval`.

## Docs (`docs/waybar-contract.md`)

- Novos campos `percentage`/`alt` na saída single-provider, com exemplo de
  `format-icons` por `alt` (objeto) e por `percentage` (array).
- O config `signal` no módulo: como ligar (`waybar.signal` no settings) e o
  recipe de refresh on-demand: `agent-bar -p <provider> -r && pkill -RTMIN+<N> waybar`,
  com exemplo de Stop hook do Claude Code que dispara o refresh ao terminar uma
  task.

## Testes

- **`tests/settings.test.ts`**: `waybar.signal` válido é preservado;
  não-inteiro / ≤ 0 / fora da faixa é dropado (off); ausência = off.
- **`tests/waybar-contract.test.ts`**: `exportWaybarModules` inclui `signal: N`
  quando o setting está presente e o omite quando ausente; demais campos do
  módulo inalterados.
- **`tests/formatters*.test.ts` / `tests/waybar-contract.test.ts`**:
  `formatProviderForWaybar` emite `percentage` (displayMode-aware, batendo com o
  `text`) e `alt` (estado correto por bucket de health); omite `percentage` e
  marca `alt: 'disconnected'` no estado disconnected; `formatForWaybar`
  (agregado) não emite os campos.

## Boundaries e risco

- **100% aditivo**: Waybar ignora campos JSON extras; consumidores atuais
  (incl. o contrato `--format json`, que é outro caminho — `json.ts` — e **não**
  é tocado) ficam intactos.
- **Settings schema v2 mantido**: `waybar.signal` é opcional e normalizado na
  carga; não exige bump de versão nem migração.
- **Sem colisão por default**: `signal` só entra no módulo quando o usuário opta;
  default off elimina risco de colidir com outros módulos do Waybar do usuário.
- **Idempotência do setup**: o módulo gerado com/sem `signal` continua passando
  pelo patch in-place do `waybar-integration.ts` (sem round-trip
  `JSON.parse`/`stringify` da config viva).
