# Fase 3a — Refatoração dos formatters — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminar a tripla duplicação dos builders de formatação (`terminal.ts` ANSI / `waybar.ts` Pango / `tui/list-all.ts` colorize) via um pipeline funcional segment-emitter, sem alterar a saída observável.

**Architecture:** Pipeline de 3 estágios — `resolve` (business logic isolada em `view-model.ts`) → `build` (um builder puro por provider, emite `Line[]` abstrato) → `render` (3 renderers finos: ANSI/Pango/colorize). O modelo `Line`/`Segment` estende o `segments.ts` atual. Refatoração protegida pela suíte de snapshot.

**Tech Stack:** TypeScript strict, Bun (`bun:test`), Biome.

---

## Contexto para o engenheiro

`agent-bar` é um monitor de quota LLM para Waybar. A camada de formatação tem 3
implementações paralelas com ~30-40% de duplicação estrutural. Spec de origem:
`docs/superpowers/specs/2026-05-15-fase-3a-refatoracao-formatters-design.md`.

Convenções: **Bun apenas** (`bun test`, `bun run typecheck`, `bun run lint`).
TypeScript strict. Biome (2 espaços, aspas simples, 120 colunas, import não usado =
erro). Commits em Português, Conventional Commits, subject ≤ 50 chars.

### O oráculo de correção: snapshots — e uma assimetria crítica

`tests/formatters-snapshot.test.ts` é a rede de segurança. A função `sanitize()`
nesse arquivo **remove códigos ANSI** (`ANSI_RE`). Consequência:

- **Snapshot de terminal:** valida texto e layout (espaçamento, box chars, padding),
  **não** os bytes ANSI. Reposicionar códigos ANSI é livre desde que o texto visível
  e o layout fiquem idênticos.
- **Snapshot de Waybar:** o markup Pango **não** é ANSI, então `sanitize()` não o
  remove — o snapshot captura o Pango **inteiro**. A saída de Waybar precisa ficar
  **byte-idêntica**.
- **Snapshot de TUI:** se existir, idem terminal (ANSI/colorize via `tui/colors.ts`).

**Esta assimetria é o risco central da fase.** Ver "Risco conhecido" no fim.

## Estrutura de arquivos

| Arquivo | Papel |
| --- | --- |
| `src/providers/extras.ts` *(novo)* | Getters de type-narrowing — contêm o cast `as XxxQuotaExtra` em 1 lugar |
| `src/formatters/segments.ts` *(estendido)* | `ColorToken` ampliado + tipo `Line` |
| `src/formatters/view-model.ts` *(novo)* | Resolve settings/window policy/filtro Codex |
| `src/formatters/builders/{claude,codex,amp,copilot}.ts` *(novos)* | Builder puro por provider → `Line[]` |
| `src/formatters/render-ansi.ts` *(novo)* | `Line[]` → string ANSI |
| `src/formatters/render-pango.ts` *(novo)* | `Line[]` → string Pango (com escape XML) |
| `src/tui/render-colorize.ts` *(novo)* | `Line[]` → string via `colorize` |
| `src/formatters/terminal.ts` | Encolhe para dispatcher |
| `src/formatters/waybar.ts` | Encolhe para dispatcher |
| `src/tui/list-all.ts` | Encolhe para dispatcher |
| `src/tui/configure-models.ts` | Passa a usar `getCodexExtra` |

## Sequência das tasks

Cada task termina com `bun test && bun run typecheck && bun run lint` verde. As
Tasks 1-2 são preparatórias e neutras quanto a saída. A Task 3 cria o modelo. A
Task 4 constrói o pipeline + migra Claude. As Tasks 5-7 migram um provider cada.
A Task 8 colapsa o código antigo.

---

## Task 1: `extras.ts` — getters de type-narrowing

Elimina os 13 casts `as XxxQuotaExtra` espalhados, concentrando-os em 4 getters.
`ProviderQuota` é uma union discriminada, mas `GenericQuota` tem `provider: string`
(não-literal), o que impede o narrowing puro — por isso o cast permanece, **contido**
nos 4 getters em vez de espalhado em 4 arquivos.

