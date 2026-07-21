# Design: Omarchy-first — redesign do popup, contrato windowKind e Waybar como legado (v9.0.0)

Data: 2026-07-21 · Status: aprovado pelo dono (brainstorming com mockups no visual companion)

## Contexto

Auditoria profunda de 2026-07-21 (21 agentes, read-only) confirmou contra o payload
real da máquina do dono:

1. **Codex duplicado**: no plano Plus, `primary` e `secondary` chegam ambos com
   `windowMinutes: 10080` e mesmo `resetsAt` → o popup mostra 2 linhas "Weekly"
   idênticas + uma 3ª igual em models. Agravante: `normalize.rs:88-98` força a
   janela primária no slot `five_hour` mesmo quando `classify_window` a rejeitou
   (teste `unrecognized_window_uses_fallback_mapping` documenta).
2. **Sem countdown**: `format_eta` existe (`formatters/shared.rs:188`) e é usado
   no tooltip Waybar, mas popup QML e TUI nunca o chamam. A TUI ainda mostra o
   reset em **UTC cru** (`tui/render/detail/format.rs:69` fatia o ISO sem
   `to_offset`).
3. **`extra` invisível**: o Widget.qml nunca lê o campo `extra` — créditos do
   Amp ($), sessões/turnos/contexto do Grok, extra usage do Claude morrem antes
   da tela.
4. **Ações como texto**: "right-click: settings · middle: refresh" é `Text` sem
   clique; "Abrir menu (TUI)" é link sublinhado. Largura fixa `Style.space(370)`.
5. **Heurísticas triplicadas**: severidade e rótulo de janela reimplementados em
   Rust, QML (`windowLabel` com magic numbers ±90/±1440) e TUI; clamp do refresh
   em 5 lugares.
6. **3 caminhos escrevem `~/.config/waybar/` sem gate** (Save da TUI Config,
   `update` ManagedGit, `update` Standalone) num desktop 100% Omarchy.
7. **`update` não atualiza o plugin QML** — binário novo + widget velho até um
   `setup` manual (só um hint impresso).

## Relação com decisões anteriores (transparência)

- **ADR-0001 (right-click = settings) é MANTIDO** — o redesign muda conteúdo e
  forma do popup, não o roteamento dos cliques.
- **Reverte conscientemente** duas decisões da spec 8.5.0 (mesmo dia): o rodapé
  de dicas em texto vira botões reais, e o atalho `s` de salvar morre (salvar só
  pelo botão). Pedido explícito do dono em 2026-07-21.
