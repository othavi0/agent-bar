# TUI (ratatui) — Design / Handoff completo

> **Status (v2, 2026-06-19):** design da TUI aprovado + **expandido** com o **engine de Usage/Custo (§4b)** após a decisão do usuário "tudo agora" e a investigação que confirmou que tokens/custo SÃO derivá­veis dos session logs locais. As 4 perguntas em aberto (§9) foram RESOLVIDAS. **Plano 6 (install) está FEITO** (branch @ `c6ed326`). Próximo passo: usuário revisa este spec v2 → `writing-plans` (Plano 7) → execução subagent-driven. **Ainda NÃO há código de TUI/usage** (fase de design).
>
> **Regra absoluta deste subsistema:** **ZERO emoji** em qualquer lugar (código, exemplos, UI). Só box-drawing, block elements (`█ ▓ ▒ ░`), sparkline braille (`▁▂▃▄▅▆▇`) e formas geométricas de apresentação-texto (`● ○ ◆ ┃`), sem variation selector VS16. (Preferência explícita e firme do usuário.)

## 0. Onde isto se encaixa (contexto macro)

Projeto **agent-bar**: monitor de quotas LLM (Claude/Codex/Amp) pra Waybar. Em **reescrita TS/Bun → binário Rust estático único** (branch `rust-rewrite`). Estado da reescrita: **Planos 1–6 COMPLETOS** (formatação + providers async + CLI + install), 433 testes, branch @ `c6ed326`. Faltam: **a TUI (este doc = Plano 7)**, **dist/cutover (Plano 8)**. Ver `docs/superpowers/rust-rewrite-resume.md` (handoff mestre) e `.superpowers/sdd/progress.md` (ledger).

**Por que uma TUI agora:** hoje o app só exibe quota usada/restante na barra. O **norte do usuário** é uma **ferramenta de monitoramento** (consumo de tokens, valor gasto em R$/US$, tempo, histórico). Isso justifica `ratatui` (full-screen). **Decisão "tudo agora" (2026-06-19):** o monitoramento de custo/tokens É v1 (não mais semente) — a investigação confirmou que os tokens estão nos session logs locais (§4b). A aba History vira tendência real de custo/tokens no tempo.

## 1. Decisões TRAVADAS (não relitigar)

1. **`ratatui` 0.30+ é a escolha** (não OpenTUI, não cliclack/dialoguer). Ver §7 (OpenTUI) e §8 (libs de prompt) pro porquê.
2. **ZERO emoji.** (Regra do topo.)
3. **Identidade visual compartilhada com a Waybar:** a TUI consome o **mesmo `rust/src/theme.rs`** (One Dark + box_chars + provider_hex). Uma ponte fina (`theme_bridge.rs`) converte `ColorToken` → `ratatui::Color::Rgb`. Um teste cruza `provider_color(id)` vs `provider_hex(id)` pra garantir que nunca divirjam.
4. **Esqueleto de layout (aprovado):** barra de **abas globais** no topo (`Dashboard | Waybar | History | Login`) + **lateral SÓ com providers** (é o "qual provider") + painel de conteúdo à direita + footer de atalhos. Modelo de 2 eixos: **aba = aspecto, lateral = provider, direita = o cruzamento**. (A lateral NÃO repete Waybar/Login — esses vivem só nas abas; redundância removida a pedido do usuário.)
5. **Escopo v1 ("tudo agora" — decisão do usuário 2026-06-19):** Dashboard (monitor: tabela + detalhe + refresh + animações + **custo/tokens US$/R$**), Login (lançar CLIs), Waybar (config de layout), History (**REAL** — tendência de tokens/custo no tempo dos logs), **+ engine de Usage/Custo (§4b)**. `configure-models` e persistência de %-quota = futuro.
6. **Sequência de planos:** `Plano 6 (install) ✅ FEITO → Plano 7: TUI (este) → Plano 8: dist/cutover`. A TUI precisa estar pronta **antes do cutover** e reusa peças do Plano 6 (waybar-integration p/ a aba Config; `install.rs`/locator p/ a aba Login — `install.rs` foi descopado do Plano 6 PRA CÁ).
7. **Entrypoint:** `agent-bar` sem args num TTY **abre a TUI** (`tui::run_tui(&ctx)`); `menu` continua entrypoint explícito; `--help` p/ ajuda. (Hoje `menu` é stub do Plano 6 em `main.rs`.)
8. **Runtime:** `tokio` current_thread (já é o do projeto). Sem `unwrap()`/`expect()` em produção. Snapshots via `insta` (mesmo padrão dos 58 goldens atuais).
9. **Custo = tokens locais × preço público** (§4b): session logs têm os tokens (Claude/Codex); tabela de preço estática versionada; US$ exato + R$ via `fx_rate` configurável; modelo desconhecido → custo omitido (nunca chutar).
10. **Glyphs:** box-drawing/block/geométrico universal + **nerd-font opt-in** (degrada gracioso). Zero emoji.
11. **History vem dos logs** (não de persistência nova) — os timestamps dos session logs são a fonte do histórico de tokens/custo.