**Files:**
- Create: `src/providers/extras.ts`
- Modify: `src/formatters/terminal.ts`, `src/formatters/waybar.ts`, `src/tui/list-all.ts`, `src/tui/configure-models.ts`

- [ ] **Step 1: Criar `src/providers/extras.ts`**

```typescript
import type {
  AmpQuotaExtra,
  ClaudeQuotaExtra,
  CodexQuotaExtra,
  CopilotQuotaExtra,
  ProviderQuota,
} from './types';

/** Returns the Claude-specific `extra` payload, or undefined for other providers. */
export function getClaudeExtra(q: ProviderQuota): ClaudeQuotaExtra | undefined {
  return q.provider === 'claude' ? (q.extra as ClaudeQuotaExtra | undefined) : undefined;
}

/** Returns the Codex-specific `extra` payload, or undefined for other providers. */
export function getCodexExtra(q: ProviderQuota): CodexQuotaExtra | undefined {
  return q.provider === 'codex' ? (q.extra as CodexQuotaExtra | undefined) : undefined;
}

/** Returns the Amp-specific `extra` payload, or undefined for other providers. */
export function getAmpExtra(q: ProviderQuota): AmpQuotaExtra | undefined {
  return q.provider === 'amp' ? (q.extra as AmpQuotaExtra | undefined) : undefined;
}

/** Returns the Copilot-specific `extra` payload, or undefined for other providers. */
export function getCopilotExtra(q: ProviderQuota): CopilotQuotaExtra | undefined {
  return q.provider === 'copilot' ? (q.extra as CopilotQuotaExtra | undefined) : undefined;
}
```

- [ ] **Step 2: Substituir os casts nos 4 arquivos**

Em `src/formatters/terminal.ts`, `src/formatters/waybar.ts`, `src/tui/list-all.ts`,
`src/tui/configure-models.ts`: localizar cada expressão da forma
`p.provider === 'xxx' ? (p.extra as XxxQuotaExtra | undefined) : undefined` e
substituir pela chamada do getter correspondente (`getClaudeExtra(p)`,
`getCodexExtra(p)`, `getAmpExtra(p)`, `getCopilotExtra(p)`). Importar os getters de
`../providers/extras` (ou `../../providers/extras` conforme a profundidade). Ajustar
os imports de tipo que ficarem órfãos (Biome aponta import não usado).

- [ ] **Step 3: Verificar**

Run: `bun test && bun run typecheck && bun run lint`
Expected: PASS. `grep -rn "as ClaudeQuotaExtra\|as CodexQuotaExtra\|as AmpQuotaExtra\|as CopilotQuotaExtra" src` deve retornar **somente** as 4 ocorrências dentro de `src/providers/extras.ts`. Snapshots inalterados (a saída é idêntica — só trocou cast por chamada de função).

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor: concentra casts de extra em providers/extras.ts"
```

---

## Task 2: `view-model.ts` — isolar a business logic

Hoje `terminal.ts` (`buildCodex`), `waybar.ts` e `tui/list-all.ts` carregam settings
e aplicam window policy + filtro de modelos Codex inline. Extrair essa resolução
para um módulo dedicado.

**Files:**
- Create: `src/formatters/view-model.ts`
- Modify: `src/formatters/terminal.ts`, `src/formatters/waybar.ts`, `src/tui/list-all.ts`

- [ ] **Step 1: Criar `src/formatters/view-model.ts`**

O módulo expõe uma função que, dada uma `ProviderQuota` e a `DisplayMode`, devolve
o que os builders precisam sem que eles toquem em settings. Mínimo necessário hoje
(Codex): a lista de `CodexModelEntry` já filtrada e a `WindowPolicy` do provider.

```typescript
import type { ProviderQuota } from '../providers/types';
import { loadSettingsSync, type WindowPolicy } from '../settings';
import { applyCodexModelFilter, type CodexModelEntry, codexModelsFromQuota } from './codex-helpers';

