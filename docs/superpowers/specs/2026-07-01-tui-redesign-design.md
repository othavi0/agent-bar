# Redesign da TUI (`agent-bar menu`) — Design

Data: 2026-07-01 · Status: aprovado no brainstorming (mockups validados no
visual companion; sessão em `.superpowers/brainstorm/`, gitignored)

## 1. Contexto e problema

A TUI atual (pós-rewrite Rust) falha nas quatro frentes reportadas pelo
usuário, todas com causa raiz identificada em auditoria:

| Queixa | Causa raiz |
| --- | --- |
| UI congela / parece quebrada | `fetch_all().await` e `RealLogin::launch` rodam **inline no branch do `tokio::select!`** (`event_loop.rs:173-236`, `:114-119`) — nenhum `terminal.draw()` roda por até ~21-30s no boot e a cada 60s; o throbber é código morto (Loading é setado e limpo na mesma iteração). |
| Gráficos não funcionam | O sparkline "tokens/h" do detail é **string literal hardcoded** (`detail.rs:209-218`, comentário "placeholder (T10)" — copiado do mockup do spec de 2026-06-19 e nunca implementado). Amp nunca gera `UsageRecord` (só `amp_dollars`), então não existe no History. Bucketing é diário: 7 pontos esticados na largura da tela. |
| Logins/agentes contraditórios | A aba Login usa checagem fraca (`login.rs:19-29`: arquivo existe / binário no PATH) enquanto o dashboard exige token OAuth válido (`claude.rs:181-186`) — Amp `[ok]` significa só "CLI instalada". Pós-login não há refetch (`update.rs:476-482`). |
| % de uso sem graça | `block_bar()` é preenchimento binário `█/░` de cor única (`quota_gauge.rs:16-22`); nenhum widget nativo do ratatui (Gauge/Sparkline/Chart) é usado; 60-80% da área vertical fica vazia em todas as views; sem reflow em terminal largo. |

Além disso, o provider Claude parseia só o formato **legado** da API OAuth
(`seven_day_opus/sonnet/cowork` — hoje sempre `null`) e ignora os blocos
novos `limits[]` e `spend`, que são o que o `/usage` do Claude Code exibe
(incl. limite semanal por-modelo com `display_name` e extra usage).

## 2. Objetivos

1. TUI nunca congela: todo IO fora do event loop, com feedback visível.
2. Todo elemento visual liga em dado real (princípio "dado real ou nada").
3. Cobertura completa do `/usage` do Claude (sessão, semana, por-modelo,
   extra usage/créditos, severidade oficial).
4. Nova IA: sidebar única sem tabs, mouse completo, denso estilo btop.
5. Sistema visual One Dark evoluído com contrato de alinhamento e motion
   comunicando estado.

## Não-objetivos

- Não altera o contrato Waybar (módulos, CSS, JSON de saída, tooltips
  Pango) — só a TUI do `menu`.
- Não adota framework de componentes (tuirealm): a arquitetura Elm/MVU
  atual (state/action/update puros + event_loop com IO) permanece.
- Não reintroduz nada do legado morto (qbar etc. — CLAUDE.md §1).
- Não muda `scripts/agent-bar-open-terminal` para Rust (recebe só a nova
  injeção de fonte, permanece Bash).

## 3. Runtime assíncrono

- **Fetch**: `fetch_all` sai do branch do select! para `tokio::spawn`
  (ou task local), emitindo `Action::FetchStarted(provider)` /
  `Action::ProviderQuotaLoaded(id, result)` num canal já existente
  (`bg_rx`). O loop volta a desenhar a cada frame: spinner braille
  (throbber-widgets-tui, já dep) + checklist de progresso por provider
  no header ("claude ✓ codex ✓ amp …").
- **Login**: `LoginRequested` desenha 1 frame de status antes de
  suspender o terminal; ao retornar, dispara refetch automático do
  provider logado (novo `Action::LoginFinished(id)` → fetch único).
- **SaveConfig**: mesmo padrão — frame com "Salvando…" antes do IO.
- Parse de session logs continua em thread (`spawn_usage_load`), agora
  emitindo também records do Amp e bucketing horário.

## 4. Camada de dados