## 2. Seção 1 — Identidade visual

Ponte de tema (`rust/src/tui/theme_bridge.rs`), sem hex hardcoded nos renders:
```rust
use ratatui::style::Color;
use crate::theme::ColorToken;

pub fn to_ratatui(token: ColorToken) -> Color {
    let h = token.hex().trim_start_matches('#');
    let p = |s: &str| u8::from_str_radix(s, 16).unwrap_or(0); // unwrap_or, NÃO unwrap
    Color::Rgb(p(&h[0..2]), p(&h[2..4]), p(&h[4..6]))
}
pub fn provider_color(id: &str) -> Color {
    match id {
        "claude" => to_ratatui(ColorToken::Orange),  // #d19a66
        "codex"  => to_ratatui(ColorToken::Green),   // #98c379
        "amp"    => to_ratatui(ColorToken::Magenta), // #c678dd
        _        => to_ratatui(ColorToken::Text),
    }
}
```
`Color::Rgb` é OBRIGATÓRIO (não `Color::Indexed` — dependeria da paleta do emulador e divergiria da barra).

| Elemento | Tratamento |
| --- | --- |
| Bordas | `BorderType::Thick` → `┏ ━ ┃ ┗` (mesmos codepoints do `box_chars`); borda **colorida por provider** |
| Foco de painel | borda Blue `#61afef` (focado) / Comment `#6a7485` (inativo). Foco vive em `AppState.focus`, passado ao construir o `Block` (ratatui não tem foco global) |
| Gauges | `Gauge::use_unicode(true)` → preenchimento parcial `▓▒░` (1/8 de célula); **cor por severidade** |
| Severidade (% restante) | green ≥60 · yellow 30–59 · orange 10–29 · red <10 (tokens One Dark) |
| Seleção (lista) | `highlight_symbol(" ◆ ")` + bg sutil `Rgb(45,53,65)` + bold; itens `●`/`○` (box_chars DOT/DOT_O) |
| Abas | ativa = bold+underline `#e2e8f0`; inativas `#97a1ae`; separador `┃` |

## 3. Seção 2 — Animações (4, tasteful; nada se mexe à toa)

| # | Animação | Por quê | Como |
| --- | --- | --- | --- |
| A | **Gauge lerp** | barra desliza até o valor novo (não pisca) → percebe-se o refresh | `display_ratio += (target-display)*0.20` a cada AnimTick 30ms (~360ms); ~10 linhas, sem lib |
| B | **Coalesce de texto** | card "materializa" o dado novo, sutil | `tachyonfx::coalesce(350ms, QuadOut)` filtrado em texto, dispara 1× por refresh |
| C | **Throbber no fetch** | indica "buscando…" antes do nome | `throbber-widgets-tui` braille (`⠷⠯⠟`), Cyan, 10 Hz, só enquanto `Loading` (CLOCK set proibido — é emoji) |
| D | **Pulse em crítico** | `<10%` restante → `●` pisca devagar | blink ~450ms, só quando `remaining < 10.0` |

**Evitar:** slide/sweep de área grande a cada refresh; `hsl_shift` contínuo (a paleta é identidade); fade em loop; qualquer símbolo do conjunto CLOCK. Tick de animação (~30ms) é **separado** do tick de dados (60–90s, TTL per-provider já existente).

## 4. Seção 3 — Arquitetura (MVU, testável, reusa o existente)