/** Codex view data resolved from settings — what the Codex builder needs. */
export interface CodexViewModel {
  models: CodexModelEntry[];
  policy: WindowPolicy;
}

/** Resolve the Codex view model: filtered models + window policy, from settings. */
export function resolveCodexViewModel(p: ProviderQuota): CodexViewModel {
  const settings = loadSettingsSync();
  const policy: WindowPolicy = settings.windowPolicy?.[p.provider] ?? 'both';
  const models = applyCodexModelFilter(codexModelsFromQuota(p), settings.models?.[p.provider]);
  return { models, policy };
}
```

(Se a leitura adicional dos formatters revelar outra business logic dependente de
settings — ex.: outros providers — adicionar resolvers análogos no mesmo módulo.
Hoje só o Codex carrega settings nos builders.)

- [ ] **Step 2: `waybar.ts` — manter o cache de 5s envolvendo o resolve**

`waybar.ts` tem hoje `loadSettingsCached()` (TTL 5s) no hot path. Mover essa função
para envolver `resolveCodexViewModel` — o Waybar não pode reler settings a cada poll.
Concretamente: `waybar.ts` mantém um cache de 5s do resultado de
`resolveCodexViewModel` (ou do `loadSettingsSync` subjacente). Terminal e TUI chamam
`resolveCodexViewModel` direto.

- [ ] **Step 3: Atualizar os 3 formatters para usar o resolver**

Em `buildCodex` de cada um dos 3 formatters, substituir o load inline de settings +
`codexModelsFromQuota` + `applyCodexModelFilter` + leitura de `windowPolicy` pela
chamada ao resolver. Os builders deixam de importar `loadSettingsSync`/`settings`.

- [ ] **Step 4: Verificar**

Run: `bun test && bun run typecheck && bun run lint`
Expected: PASS. Snapshots inalterados — a resolução é a mesma, só mudou de lugar.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: isola business logic dos formatters em view-model"
```

---

## Task 3: Modelo `Line` — estender `segments.ts`

**Files:**
- Modify: `src/formatters/segments.ts`

- [ ] **Step 1: Ampliar `ColorToken`**

Os builders usam toda a paleta (`magenta`, `cyan`, `textBright`, `muted`, e cores de
provider). `ColorToken` hoje só tem 6 tokens. Ampliar para a paleta completa usada
pelos formatters. Substituir a definição atual por:

```typescript
/** Theme-neutral color token. Renderers map this to ANSI or Pango hex. */
export type ColorToken =
  | 'green'
  | 'yellow'
  | 'orange'
  | 'red'
  | 'comment'
  | 'text'
  | 'textBright'
  | 'muted'
  | 'magenta'
  | 'cyan'
  | 'blue'
  | 'brightBlue';
```

`STATUS_TO_COLOR` e `colorForDisplay` continuam usando apenas os 4 tokens de saúde —
inalterados.

- [ ] **Step 2: Adicionar o tipo `Line`**

Uma linha é uma sequência de `Segment`s (o mesmo `Segment` já existente: `text`,
`color`, `bold?`). Adicionar ao fim de `segments.ts`:

```typescript
/** A single rendered line: an ordered list of colored text segments. */
export type Line = Segment[];
```

Os builders compõem uma `Line` concatenando segments de texto com os segments de
`barSegments(...)` / `indicatorSegments(...)`.

- [ ] **Step 3: Verificar**

