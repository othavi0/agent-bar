# Fase 3a — Refatoração dos formatters

**Data:** 2026-05-15
**Status:** aprovado, pronto para planejamento
**Projeto:** agent-bar — mutirão de limpeza (Fase 3, sub-fase 3a)

## Contexto

As Fases 1 (correção/endurecimento) e 2 (remoção total do legacy) estão concluídas
e publicadas. A Fase 3 foi decomposta em quatro sub-fases:

- **3a (este spec):** refatoração da camada de formatação.
- **3a-bis:** extração de um `BaseProvider`.
- **3b:** cobertura de testes + limpezas finais de backlog.
- **3c:** rewrite de `AGENTS.md` e `CLAUDE.md`.

A camada de formatação tem três implementações paralelas — `formatters/terminal.ts`
(ANSI, 479 linhas), `formatters/waybar.ts` (Pango, 601 linhas) e `tui/list-all.ts`
(colorize, 356 linhas) — com quatro builders por provider cada (~750 linhas de
builders + ~240 de helpers). A estrutura é 30-40% duplicada: mesma lógica de negócio
e visual, apenas o markup difere. Além disso:

- 13 casts `as XxxQuotaExtra` espalhados (`terminal.ts` 4, `waybar.ts` 5,
  `tui/list-all.ts` 3, `tui/configure-models.ts` 1).
- Business logic (load de settings, window policy, filtro de modelos Codex)
  duplicada 3-4× **dentro** dos formatters.
- `tui/list-all.ts` reduplica `barSegments`/`indicatorSegments` em vez de importar
  de `formatters/segments.ts`.

## Objetivo

Eliminar a tripla duplicação dos builders via um pipeline funcional de
**segment-emitter**, sem alterar a saída observável (exceto uma divergência
conhecida, descrita abaixo).

## Arquitetura — pipeline em 3 estágios

```
quota + settings  →  [resolve]  →  view model  →  [build]  →  Line[]  →  [render]  →  string
```

### Estágio 1 — Resolve (business logic isolada)

Novo módulo `src/formatters/view-model.ts`. Carrega settings, aplica window policy
e filtro de modelos Codex **uma única vez**, e produz a estrutura de dados que os
builders consomem. Os builders deixam de chamar `loadSettings*` e
`applyCodexModelFilter`.

- O entry point do Waybar mantém o cache de 5s (a função `loadSettingsCached` hoje
  em `waybar.ts`) envolvendo esse resolve — o hot path do Waybar não pode reler
  settings a cada poll.
- Terminal e TUI fazem o resolve com load direto de settings.

### Estágio 2 — Build (um builder puro por provider)

`src/formatters/builders/{claude,codex,amp,copilot}.ts`. Cada um exporta
`build(quota, options): Line[]`. **Puro**: sem I/O, sem leitura de settings, sem
markup. Emite uma estrutura abstrata.

- `Line` = array de `Segment` tipados.
- `Segment` cobre: texto simples, texto bold, valor colorido (com `ColorToken`),
  `bar`, `indicator`, e conectores box-drawing.
- O modelo `Line`/`Segment` estende o `formatters/segments.ts` atual (que já tem
  `Segment`, `ColorToken`, `barSegments`, `indicatorSegments`).

### Estágio 3 — Render (três renderers finos)

Cada renderer transforma `Line[]` em string:

- `renderAnsi` (`src/formatters/render-ansi.ts`) — ANSI true-color.
- `renderPango` (`src/formatters/render-pango.ts`) — Pango markup, com escape XML
  do conteúdo dinâmico.
- `renderColorize` — usa `colorize` de `src/tui/colors.ts`; fica no diretório
  `src/tui/` (perto do seu dependente de cor).

## Diferenças de conteúdo entre as três saídas

As três saídas hoje **não** são idênticas em conteúdo:

- O Waybar tem cabeçalho/rodapé com selo `cached · Xm ago` (`fetchedAt`); terminal
  e TUI não.
- O Amp no terminal tem sub-linhas de "Free Tier" com conectores de árvore; o
  Waybar inlina o ETA.

Essas diferenças viram **opções explícitas** — um parâmetro `BuildOptions` passado
ao builder (ex.: `{ mode, footerStamp?, ampSubLines? }`). Não há três caminhos de
código; há um builder parametrizado. Cada entry point passa as opções da sua
superfície.