Módulos sob `rust/src/tui/`:
```
mod.rs           run_tui(&ctx) + setup/restore do terminal + panic hook
state.rs         AppState, Tab, FetchStatus, AnimState, Mode, Panel
action.rs        Action (toda entrada tipada)
update.rs        update(&mut AppState, Action) -> Vec<Action>   [PURA, testável]
event_loop.rs    tokio::select! (input + tick dados + tick anim) + fetch via mpsc
theme_bridge.rs  to_ratatui(ColorToken) + provider_color
login_spawn.rs   trait ProviderLogin + RealLogin (suspend->spawn->restore)
render/          mod + dashboard + detail + config + login + history + status_bar
widgets/         gauge + sparkline + provider_list + key_hint
```
Núcleo:
```rust
pub struct AppState {
    pub tab: Tab,                       // Dashboard | Waybar | History | Login
    pub providers: Vec<ProviderView>,   // quota + FetchStatus + anim (gauge lerp)
    pub selected: usize,                // índice na lateral
    pub mode: Mode,                     // List | Detail
    pub focus: Panel,                   // Sidebar | Content
    pub last_update: Option<time::OffsetDateTime>,
}
pub enum Action {
    Key(crossterm::event::KeyEvent), Tick, AnimTick,
    DataFetched(crate::providers::types::AllQuotas), FetchFailed(String),
    OpenDetail, Back, SwitchTab(Tab), SaveConfig, Quit,
}
pub fn update(state: &mut AppState, action: Action) -> Vec<Action>; // sem IO
```
Event loop (combina com current_thread):
```rust
tokio::select! {
    Some(Ok(ev)) = events.next() => drain(update(state, Action::Key(ev))),
    _ = data_tick.tick()         => spawn_fetch(&ctx, tx.clone()), // re-usa fetch_all/get_quota_for
    Some(act) = rx.recv()        => drain(update(state, act)),      // DataFetched/FetchFailed via mpsc
    _ = anim_tick.tick()         => update(state, Action::AnimTick),
}
terminal.draw(|f| render(state, f))?;
```
- **Reusa, não reescreve:** `providers::{registry, fetch_all, get_quota_for, Ctx}`, `settings::{load, save, Settings}`, `theme.rs`. A TUI é superfície NOVA (não toca `format_for_*`).
- **Login** (`login_spawn.rs`): trait mockável `ProviderLogin::launch(id)`; `RealLogin` faz `LeaveAlternateScreen`+`disable_raw_mode` → spawn `claude`/`codex auth login`/`amp login` (stdio herdado) → restaura + `clear()`.
- **Config (aba Waybar):** edita `Settings` via `tui-input`, salva com `settings::save`, aplica + sinaliza barra (`pkill -SIGUSR2`) — reusa Plano 6.

**Testabilidade (3 camadas, mesma do projeto):**
| Camada | Como |
| --- | --- |
| `update` puro, severidade→cor, transições | unit test |
| Cada tela | `TestBackend` + `insta` snapshot (igual aos 58 goldens) |
| Login spawn, animação, terminal real | smoke manual |

## 4b. Engine de Usage/Custo (NOVO subsistema — v1, decisão "tudo agora")

O norte do usuário (tokens consumidos + valor gasto R$/US$ no tempo) é alcançável lendo os **session logs locais**. Subsistema novo, separável da TUI (testável puro), vive em `rust/src/usage/`.

**Fontes por provider (formatos REAIS, verificados 2026-06-19):**

| Provider | Arquivo | Tokens | Modelo | Timestamp |
| --- | --- | --- | --- | --- |
| **Claude** | `~/.claude/projects/<hash>/<uuid>.jsonl` | linhas `type:"assistant"` → `message.usage.{input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens}` (por chamada) | `message.model` (ex `claude-opus-4-8`) na MESMA linha | `timestamp` ISO por linha |
| **Codex** | `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` | `event_msg/token_count` → `payload.info.last_token_usage` (delta por chamada) e `total_token_usage` (acumulado da sessão) c/ `input/cached_input/output/reasoning/total` | NÃO no evento de token → vem de `session_meta` (1×) ou `turn_context` (N×) da sessão | `timestamp` ISO por evento |
| **Amp** | `amp usage` (CLI, sem arquivo) | — (sem tokens) | — | — (saldo ao vivo) |