Run: `bun test && bun run typecheck && bun run lint`
Expected: PASS. Nenhuma mudança de comportamento — só ampliação de tipo.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor: amplia ColorToken e adiciona tipo Line"
```

---

## Task 4: Pipeline + migração do Claude

Constrói os 3 renderers e o primeiro builder (Claude), e liga o Claude ao novo
pipeline nos 3 entry points. Os outros 3 providers continuam no código antigo —
build verde, snapshots verdes.

**Files:**
- Create: `src/formatters/render-ansi.ts`, `src/formatters/render-pango.ts`, `src/tui/render-colorize.ts`, `src/formatters/builders/claude.ts`
- Modify: `src/formatters/terminal.ts`, `src/formatters/waybar.ts`, `src/tui/list-all.ts`

- [ ] **Step 1: Criar os 3 renderers**

Cada renderer recebe `Line[]` e devolve `string` (linhas unidas por `\n`).
- `render-ansi.ts` — mapeia cada `ColorToken` para o código ANSI de `theme.ts`
  (`ANSI`), emite `cor + bold? + text` por segment e um `ANSI.reset` ao fim de cada
  linha. Equivale ao `renderAnsi` atual de `terminal.ts`, generalizado para a paleta
  completa.
- `render-pango.ts` — mapeia cada `ColorToken` para o hex de `ONE_DARK`, emite um
  `<span foreground="#hex">...</span>` por segment (com `weight="bold"` quando
  `bold`), aplicando escape XML ao `text`. **Deve reproduzir exatamente** o formato
  de span que o `waybar.ts` atual produz (ver Risco).
- `render-colorize.ts` — usa `colorize` de `src/tui/colors.ts` para cada segment.

A correção dos renderers é validada indiretamente pelos snapshots na Task 4 Step 4.

- [ ] **Step 2: Criar `src/formatters/builders/claude.ts`**

Extrair a lógica de `buildClaude` (hoje em `terminal.ts:95-151`, e as variantes em
`waybar.ts` e `tui/list-all.ts`) para uma função pura
`buildClaude(quota, options): Line[]`. A função:
- não lê settings, não faz I/O, não emite markup;
- recebe `options` (ver abaixo) para as diferenças entre superfícies;
- emite `Line[]` compondo segments (texto + `barSegments`/`indicatorSegments`).

`options` carrega as diferenças de conteúdo entre as 3 saídas:
```typescript
export interface BuildOptions {
  mode: DisplayMode;
  /** Waybar inclui cabeçalho/rodapé com selo `cached · Xm ago`. */
  footer?: { fetchedAt?: string };
}
```
(Ajustar os campos de `BuildOptions` conforme as diferenças reais observadas ao ler
os 3 builders de Claude. O tipo vive em `segments.ts` ou num `builders/types.ts`.)

O builder de Claude deve reproduzir o conteúdo das 3 saídas atuais — qualquer
diferença entre terminal/waybar/tui vira um campo de `options`.

- [ ] **Step 3: Ligar o Claude ao novo pipeline nos 3 entry points**

Em `terminal.ts`, `waybar.ts`, `tui/list-all.ts`: para o provider `claude`, chamar
`buildClaude(quota, options)` e renderizar com o renderer da superfície
(`renderAnsi` / `renderPango` / `renderColorize`). Os outros 3 providers continuam
chamando os builders antigos. Os entry points ainda funcionam (dispatch misto).

- [ ] **Step 4: Verificar — o passo crítico**

Run: `bun test`
Expected: **todos os snapshots verdes**. O snapshot de terminal valida texto/layout
do Claude; o de Waybar valida o Pango do Claude byte-a-byte. Se o snapshot de Waybar
falhar, o `render-pango.ts` não reproduziu o markup atual — iterar no renderer até
bater (ver Risco; NÃO regenerar o snapshot de Waybar sem antes esgotar o ajuste).
Depois: `bun run typecheck && bun run lint`.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor: pipeline de builders + migra Claude"
```

---

## Task 5: Migrar o Codex

Os renderers já existem (Task 4). Esta task só adiciona o builder do Codex e troca
o dispatch.

**Files:**
- Create: `src/formatters/builders/codex.ts`
- Modify: `src/formatters/terminal.ts`, `src/formatters/waybar.ts`, `src/tui/list-all.ts`

- [ ] **Step 1: Criar `src/formatters/builders/codex.ts`**

Extrair `buildCodex` para `buildCodex(quota, viewModel, options): Line[]` — puro,
emite `Line[]`. Recebe o `CodexViewModel` (de `resolveCodexViewModel`, Task 2) já
com `models` filtrados e `policy` — o builder não toca settings. Reproduz o conteúdo
das 3 saídas atuais de Codex; diferenças entre superfícies vão para `options`.