## Divergência conhecida a resolver

`tui/list-all.ts` `buildClaude` aparenta usar uma lista de modelos hardcoded
(Opus/Sonnet/Haiku) enquanto `terminal.ts`/`waybar.ts` derivam os modelos
dinamicamente da quota. Ao unificar para um builder único, o TUI converge para o
comportamento dinâmico canônico (o correto).

- **Consequência:** o snapshot do TUI muda **de propósito**. Durante a
  implementação, a divergência deve ser confirmada lendo o código; se confirmada,
  o snapshot do TUI é regenerado intencionalmente e a mudança documentada no
  commit. Se NÃO se confirmar (o TUI já é dinâmico), nenhum snapshot muda.
- Os snapshots de terminal e Waybar devem permanecer **byte-idênticos**.

## Eliminação dos casts

Novo módulo `src/providers/extras.ts` com getters de type-narrowing:
`getClaudeExtra(q)`, `getCodexExtra(q)`, `getAmpExtra(q)`, `getCopilotExtra(q)` —
cada um retorna o `extra` tipado do provider ou `undefined`. Substitui os 13 casts
`as XxxQuotaExtra` em `terminal.ts`, `waybar.ts`, `tui/list-all.ts` e
`tui/configure-models.ts`.

## Estrutura de arquivos

| Arquivo | Ação |
| --- | --- |
| `src/providers/extras.ts` | **Criar** — getters de type-narrowing |
| `src/formatters/segments.ts` | **Estender** — modelo `Line` + tipos de `Segment` ricos |
| `src/formatters/view-model.ts` | **Criar** — resolve de settings/policy/filtro |
| `src/formatters/builders/claude.ts` | **Criar** — builder puro do Claude |
| `src/formatters/builders/codex.ts` | **Criar** — builder puro do Codex |
| `src/formatters/builders/amp.ts` | **Criar** — builder puro do Amp |
| `src/formatters/builders/copilot.ts` | **Criar** — builder puro do Copilot |
| `src/formatters/render-ansi.ts` | **Criar** — renderer ANSI |
| `src/formatters/render-pango.ts` | **Criar** — renderer Pango + escape XML |
| `src/formatters/terminal.ts` | **Encolher** — dispatcher fino |
| `src/formatters/waybar.ts` | **Encolher** — dispatcher fino |
| `src/tui/list-all.ts` | **Encolher** — dispatcher fino + renderer colorize |
| `src/tui/configure-models.ts` | **Editar** — passa a usar `getCodexExtra` |

Cada builder e cada renderer é um arquivo focado, testável isoladamente. Os entry
points (`terminal.ts`, `waybar.ts`, `list-all.ts`) ficam como dispatchers: resolvem
o view model, escolhem o builder do provider, renderizam com o renderer da
superfície.

## Plano de verificação

- **Rede de segurança:** `tests/formatters-snapshot.test.ts` — a saída de terminal
  e Waybar deve permanecer byte-idêntica. Nenhum `--update-snapshots` para esses
  dois. O único snapshot que pode mudar é o do TUI (divergência de modelos), e só
  se a divergência for confirmada — nesse caso, regenerado de propósito.
- `tests/formatters.test.ts` e `tests/formatters-segments.test.ts` continuam verdes.
- `bun run typecheck` — zero cast `as XxxQuotaExtra` restante nos formatters/TUI.
- `bun run lint` — Biome limpo.
- Antes do handoff: `bun test && bun run typecheck && bun run lint`.

## Fora de escopo

- `BaseProvider` e a unificação da camada de providers — Fase 3a-bis.
- Cobertura de testes nova para os módulos criados — Fase 3b (esta fase confia nos
  snapshots existentes para garantir não-regressão; testes unitários dos novos
  builders/renderers vêm na 3b).
- Itens de backlog (`APP_WINDOW_TITLE` morto, conflito `runInteractive`+spinner,
  cast sem guard no `codex.ts`) — Fase 3b.
- Rewrite de docs — Fase 3c.

## Risco conhecido

A unificação é ampla e toca os três formatters. A mitigação central são os
snapshots byte-exatos de terminal/Waybar. O risco maior é o builder parametrizado
não reproduzir exatamente uma das saídas — detectado imediatamente pelo snapshot.
A refatoração deve ser feita em passos pequenos (provider a provider, ou estágio a
estágio), com a suíte de snapshot verde a cada commit.
