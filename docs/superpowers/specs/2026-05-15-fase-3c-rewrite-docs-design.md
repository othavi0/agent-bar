# Fase 3c — Rewrite de AGENTS.md + CLAUDE.md

**Data:** 2026-05-15
**Status:** aprovado, pronto para planejamento
**Projeto:** agent-bar — mutirão de limpeza (Fase 3, sub-fase 3c)

## Contexto

As Fases 1, 2, 3a, 3a-bis e 3b estão concluídas e publicadas. A Fase 3 foi
decomposta em quatro sub-fases: 3a (formatters), 3a-bis (`BaseProvider`), 3b
(consolidação) e **3c (este spec)** — o rewrite das instruções de agente.

`AGENTS.md` (243 linhas) é o contrato canônico para agentes neste repositório;
`CLAUDE.md` é um shim de compatibilidade de ~13 linhas que delega a ele. As
Fases 1-3b mudaram o código sem atualizar o `AGENTS.md`, acumulando drift
factual.

### Drift identificado

| Drift | Detalhe |
| --- | --- |
| Teste inexistente | `tests/waybar-integration.test.ts` é referenciado 2× (tabela de verificação + testing patterns) — o arquivo não existe |
| Architecture Map incompleto | Omite ~12 módulos: `amp-cli.ts`, `copilot-cli.ts`, `install.ts`, `logger.ts`, `menu.ts`, `setup.ts`, `uninstall.ts`, `remove.ts`, `update.ts`, `providers/base.ts`, `providers/registry.ts`, `providers/extras.ts` |
| Provider Contract desatualizado | Diz "Every provider implements `Provider`" — silencioso sobre `BaseProvider` (Fase 3a-bis): Codex/Copilot/Amp estendem o template-method; Claude é standalone |
| Formatters subdescrito | `src/formatters/` resumido em uma linha — na verdade é um pipeline (builders puros `Line[]` → view-model/segments → renderers ANSI/Pango/colorize). A seção "Formatters and UI Rules" não menciona o pipeline |
| Verification table | Não lista `tests/formatters-segments.test.ts` nem ~9 outros arquivos de teste atuais (a suíte tem 20 arquivos) |

## Objetivo

Reescrever `AGENTS.md` com estrutura nova (organização por propósito) e zero
drift factual contra o código pós-Fases 1-3b, e ressincronizar o fraseado dos
bootstraps do `CLAUDE.md` com o novo `AGENTS.md`. Nenhuma mudança de código.

## Estrutura nova do AGENTS.md

Organização **por propósito** (rules → como trabalhar → modelo mental →
contratos → how-to → ponteiros), 7 seções no lugar das 15 flat atuais:

- **Intro** — um parágrafo: o que é o projeto + AGENTS.md canônico, CLAUDE.md
  delega.
- **1. Hard Rules** — funde "Non-Negotiables" + "Legacy Policy" num bloco
  escaneável: Bun-only; não rodar `bun ./scripts/agent-bar`; não mutar o
  desktop live como verificação; não editar `~/.config/waybar` /
  `~/.config/agent-bar` à mão para testes; não converter shims Bash em
  TypeScript; manter stdout limpo para o JSON do Waybar; preservar mudanças
  não relacionadas do usuário; nomes `qbar` / `agent-bar-omarchy` / Antigravity
  / `llm-usage` e acoplamento Omarchy removidos no `4.0.0` — não reintroduzir.
- **2. How to Work** — Commands (bloco `bun`); Verification (tabela de
  verificação mais estreita, regenerada contra os 20 arquivos de teste reais);
  Code Style (Biome 2 espaços / aspas simples / 120 colunas, ESM, strict mode,
  identificadores em inglês, commits em PT, preferir constantes de identidade);
  Safe Development Workflow (5 passos).
- **3. Architecture Map** — completo e atual, agrupado em: Entry & CLI;
  Lifecycle (`setup`/`update`/`uninstall`/`remove`/`install`/`action-right`);
  Providers (`base`/`registry`/`extras`/`index` + as 4 implementações);
  Formatters; Waybar (`waybar-contract`/`waybar-integration`); TUI; Support
  (`config`/`cache`/`settings`/`logger`/`theme`/`app-identity`).