- [ ] **Step 2: Ligar o Codex ao pipeline nos 3 entry points**

Substituir o dispatch de `codex` nos 3 entry points pela chamada
`buildCodex(quota, resolveCodexViewModel(quota), options)` + renderer da superfície.
O Waybar usa o resolve cacheado (Task 2 Step 2).

- [ ] **Step 3: Verificar**

Run: `bun test && bun run typecheck && bun run lint`
Expected: snapshots verdes (terminal texto/layout; Waybar Pango byte-exato).

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor: migra Codex para o pipeline de builders"
```

---

## Task 6: Migrar o Amp

**Files:**
- Create: `src/formatters/builders/amp.ts`
- Modify: `src/formatters/terminal.ts`, `src/formatters/waybar.ts`, `src/tui/list-all.ts`

- [ ] **Step 1: Criar `src/formatters/builders/amp.ts`**

Extrair `buildAmp` para `buildAmp(quota, options): Line[]` — puro. Atenção à
diferença conhecida: o terminal emite sub-linhas de "Free Tier" com conectores de
árvore (`├─`/`└─`); o Waybar inlina o ETA. Isso vira um campo de `options`
(ex.: `ampSubLines?: boolean`), não dois caminhos de código.

- [ ] **Step 2: Ligar o Amp ao pipeline nos 3 entry points**

- [ ] **Step 3: Verificar**

Run: `bun test && bun run typecheck && bun run lint`
Expected: snapshots verdes.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor: migra Amp para o pipeline de builders"
```

---

## Task 7: Migrar o Copilot

**Files:**
- Create: `src/formatters/builders/copilot.ts`
- Modify: `src/formatters/terminal.ts`, `src/formatters/waybar.ts`, `src/tui/list-all.ts`

- [ ] **Step 1: Criar `src/formatters/builders/copilot.ts`**

Extrair `buildCopilot` para `buildCopilot(quota, options): Line[]` — puro. Os
helpers específicos do Copilot em `terminal.ts` (`copilotUsedPercent`,
`copilotDisplayValue`, `copilotSnapshotDetail`, `formatCount`, `formatRawPercent`,
`boundedPercent`) que forem lógica de dados (não markup) movem para o builder ou
para `codex-helpers.ts`/um `copilot-helpers.ts`; a parte de markup é expressa via
`Segment`s.

- [ ] **Step 2: Ligar o Copilot ao pipeline nos 3 entry points**

- [ ] **Step 3: Verificar**

Run: `bun test && bun run typecheck && bun run lint`
Expected: snapshots verdes.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor: migra Copilot para o pipeline de builders"
```

---

## Task 8: Colapsar — dispatchers finos + deletar código antigo

Com os 4 providers no novo pipeline, o código antigo de builder nos 3 entry points
está morto.

**Files:**
- Modify: `src/formatters/terminal.ts`, `src/formatters/waybar.ts`, `src/tui/list-all.ts`

- [ ] **Step 1: Remover os builders antigos e helpers órfãos**

Em cada um dos 3 entry points: deletar as 4 funções builder antigas (`buildClaude`/
`buildCodex`/`buildAmp`/`buildCopilot` antigas) e todo helper que ficou órfão
(`modelLine`, `codexModelLine`, `label`, `v`, `bar`, `indicator` antigos, etc. — o
lint aponta os não usados). O entry point fica como dispatcher: itera providers,
escolhe builder + renderer, trata o caso `generic`/sem-provider.

- [ ] **Step 2: `tui/list-all.ts` — usar `segments.ts`**

Confirmar que `tui/list-all.ts` não tem mais cópias próprias de `bar`/`indicator`
(deve usar `barSegments`/`indicatorSegments` via o builder/renderer). Remover
qualquer duplicata remanescente.

- [ ] **Step 3: Verificar**

Run: `bun test && bun run typecheck && bun run lint`
Expected: snapshots verdes; `terminal.ts`, `waybar.ts`, `list-all.ts` reduzidos a
dispatchers (alvo da spec: ~80-120 linhas cada). Nenhum import órfão.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor: colapsa formatters em dispatchers finos"
```