- **Expande o escopo v1 do plugin** (que excluía "settings UI própria além do
  manifest" e dados ricos no popup) — expansão consciente, não bug-fix.

## Decisões travadas (aprovadas via mockups, 2026-07-21)

| Tema | Decisão |
| --- | --- |
| Estrutura do popup | B1: hero % por provider (mesmo número do chip), ações no topo à direita |
| Pele | Painéis: um cartão por provider (`#1f2530`-like sobre fundo mais escuro, radius 9) — cores reais vêm do tema do shell |
| Título | `agent-bar` à esquerda + "há Xm" relativo; botões ↻ ⚙︎ ❯ à direita |
| Ícones de ação | Unicode puro (↻ U+21BB, ⚙︎ U+2699+VS15, ❯ U+276F) — sem dependência de Nerd Font |
| Largura | `Style.space(540)` (antes 370), igual nos dois modos |
| Simetria | Grade de colunas fixas `rótulo 82 · barra flex · % 44 · reset 132` (proporções; valores exatos calibrados no shell) — toda barra idêntica, % em coluna tabular |
| Countdown | `1h 46m · 18:30` / `7d 0h · seg 16:43`, sempre na mesma coluna; Grok (sem reset) usa a coluna para contexto `253.9k/500k tok` |
| Linha por modelo | Só quando divergir da janela compartilhada (mata o triplo Weekly do Codex); mesma barra, rótulo indentado |
| `extra` na tela | Amp: `Créditos $X · replenish`; Grok: `Hoje: N sessões · N turnos · modelo`; Claude: extra usage quando houver |
| staleReason | Linha de texto discreta + opacity (hoje é só opacity) |
| displayMode used | Passa a valer nas linhas do popup, não só no chip |
| Settings | Painéis Providers (toggle + ↑↓) / Exibição (segmentado + **prévia ao vivo da barra**) / Alertas & atualização; `← Uso` e `Salvar` no topo à direita |
| Salvar | **Só pelo botão**. Sem atalho `s` (remove o path de save do PanelKeyCatcher); esc/clique-fora só fecham, sem dica no rodapé |
| Motion | M1 barras preenchem na abertura (≈320ms ease-out, stagger 60ms) + M2 ↻ gira durante fetch + M4 hover 160ms. **Sem pulso** (coerente com decisão do TUI v8). Gate por `menu.animations` |
| Versão | **9.0.0** — marco de produto (Omarchy-first, Waybar vira tier legado) |

## A. Contrato de dados (backend primeiro — destrava tudo)

- **`windowKind`** novo em `QuotaWindow`: `"fiveHour" | "sevenDay" | "daily" |
  "context" | "other"`, decidido uma única vez no Rust. Cada provider seta na
  origem (Claude 5h/7d; Codex via `classify_window` SEM o fallback forçado;
  Amp `daily`; Grok `context`). Rótulos de UI derivam só disso.
- **Fix do mislabeling Codex**: `build_model_windows` deixa de forçar
  `primary→five_hour`/`secondary→seven_day` quando a classificação diverge —
  janela fora de tolerância vira `other` com `windowKind: "other"` e rótulo pela
  duração real (ex.: "1h window").
- **Dedup é display-level**: o JSON continua emitindo primary/secondary/models
  (contrato intacto, mudança 100% aditiva, `schemaVersion` permanece 1);
  QML e TUI colapsam janelas com mesmo `(windowKind, resetsAt, remaining)`.
- **QML deixa de ter heurísticas**: `windowLabel` (magic numbers) morre;
  severidade continua espelhada (severity da API + threshold), mas rótulo vem
  pronto. Countdown e "há Xm" calculados no QML a partir de `resetsAt`/
  `fetchedAt` (JS `Date`, fuso local automático).
- **`extra` documentado por provider** em `docs/json-output.md` (shapes de
  `AmpQuotaExtra.meta`, `GrokQuotaExtra`, `ClaudeQuotaExtra.extraUsage`,
  `CodexQuotaExtra.modelsDetailed`) — continua `unstable`, mas com shape
  descrito para o widget first-party consumir.

## B. Widget.qml — Usage popup

- Titlebar: `agent-bar` + fetched-ago relativo + iconbtns ↻ ⚙︎ ❯ (Refresh /
  Settings mode / `openTui()` via terminal helper). Tooltips no hover
  (`bar.showTooltip`, como os chips já fazem).
- Um `Panel` por provider: header (ícone real, nome, plan/account elidido,
  hero % à direita = mesmo valor e severidade do chip) + grade de janelas
  (colunas fixas) + linha(s) de info do `extra`.
- Estados: unavailable → mensagem de erro no cartão (comportamento atual);
  stale → linha de motivo + opacity; parse falho → mantém último payload bom
  (contrato atual do `applyPayload`).
- Motion: M1/M2/M4 com `Behavior`/`NumberAnimation` QML; desliga quando
  `menu.animations = false`. Requisito de contrato: `config show` passa a expor
  `menu.animations` (read-only para o widget; edição continua via settings.json).

## C. Widget.qml — Settings mode

- Mantém dual-write do ADR-0002 (`config apply` → `updateEntryInline`).
- Painéis: Providers (Toggle + ↑↓ por linha, mín. 1 ativo), Exibição
  (segmentado Restante/Usado + prévia da barra com chips e valores reais
  trocando ao vivo), Alertas & atualização (notify toggle, interval −/+ com
  clamp 30–3600 — clamp centralizado numa função única no QML).
- Header: `agent-bar · Settings`, botões `← Uso` e `Salvar` (busy → "Salvando…").
- Remoções: rodapé de dicas; save via tecla `s` (o `PanelKeyCatcher` fica só
  para esc-fecha, se necessário).

## D. TUI — paridade de dados (este spec) + redesign visual (sub-projeto)

Neste spec (independem de visual):
- `fmt_reset` passa a converter para fuso local (usar `format_reset_time`
  existente com `Clock.local_offset`).
- Countdown `→ 1h 46m` nas janelas do Detail (via `format_eta` existente).
- Rótulos de janela via `windowKind` + dedup display-level (some o duplo Weekly
  também na TUI).
- Config platform-aware: Separators/Signal/Interval-waybar ocultos quando
  Omarchy-only (mesma detecção da seção E).

**Sub-projeto seguinte (fora deste spec):** redesign visual da TUI — decisão do
dono em 2026-07-21. Terá rodada própria de mockups e spec próprio; nada aqui
pressupõe o resultado dele.

## E. Waybar → tier legado isolado

- Módulo `src/waybar/` agrupando `waybar_contract.rs`, `waybar_integration.rs`,
  `formatters/waybar.rs` e `render_pango.rs` (re-exports para não quebrar a
  matriz de testes do CLAUDE.md; filtros `cargo test waybar_contract` etc.
  continuam funcionando).
- **`platform::detect()` único** (Omarchy presente? Waybar presente?) usado por:
  `setup` (já faz), `update` ManagedGit (hoje hardcoda `omarchy: None,
  skip_waybar: false`), `update` Standalone (hoje sempre recopia assets) e
  **Save da TUI Config** (hoje incondicional — pode criar `~/.config/waybar/`
  do zero). Zero escrita em `~/.config/waybar/` sem Waybar no PATH.
- Docs declaram Waybar em manutenção (tier legado): funciona, recebe fix, não
  recebe feature nova. Remoção futura = deletar `src/waybar/` + gates + docs.

## F. Omarchy first-class

- `agent-bar update` (ambos os ramos) **reinstala o plugin** quando
  `detect_omarchy_shell()` — mata o drift binário↔QML (risco nº 1 da auditoria).
  O hint pós-update vira fallback para quando a detecção falhar.
- `doctor` ganha checagens Omarchy: versão do manifest instalado vs binário;
  entrada `agent-bar.usage` no shell.json sem diretório (ou vice-versa).
- `uninstall`: só remove o diretório do plugin se os comandos `omarchy ...
  remove` tiverem funcionado (ou avisar que ficou referência no shell.json).

## G. Limpeza de legado (verificada adversarialmente, aprovada)

Remover: variante `Command::Terminal` (+ ajuste do teste sintético e do match
em main.rs), `waybar_contract::get_all_provider_ids`, `install::ensure_amp_cli`
(+ comentário de módulo), `amp_cli::AMP_INSTALL_COMMAND` duplicada (+ teste
aponta pra `app_identity`), dep `tokio-util`. Higiene: `waybar.show_percentage`
(migração settings v2→v3 que dropa a chave), `ConfigField::settings_key()`,
7 variantes órfãs de `Icon` (+ teste), comentário morto em `render/shared.rs`.
Housekeeping: `git worktree remove .worktrees/omarchy-settings-cli` + branch
local/remota (mergeada). NÃO remover (refutados): `watch.rs`, `doctor.rs`,
`notify.rs`, `pricing.rs`, `extras.rs`, `install.rs`, subcomandos CLI, specs
de 2026-07-17.

## H. Docs

- `README.md`: tagline e requisitos Omarchy-first ("Waybar suportada, tier
  legado"); `CONTRIBUTING.md`: MSRV 1.88; `docs/waybar-contract.md`: +grok e
  banner de tier legado; `docs/json-output.md`: seção Future removida, `extra`
  shapes e `windowKind` documentados; `docs/troubleshooting.md` +
  `docs/runtime.md`: seções Grok; `docs/architecture.md`: reescrito com
  omarchy-shell no centro (Widget.qml, `--format json`, `config show/apply`
  no diagrama); CLAUDE.md: linha `providers::grok_cli` na matriz.

## Testes e verificação

- Rust: unit para `windowKind` por provider (incl. regressão do fallback Codex
  com janela de 60min → `other`), golden para formatos `Xh Ym · HH:MM`, dedup,
  fuso local na TUI (Clock mockado), snapshots insta atualizados de propósito.
- Gates de plataforma: testes de `update`/`setup`/TUI-save com dirs temporários
  provando zero escrita em waybar-dir quando Omarchy-only.
- QML (sem harness): validação ao vivo no desktop do dono com as 3 provas —
  funcional (fluxos clique/refresh/settings), perceptual (screenshot vs mockup
  aprovado), dados (payload real conferido contra o renderizado).

## Entrega

PRs em sequência sobre `master`:
1. `chore`: limpeza de legado (G) + worktree.
2. `feat`: contrato de dados `windowKind` + fixes normalize/fuso/eta (A + D-dados).
3. `feat`: Widget.qml novo — usage + settings + motion (B + C).
4. `feat`: gates de plataforma + módulo `src/waybar/` + update-reinstala-plugin +
   doctor (E + F).
5. `docs`: H + CHANGELOG.
Release **9.0.0** ao final (runbook `docs/releasing.md`; AUR automatizado no CI).

## Não-objetivos

- Custo $/tokens/histórico no popup (exige expor `src/usage/*` no envelope —
  candidato a fase 2, decisão adiada de propósito).
- Mudar roteamento de cliques (ADR-0001 mantido) ou bump de `schemaVersion`.
- Drag-and-drop na ordenação de providers.
- Remover Waybar (só isolar; remoção é decisão futura).
- Redesign visual da TUI (sub-projeto próprio com mockups, na sequência).