**Normalização:** `UsageRecord { provider, model: Option<String>, input, output, cache_read, cache_write, ts: OffsetDateTime }`. Amp não gera `UsageRecord` (entra como $ direto, sem derivação de token).

**Tabela de preços (estática, versionada em `usage/pricing.rs`):** `model → Pricing { input, output, cache_read, cache_write }` em **US$ por 1M tokens** (preço público; cache_read tem desconto). Cobre os modelos conhecidos (Claude opus/sonnet/haiku; Codex/gpt-5.x). **Modelo desconhecido → custo OMITIDO (mostra tokens, sem $) — NUNCA chutar preço silenciosamente** (mesmo princípio do "não reportar ok sem dado" do Waybar). Tabela é fácil de atualizar; comentar a data/fonte do preço.

**Custo:** `cost_usd = Σ (tokens_tipo × preço_tipo / 1e6)`. **R$:** `cost_brl = cost_usd × fx_rate`, com `fx_rate` configurável em settings (default sensato, ex 5.50; editável; fetch de FX ao vivo = DEFERIDO, estático é o v1). Mostra **US$ exato como primário, R$ como secundário**.

**Codex — atribuição de modelo:** usar o `total_token_usage` final da sessão (acumulado, evita double-count) atribuído ao modelo do `session_meta`/último `turn_context`. Pra buckets de tempo finos dentro da sessão, usar `last_token_usage` por evento com o `timestamp`.

**Agregação:** por modelo, por provider, por janela de tempo (hoje / janela 5h / janela 7d / all-time). Powers o painel de custo (Dashboard/detalhe) + a aba History (tendência no tempo, dos timestamps).

**Performance (CRÍTICO — log do Claude tem 12.8MB/sessão, ~1748 chamadas):** parsing **incremental com cache**. Cache de agregados em `~/.cache/agent-bar/usage/` keyed por (path do arquivo, size+mtime); no refresh, só re-parseia arquivos novos/que cresceram (lê do último offset p/ arquivos append-only). Os logs SÃO a fonte da verdade do histórico — sem DB próprio na v1.

**Módulos `rust/src/usage/`:** `mod.rs` (API: `aggregate(ctx, window) -> UsageSummary`), `claude.rs`/`codex.rs`/`amp.rs` (parsers por fonte, PUROS sobre `&str`/linhas — testáveis com fixtures), `pricing.rs` (tabela + `cost_of(record) -> Option<Cost>`), `cache.rs` (índice incremental por size+mtime). Tudo síncrono/puro (não está no hot-path async; roda no tick de dados da TUI ou sob demanda).

**Testabilidade:** parsers recebem linhas de fixture (1-2 linhas reais de cada formato, sanitizadas) → `UsageRecord` esperado; `pricing::cost_of` com tabela fixa → custo exato; modelo desconhecido → `None`. Sem tocar `~/.claude`/`~/.codex` reais nos testes (fixtures em `tests/` ou inline).

## 5. Seção 4 — Escopo v1 + tasks

| Aba | v1 ("tudo agora" — custo incluso) | Futuro |
| --- | --- | --- |
| Dashboard | tabela + detalhe (Enter) + refresh ao vivo + 4 animações + **coluna de custo/tokens (US$/R$) do engine §4b** | — |
| Login | lançar `claude`/`codex`/`amp login` | — |
| Waybar | providers/ordem/separador/modo (porta `configure-layout`) | `configure-models` |
| History | **REAL: tendência de tokens/custo no tempo, agregada dos session logs** (§4b) — sparkline/bar por provider/dia | persistência de % de quota, export |
| (engine) | **Usage/Custo §4b: parsers Claude/Codex/Amp + pricing US$/R$ + agregação + cache incremental** | fetch de FX ao vivo, mais modelos |