---

## Task 9: Verificação final da sub-fase

**Files:** nenhum (verificação).

- [ ] **Step 1: Suíte completa + typecheck + lint**

Run: `bun test && bun run typecheck && bun run lint`
Expected: PASS — todos os snapshots verdes (incluindo `formatters-snapshot.test.ts`,
`formatters.test.ts`, `formatters-segments.test.ts`), typecheck e lint limpos.

- [ ] **Step 2: Confirmar a eliminação dos casts e da duplicação**

Run: `grep -rn "as ClaudeQuotaExtra\|as CodexQuotaExtra\|as AmpQuotaExtra\|as CopilotQuotaExtra" src; echo "---"; wc -l src/formatters/terminal.ts src/formatters/waybar.ts src/tui/list-all.ts`
Expected: casts só dentro de `src/providers/extras.ts`; os 3 entry points
encolhidos para dispatchers.

- [ ] **Step 3: Confirmar histórico**

Run: `git log --oneline -9`
Expected: os 8 commits da sub-fase acima do commit do spec.

---

## Self-Review (preenchido pelo autor do plano)

**Cobertura do spec:**
- `extras.ts` getters / eliminação dos 13 casts → Task 1 ✓
- Business logic fora dos formatters (`view-model.ts`) → Task 2 ✓
- Modelo `Line`/`Segment` (estende `segments.ts`) → Task 3 ✓
- Builders puros por provider → Tasks 4-7 ✓
- 3 renderers (ANSI/Pango/colorize) → Task 4 ✓
- `tui/list-all.ts` passa a usar `segments.ts` → Task 8 Step 2 ✓
- Encolher `waybar.ts`/`terminal.ts`/`list-all.ts` → Task 8 ✓
- `configure-models.ts` usa `getCodexExtra` → Task 1 Step 2 ✓

**Placeholders:** o plano dá código completo para o que é genuinamente novo e
estável (`extras.ts`, `view-model.ts`, ampliação de `ColorToken`/`Line`). Para as
tasks de extração (4-8), o "código completo" de um refactor protegido por snapshot
é: a assinatura-alvo, o que move para onde, e o oráculo (snapshot byte/texto). O
implementador lê o builder atual e o snapshot define a correção. Isto é deliberado
e adequado a um refactor mecânico desta escala — não é placeholder.

**Consistência de tipos:** `Line`, `Segment`, `ColorToken`, `BuildOptions`,
`CodexViewModel`, os getters `getXxxExtra` e a assinatura `build(quota, ...): Line[]`
são usados de forma consistente entre as tasks.

## Risco conhecido

**Byte-exatidão do Pango (Waybar) — o risco central.** O snapshot de Waybar captura
o markup Pango inteiro; o `render-pango.ts` precisa reproduzi-lo byte-a-byte. Se,
ao migrar um provider, o snapshot de Waybar falhar por uma diferença **puramente de
markup** (ex.: spans adjacentes mesclados vs separados, ordem de atributos) que seja
visualmente equivalente, o implementador deve: (1) primeiro tentar ajustar
`render-pango.ts` para casar exatamente; (2) só se a paridade byte-exata exigir
contorção desproporcional, regenerar o snapshot de Waybar **como mudança intencional
de markup visualmente equivalente**, documentando no commit e sinalizando no report
para revisão explícita. O snapshot de terminal é tolerante (ANSI removido pelo
`sanitize()`), então o terminal não corre esse risco.

**Divergência do TUI.** Se `tui/list-all.ts` `buildClaude` de fato hardcoda modelos
(Opus/Sonnet/Haiku), a unificação converge o TUI para o comportamento dinâmico e o
snapshot do TUI muda de propósito — regenerar e documentar no commit da Task 4.
Confirmar lendo o código antes.