### 4.1 Claude: `limits[]` + `spend`

Novos tipos no `ClaudeUsageResponse` (aditivos, `#[serde(default)]`):

- `limits: Vec<ClaudeLimitRaw>` — `kind` (`session`, `weekly_all`,
  `weekly_scoped`, outros ignorados com log), `percent`, `severity`
  (string da API), `resets_at`, `is_active`,
  `scope.model.display_name` (nome do modelo para limites scoped).
- `spend: Option<ClaudeSpendRaw>` — `used {amount_minor, currency,
  exponent}`, `limit`, `percent`, `severity`, `enabled`,
  `can_purchase_credits`.

Mapeamento: `limits[]` é a fonte primária (session→primary,
weekly_all→secondary, weekly_scoped→`weekly_models[display_name]`);
campos legados (`five_hour`, `seven_day*`, `extra_usage`) viram
fallback quando `limits` está vazio. `severity` da API é propagada no
`QuotaWindow` (novo campo opcional) e tem precedência sobre os
thresholds locais 60/30/10 quando presente. `spend` alimenta a linha
"extra usage" do detail (desativado / $X de $Y).

Strings de erro existentes não mudam (contrato de teste).

### 4.2 Histórico

- `usage::records()` passa a produzir records do **Amp** (scan do log
  local da CLI do Amp; se não houver fonte de tokens, o Amp aparece no
  History com a série de custo `amp_dollars` — nunca some da lista).
- Bucketing **horário** (helper novo `bucket_by_hour`), com agregação
  diária derivada para a tabela 7d. O sparkline "tokens/h" do detail
  usa as últimas 24h do provider selecionado — dado real, por
  provider.

### 4.3 Login unificado

`is_logged_in` da tela de login é substituído pela mesma fonte do
dashboard: o resultado do último fetch (`is_available` + erro
NotLoggedIn tipado). Estados exibidos: `ok` (fetch com sucesso),
`sem token` (arquivo/binário presente mas auth inválida), `deslogado`
(fonte ausente), `verificando…` (fetch em voo). Nunca mais `[ok]`
derivado de `path.exists()`.

## 5. Navegação e IA

Sem tabs. Layout: `│ sidebar (17) │ conteúdo │` + linha de chips no
rodapé do conteúdo.

- **Sidebar**: seção `VISÃO` (item Geral), seção `PROVIDERS` (um item
  por provider: marca colorida + nome + % de relance; deslogado fica
  dim), seção `MAIS` (Histórico, Login, Waybar). Item selecionado tem
  fundo `selbg` + bold; hover (mouse) tem fundo mais sutil.
- **Estados do conteúdo**: Geral (cards compactos de todos), Detalhe
  (provider selecionado), Histórico, Login, Waybar (config atual).
- **Mouse** (crossterm `EnableMouseCapture`): click seleciona item de
  sidebar/card/chip; wheel rola a lista de cards e o histórico; hover
  destaca. Implementação: registro de `Rect`s clicáveis produzido no
  render (`Vec<(Rect, Action)>` no frame), consultado no handler de
  `MouseEvent` — mantém `update()` puro. Teclado permanece completo
  (↑↓ sidebar, Enter abre, Esc volta, atalhos dos chips).
- Trade-off documentado: mouse capture desabilita seleção de texto
  nativa do terminal (shift+drag contorna na maioria dos emuladores).

## 6. Telas

### 6.1 Visão geral (estado inicial)

```
╭─ agent-bar ────┬─ Visão geral ───────────── $434.12 hoje · 18:42 ⣾ ─╮
│ VISÃO          │                                                     │
│ ▸ Geral        │ ◆ Claude · Max 20x                            ● ok │
│                │   sessão ███████████▒▒   89%  ↻ 02:39              │
│ PROVIDERS      │   semana █████████████   97%  ↻ qui 22:59          │
│ ◆ Claude  89%  │   tokens/h ⣀⣠⣤⣶⣾⣿⣷⣄  $429.97 hoje                  │
│ ● Codex    –   │                                                     │
│ ● Amp     42%  │ ● Codex                                ○ deslogado │
│                │   sem sessão — clique aqui ou g para logar          │
│ MAIS           │                                                     │
│   Histórico    │ ● Amp · Pro                                   ● ok │
│   Login        │   créditos ██████▒▒▒▒▒▒   42%  $4.20/$10           │
│   Waybar       │   tokens/h ⣀⣀⣠⣤⣶⣄⣀⣀  cr $5.80                      │
│                │                                                     │
│                │        [↵ abrir] [r atualizar] [? ajuda] [q sair]   │
╰────────────────┴─────────────────────────────────────────────────────╯
```