Tasks (incremental, cada uma testável + commit). **Bloco U = engine de custo (§4b); bloco T = TUI.** O engine é dependência do Dashboard-custo (T4b) e da History (T9):
| # | Entrega | Teste |
| --- | --- | --- |
| U1 | `usage/pricing.rs`: tabela estática + `cost_of(record) -> Option<Cost{usd,brl}>` (modelo desconhecido → None) | unit (custo exato + None) |
| U2 | `usage/claude.rs`: parser `.claude/projects/**/*.jsonl` → `Vec<UsageRecord>` (model+tokens+ts) | unit c/ fixture real |
| U3 | `usage/codex.rs`: parser `.codex/sessions/**` → `UsageRecord` (modelo de session_meta/turn_context + total_token_usage) | unit c/ fixture |
| U4 | `usage/cache.rs` + `mod.rs`: índice incremental (path,size,mtime) + `aggregate(ctx, window) -> UsageSummary` (por modelo/provider/tempo) + Amp $ direto | unit (agregação + incremental) |
| T1 | Scaffold `tui/` + `theme_bridge` + setup/restore + panic hook | unit: `provider_color` vs `provider_hex` |
| T2 | `AppState`+`Action`+`update` puro + event loop (navegar abas/lista) | unit: transições |
| T3 | Fetch via mpsc reusando `fetch_all`/`Ctx` + `FetchStatus` + aba Dashboard (tabela) | unit `update(DataFetched)` + snapshot |
| T4 | Detalhe (Enter): gauges por severidade, modelos, reset, sparkline | snapshot |
| T4b | Integra custo (§4b) no Dashboard/detalhe: coluna US$/R$ + breakdown por modelo | snapshot |
| T5 | Widgets custom (`QuotaGauge`/`ProviderList`/`Sparkline`) + cores por severidade | snapshot |
| T6 | Animações (lerp, throbber, coalesce, pulse) + glyph mode (box-drawing/nerd opt-in) | unit (estado) + smoke |
| T7 | Aba Waybar Config: editar Settings via `tui-input` + salvar + aplicar (reusa Plano 6) | unit + snapshot |
| T8 | Aba Login: trait `ProviderLogin` + `RealLogin` (suspend→spawn→restore) | unit (mock) + smoke |
| T9 | Aba History REAL: tendência tokens/custo no tempo (agregação §4b por dia/janela) | snapshot |
| T10 | `agent-bar` sem-args/`menu` → `run_tui`; footer + overlay help (`?`); polish + review de branch | snapshot + smoke |

## 6. Mockups aprovados (ASCII, alinhamento verificado por script — 62 colunas, zero char wide)

> Gerados por `/tmp/mock_tui2.py` (script descartável; lógica: largura fixa S=13/C=46, verificação `east_asian_width`). Reproduzir o método se forem refeitos. A lateral é SÓ providers.

**Home — aba Dashboard (tabela de todos):**
```
┌─ agent-bar ─────────────────────────────────────────── 3s ─┐
│ ┃Dashboard┃│ Waybar │ History │ Login                      │
├─────────────┬──────────────────────────────────────────────┤
│PROVIDERS    │ Todos os providers                           │
│> Claude  26%│         uso          reset  custo            │
│  Codex    1%│ Claude  ██░░░░░  26%  23:00  $2.10           │
│  Amp      0%│ Codex   ░░░░░░░   1%  01:28  -               │
│  + add      │ Amp     ░░░░░░░   0%  -      cr $4.19        │
│             │                                              │
│             │ Total hoje                        ~$2.10     │
├─────────────┴──────────────────────────────────────────────┤
│ ↑↓ provider · ←→ aba · Enter detalhe · [q]uit              │
└────────────────────────────────────────────────────────────┘
```
**Enter num provider → detalhe (ainda na aba Dashboard):**
```
┌─ agent-bar ─────────────────────────────────────────── 3s ─┐
│ ┃Dashboard┃│ Waybar │ History │ Login                      │
├─────────────┬──────────────────────────────────────────────┤
│PROVIDERS    │ Claude · Max 5x                              │
│> Claude  26%│ 5h  █████░░░░░░░░░░░░░  26%  -> 23:00 (4h55m)│
│  Codex    1%│ wk  ██░░░░░░░░░░░░░░░░  12%  -> dom          │
│  Amp      0%│                                              │
│  + add      │ Models                                       │
│             │   Opus    ██████░░░░░░░░  41%                │
│             │   Sonnet  ███░░░░░░░░░░░  20%                │
│             │                                              │
│             │ tokens/h ▁▂▃▅▇▆▄▂▁          ~$2.10 hoje      │
├─────────────┴──────────────────────────────────────────────┤
│ ↑↓ provider · Esc volta · ←→ aba · [r]efresh               │
└────────────────────────────────────────────────────────────┘
```
(Abas Waybar/History/Login seguem o mesmo esqueleto, trocando o painel direito. History mostra sparklines de tokens/custo por provider.)