- **4. Contracts** — Provider Contract (interface `Provider` + `BaseProvider`
  como template-method: `getQuota()` orquestra base/gate/cache/erro, subclasses
  implementam `fetchRaw`/`buildQuota`/`unavailableError`/`toUserFacingError`;
  Claude é standalone por não caber no template); Quota Data Conventions;
  Settings Contract; Cache Contract; Waybar Contract & Integration; Formatters
  Pipeline (seção nova — builders puros emitem `Line[]`, renderers
  `render-ansi`/`render-pango`/`tui/render-colorize` convertem, dispatchers
  `terminal.ts`/`waybar.ts` finos, XML-escape centralizado no `render-pango`);
  Runtime & Owned Paths (tabela de paths + locais de credenciais).
- **5. Adding or Changing a Provider** — checklist atualizado: estender
  `BaseProvider` salvo quando o provider não couber no template (caso Claude).
- **6. Testing Patterns** — `bun:test`; sem credenciais/CLIs/rede reais; temp
  dirs; overrides `XDG_*`; mock de fs/fetch/spawn; regras de snapshot
  (terminal valida texto/layout com ANSI removido, Waybar valida Pango
  byte-exato).
- **7. Pointers** — `README.md`, `CONTRIBUTING.md`, `docs/*.md`; `CHANGELOG.md`
  tratado como histórico.

## Mudanças no CLAUDE.md

Os 4 bullets de bootstrap do `CLAUDE.md` estão factualmente corretos. A revisão
é de **fraseado**: cada bullet deve usar a mesma redação dos itens
correspondentes na seção "Hard Rules" do novo `AGENTS.md`, para que os dois
arquivos não divirjam em wording. O `CLAUDE.md` permanece um shim de ~13 linhas
que delega ao `AGENTS.md`. Os 4 bootstraps mantidos são: Bun-only; não rodar
`bun ./scripts/agent-bar`; não rodar comandos que mutam o ambiente live
(`setup`/`uninstall`/`remove`/`update`) sem pedido explícito; não editar
`~/.config/waybar` / `~/.config/agent-bar` à mão para verificação.

## Restrição de fidelidade factual

Todo fato afirmado no novo `AGENTS.md` — nome de arquivo, nome de teste,
assinatura de contrato, valor default, comando — deve ser verificado contra a
árvore de código atual durante a implementação. Itens que exigem verificação
explícita:

- A lista de módulos do Architecture Map bate com `find src -name '*.ts'`.
- A tabela de verificação só cita arquivos presentes em `tests/`.
- A versão do schema de settings bate com a constante `CURRENT_VERSION` em
  `src/settings.ts` (a redação atual cita "version `2`" — confirmar).
- Os defaults de settings, os IDs de módulo Waybar, os paths owned e os locais
  de credenciais batem com `src/settings.ts`, `src/waybar-contract.ts` e
  `src/config.ts`.
- O contrato de classes Waybar (`ok`/`low`/`warn`/`critical`/`disconnected`,
  `agent-bar-hidden`) bate com `src/formatters/waybar.ts` e `src/config.ts`.

## Plano de verificação

- Não há teste automatizado para markdown. A verificação é por revisão cruzada
  do `AGENTS.md` final contra `src/`/`tests/`.
- `git diff --check` — sem whitespace errors nos dois arquivos.
- `bun test && bun run typecheck && bun run lint` — devem continuar verdes
  (nenhum código muda; é uma sanidade de que o rewrite não tocou em código por
  engano).
- Revisão final cruzando cada fato do `AGENTS.md` com o código.

## Fora de escopo

- Qualquer mudança de código de produção ou de teste.
- Os arquivos em `docs/*.md` (`commands.md`, `runtime.md`, etc.) — o `AGENTS.md`
  apenas aponta para eles; o conteúdo deles não é reescrito aqui.
- O `CLAUDE.md` global do usuário (`~/.claude/CLAUDE.md`) — fora do repositório.

## Risco conhecido

- Rewrite estrutural pode perder uma regra sutil que estava capturada no texto
  antigo. Mitigação: a implementação parte do `AGENTS.md` atual como checklist
  de cobertura — cada regra/contrato existente deve ter destino no novo
  documento (movido ou conscientemente removido por estar obsoleto), e a
  revisão final confere essa cobertura além da fidelidade factual.