Cards rolam (tui-scrollview) quando não cabem; sidebar lista N
providers. Card de deslogado é estado desenhado que ensina a ação.

### 6.2 Detalhe (provider selecionado)

Gauges sessão/semana/por-modelo (nomes de modelo vindos da API),
seção "Modelos hoje" (barra proporcional + tokens + custo por modelo,
do usage engine), "tokens/h 24h" (sparkline larga real + pico),
linha extra usage (`spend`), totais hoje/7d. Chips centralizados:
`[esc voltar] [r atualizar] [g login] [h histórico]`.

### 6.3 Histórico

Chart braille com área preenchida (`Chart` + `GraphType::Area`,
datasets por provider nas cores de marca) das últimas 24h/7d
(toggle), + tabela ccusage-style (dia × provider × tokens × custo)
com linhas coloridas por severidade de custo. Total no rodapé.

### 6.4 Login / Waybar

Login: lista de providers com estado real (§4.3) + painel de
instrução; spawn não-bloqueante com frame de feedback. Waybar: a tela
de config atual, com o mesmo skin (bordas, chips, alinhamento).

## 7. Sistema visual

- **Tokens novos** em `theme.rs`: `Surface` (#1b202a), `SelBg`
  (#2c333f), `ChipBg` (#262d3a), `Empty` (#343b49), `GreenHi`
  (#b5e890) — aditivos; os 12 existentes não mudam de valor.
- **Gauges**: preenchimento `█` com cor por célula (gradação da cor de
  severidade ao longo da barra; head brilhante durante sweep) e trilho
  `▒` em `Empty`. Severidade: API quando presente, senão thresholds
  atuais. Cores semânticas: verde ok / âmbar atenção / vermelho
  crítico; identidade do provider (laranja/verde/magenta) fica em
  títulos e marcas, nunca na severidade.
- **Bordas**: `BorderType::Rounded`, título embutido na borda superior
  (nome à esquerda, status à direita). Help overlay via tui-popup com
  `Clear` antes de desenhar (corrige a corrupção atual).
- **Contrato de alinhamento**: chips de ação centralizados; gauges de
  um painel na mesma coluna; percentuais e valores monetários
  alinhados à direita; padding interno de 1 célula; nomes truncados
  com `…` (nunca corte seco tipo "Free Tie").
- **Ícones**: Nerd Font, faixa Font Awesome estável (U+F000–F2FF):
  ✓ `f00c` ok, ✗ `f00d` deslogado, ⚠ `f071` atenção, 🔒 `f023` sem
  token, 🕐 `f017` reset, 💲 `f155` custo, 📈 `f201` histórico,
  ⚡ `f0e7` pico, ↻ `f021` atualizar, → `f090` login, ⚙ `f013`
  waybar, ⌨ `f120` terminal. Settings `menu.icons = "nerd" | "unicode"`
  (fallback ◆●○↻$▲!✓✗).
- **Fonte**: settings `menu.font.family` (default `"IBM Plex Mono"`) e
  `menu.font.size` (default 12). `agent-bar-open-terminal` injeta por
  flag conforme o terminal detectado (alacritty `--option`, kitty
  `-o`, foot `-o`, ghostty `--font-family`, wezterm `--config`);
  caminho xdg-terminal-exec genérico não suporta → usa a fonte padrão
  do terminal (fallback documentado). Braille/NF vêm do fallback de
  símbolos do fontconfig — nenhuma fonte patched exigida.

## 8. Motion (tachyonfx + throbber, anim_tick 30ms existente)

| Efeito | Gatilho | Duração | Observação |
| --- | --- | --- | --- |
| Spinner braille + progresso por provider | fetch/login em voo | contínuo | throbber-widgets-tui (deixa de ser código morto) |
| `fx::sweep_in(LeftToRight)` nos gauges | load/refresh (1x, não em tick) | ~900ms ease-out | head de célula brilhante |
| `fx::hsl_shift` + `ping_pong` | severidade crítica (<10%) | loop 1.1s | só no gauge afetado |
| `fx::coalesce` | troca visão geral ⇄ detalhe | ≤300ms | desligável |
| Count-up custo | junto do sweep | ~800ms | manual (interpolação no state) |

Setting global `menu.animations = true|false` desativa tudo (respeito
a reduced-motion). Efeitos rodam via `EffectManager::process_effects`
no draw; nenhum efeito bloqueia input.

## 9. Dependências

Adicionar: `tachyonfx` (org ratatui, compatível 0.30),
`tui-scrollview` (idem). Já presentes e reutilizadas: ratatui 0.30
(widgets nativos Chart/Sparkline/per-cell styling), crossterm 0.29
(mouse), throbber-widgets-tui, tui-popup, tui-input. Descartados:
tuirealm (framework pesado desnecessário), ratatui-image e
tui-big-text (fora do design aprovado).

## 10. Erros e estados

- Fetch falho por provider: card mostra o erro tipado (strings de
  contrato preservadas) com ícone de atenção; demais providers seguem.
- Histórico vazio/carregando: telas desenhadas (skeleton de barras em
  `Empty` + spinner), nunca área em branco.
- API sem `limits[]` (conta antiga): fallback legado transparente.
- Terminal estreito (<80 col): sidebar colapsa para coluna de marcas
  (3 células); conteúdo mantém prioridade gauge > sparkline > chips.
- `NO_COLOR`/`menu.icons=unicode`/`menu.animations=false`: degradação
  limpa (sem cor / glifos básicos / estático).

## 11. Testes

- Snapshots insta novos por tela × estado (geral, detalhe por
  provider, histórico 24h/7d, login, waybar; deslogado, carregando,
  erro, crítico) em 80 e 160 colunas — regravação completa é
  intencional (display contract muda de propósito).
- `update()` puro: casos novos para mouse actions, seleção de sidebar,
  fetch assíncrono (started/loaded/failed), login finished→refetch.
- Parser Claude: fixtures com `limits[]+spend` (resposta real de
  2026-07-01), só-legado, e ambos (limits vence).
- Hit-test: unit tests de Rect→Action.
- Verificação (CLAUDE.md §2): `cargo test providers::claude`,
  `cargo test usage`, `cargo test --test golden` (não deve mudar —
  contrato Waybar intacto), `cargo test waybar_contract` idem,
  `cargo clippy --all-targets -- -D warnings`.

## 12. Riscos e trade-offs registrados

- Mouse capture vs seleção de texto: aceito (shift+drag contorna).
- Animações em terminal remoto/lento: mitigado por `menu.animations`.
- `limits[]` é API não documentada publicamente (observada na
  resposta real): fallback legado cobre mudanças; decode nunca pode
  derrubar o corpo inteiro (lição do `extra_usage: null` já vivida).
- Fonte via flag de terminal não cobre o caminho xdg genérico:
  fallback é a fonte do terminal — comportamento atual, sem regressão.
- Amp sem fonte local de tokens: History mostra série de custo; card
  nunca some (princípio: a barra ensina, o menu aprofunda).

## 13. Decisões

| Decisão | Escolha | Por quê |
| --- | --- | --- |
| IA | Sidebar única + drill-down, sem tabs | Escala pra N providers; elimina redundância "Claude → todos"; usuário detesta tabs |
| Densidade | btop-style | Escolha explícita do usuário |
| Arquitetura | Manter MVU, async nos efeitos | Problema era IO no loop, não o padrão |
| Paleta | One Dark evoluído | Identidade commitada + gradientes semânticos |
| Fonte | IBM Plex Mono via `menu.font` | Personalidade "vibrante/expressivo"; mecânica via helper |
| Framework | Sem tuirealm | Custo alto, ganho marginal |
| Ícones | NF (FA range) + fallback | Omarchy tem NF; fallback preserva compat |