## 7. Stack de crates (verificar versões/compatibilidade na implementação)

```toml
ratatui              = "0.30"   # padrão de facto; fork mantido do tui-rs
crossterm            = { version = "0.28", features = ["event-stream"] }
tui-input            = "0.11"   # campo de texto (config/login)
throbber-widgets-tui = "0.11"   # spinner (MSRV 1.88 — OK, toolchain do projeto é 1.95)
tui-popup            = "0.5"    # modais auto-centrados
tachyonfx            = "0.25"   # efeitos pós-render (fade/coalesce); org ratatui
tokio-util           = { version = "0.7", features = ["sync"] } # CancellationToken
# dev: insta (já existe)
```
Widgets ratatui usados: `Gauge`/`LineGauge` (quota), `Sparkline`/`Chart`/`BarChart` (histórico), `Table` (overview), `Tabs`, `Block`, `List`, `Paragraph`, `Scrollbar`. Padrão de refresh: `tokio::select!` + `EventStream` + `interval`. Login: suspend→spawn→restore (receita oficial `ratatui.rs/recipes/apps/spawn-vim`).

## 8. Caminhos considerados e DESCARTADOS (não re-explorar sem motivo novo)

- **OpenTUI:** é lib **TypeScript** (núcleo Zig + bindings TS via FFI, runtime **Bun**; React/Solid). Adotar de verdade **reintroduz Bun+npm** — contra a decisão travada do binário único/sem-npm. Existe port Rust comunitário (`Dicklesworthstone/opentui_rust`), mas é de 1 mantenedor, imaturo, + dependência de toolchain Zig/FFI. **Rejeitado** pra este projeto (ratatui é Rust-nativo, ecossistema maduro, compila no mesmo binário, compartilha theme.rs). OpenTUI só ganharia num **híbrido** (Rust core + app Bun pro menu) — que re-divide o stack; o usuário NÃO quis.
- **Libs de prompt** (`cliclack` port do @clack/prompts, `dialoguer`, `inquire`): bastam pra wizards sequenciais, mas o usuário quer **full-screen + monitoramento + refresh** → ratatui. O TUI atual do TS usa `@clack/prompts` (select/confirm/note/spinner) — 1198 linhas em `src/tui/` + `src/menu.ts`; será **substituído** pela TUI ratatui (não portado 1:1).
- **3 direções de layout iniciais** (A dashboard-first, B tabbed, C master-detail) → o usuário curtiu **B (abas) + C (lateral)** → convergiu no esqueleto §1.4. Os mockups intermediários estão no histórico do chat (não re-derivar).
- **`cursive`** (outro framework TUI): mais alto-nível, menos controle/testabilidade que ratatui pra dashboard+refresh. Rejeitado.

## 9. Perguntas em aberto — TODAS RESOLVIDAS (decisões do usuário, 2026-06-19)

1. **Glyphs:** ✅ **box-drawing/block/geométrico como base UNIVERSAL** (funciona em qualquer terminal) **+ glyphs nerd-font como opt-in** que degrada gracioso (toggle em settings; auto-fallback p/ box-drawing se a fonte não tiver). Motivo: uma TUI NÃO pode embarcar/forçar fonte (quem decide o glyph é a fonte do terminal do usuário); o Omarchy do usuário tem nerd-font, mas a distribuição AUR não garante. ZERO emoji em qualquer caso.
2. **Estilo da aba ativa:** ✅ mantido (bold+underline na cor `#e2e8f0` + separador `┃`).
3. **`menu` no TTY:** ✅ **`agent-bar` sem args num TTY ABRE a TUI direto**; a ajuda passa a ser via `--help`. (Muda o default atual — o wiring do `main.rs` no Plano 8/cutover ajusta; `menu` continua como entrypoint explícito também.)
4. **Sequência:** ✅ **Plano 6 (install) FEITO** (commits 8752dec..c6ed326). A TUI é o Plano 7, com o Plano 6 inteiro como base.

## 10. Problemas/riscos AINDA NÃO totalmente trazidos à tona

1. **DADOS pro monitoramento — RESOLVIDO (investigação 2026-06-19).** A API upstream de quota só dá %/reset, MAS os **session logs locais têm os tokens absolutos** — o norte de custo/tokens É alcançável (ver a nova **§4b. Engine de Usage/Custo**). Resumo: **Claude** (`~/.claude/projects/**/*.jsonl`) = goldmine (input/output/cache tokens + `message.model` + timestamp por chamada → custo de alta precisão); **Codex** (`~/.codex/sessions/**/*.jsonl`) = tokens nos eventos `token_count` + modelo no `session_meta`/`turn_context` (ex: `gpt-5.5`); **Amp** = sem tokens, mas o $ já é direto. **Decisão do usuário: engine de custo completo na v1 ("tudo agora").**
2. **History real — RESOLVIDO: vem dos PRÓPRIOS LOGS.** Como os session logs têm timestamp por chamada, o histórico de tokens/custo no tempo é agregável direto dos logs (sem camada de persistência separada). Persistência de poll só seria necessária pro histórico de **% de quota** (que não está nos logs) — DEFERIDO (não-v1). Ver §4b.
3. **Cross-dependência com o Plano 5 já entregue:** o `action_right.rs` tem um `login_stub` (log::error+wait_enter) que foi deixado **esperando a TUI**. Quando a TUI landar, esse stub deve chamar o `tui::login_spawn` (ou compartilhar o módulo). Anotar pra não esquecer o stub vivo.
4. **`menu` é stub no `main.rs`** (Plano 6) — a TUI substitui. E as abas Config/Login dependem do Plano 6 (`waybar-integration`, locator). Por isso a sequência 6→TUI.
5. **Versões das crates** (§7) vieram da pesquisa (jun/2026) — confirmar compat com ratatui 0.30 e pinar no momento da implementação (esp. `tachyonfx` que segue de perto as breaking changes do ratatui).
6. **Pendência herdada (Plano 5, fora da TUI):** `logger` emite Warn quando `!verbose` (o TS é silent) → spam de stderr/journald no poll do Waybar. Fix de 1 linha em `logger.rs` (`Off` quando `!verbose`) — **oferta pendente ao usuário**, não aplicada.

## 11. Arquivos de evidência (onde olhar)

- Identidade: `rust/src/theme.rs` (One Dark, box_chars, provider_hex, severidade em `config.rs::status_for_percent`).
- Reuso: `rust/src/providers/mod.rs` (`Ctx`, `registry`, `fetch_all`, `get_quota_for`), `rust/src/settings.rs` (`Settings`, `load`, `save`), `rust/src/providers/types.rs` (`AllQuotas`, `ProviderQuota`).
- O que a TUI substitui (TS): `src/tui/*.ts` (index/configure-layout/configure-models/login/login-single/list-all/colors) + `src/menu.ts` (~1198 linhas, `@clack/prompts`).
- Entrypoint a trocar: `rust/src/main.rs` (arm `Command::Menu` = stub atual).
- Stub a religar: `rust/src/action_right.rs` (`login_stub`).
- Handoff macro: `docs/superpowers/rust-rewrite-resume.md`; ledger `.superpowers/sdd/progress.md`.

## 12. Próximos passos (retomada)

1. **Usuário revisa este spec v2** (gate do brainstorming) — esp. a §4b (engine de custo) e o escopo "tudo agora".
2. ✅ As 4 perguntas (§9) e a incógnita de dados (§10.1) estão RESOLVIDAS.
3. `writing-plans` → plano do **Plano 7** com as tasks U1-U4 (engine §4b) + T1-T10 (TUI §5), código exato por task.
4. Executar via subagent-driven (mesmo loop dos Planos 4/5/6: implementer→verify-fs→review→ledger; snapshots `insta` por tela; review de branch Opus no fim).
5. ✅ **Plano 6 (install) FEITO** — a TUI tem toda a base pronta. `install.rs` (locator/ensure_command p/ a aba Login) foi descopado do Plano 6 e entra aqui no Plano 7.
