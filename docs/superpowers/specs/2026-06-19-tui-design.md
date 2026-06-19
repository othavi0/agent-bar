# TUI (ratatui) — Design / Handoff completo

> **Status:** design **aprovado pelo usuário** seção-a-seção (brainstorming concluído). Próximo passo do fluxo: usuário revisa este spec → `writing-plans` → execução subagent-driven. **Ainda NÃO há código de TUI** (fase de design). Este doc é a fonte da verdade do design + handoff de contexto.
>
> **Regra absoluta deste subsistema:** **ZERO emoji** em qualquer lugar (código, exemplos, UI). Só box-drawing, block elements (`█ ▓ ▒ ░`), sparkline braille (`▁▂▃▄▅▆▇`) e formas geométricas de apresentação-texto (`● ○ ◆ ┃`), sem variation selector VS16. (Preferência explícita e firme do usuário.)

## 0. Onde isto se encaixa (contexto macro)

Projeto **agent-bar**: monitor de quotas LLM (Claude/Codex/Amp) pra Waybar. Em **reescrita TS/Bun → binário Rust estático único** (branch `rust-rewrite`). Estado da reescrita: **Planos 1–5 COMPLETOS** (formatação + providers async + CLI), 378 testes, branch @ `d1d5fed`. Faltam: **Plano 6 (install)**, **a TUI (este doc)**, **dist/cutover**. Ver `docs/superpowers/rust-rewrite-resume.md` (handoff mestre) e `.superpowers/sdd/progress.md` (ledger).

**Por que uma TUI agora:** hoje o app só exibe quota usada/restante na barra. O **norte do usuário** é evoluir pra uma **ferramenta de monitoramento** (consumo de tokens, valor gasto em R$/US$, tempo, histórico). Isso justifica `ratatui` (full-screen, espaço pra crescer) em vez de prompts simples. **O monitoramento profundo NÃO é v1** — v1 deixa a aba History plugada como semente.

## 1. Decisões TRAVADAS (não relitigar)

1. **`ratatui` 0.30+ é a escolha** (não OpenTUI, não cliclack/dialoguer). Ver §7 (OpenTUI) e §8 (libs de prompt) pro porquê.
2. **ZERO emoji.** (Regra do topo.)
3. **Identidade visual compartilhada com a Waybar:** a TUI consome o **mesmo `rust/src/theme.rs`** (One Dark + box_chars + provider_hex). Uma ponte fina (`theme_bridge.rs`) converte `ColorToken` → `ratatui::Color::Rgb`. Um teste cruza `provider_color(id)` vs `provider_hex(id)` pra garantir que nunca divirjam.
4. **Esqueleto de layout (aprovado):** barra de **abas globais** no topo (`Dashboard | Waybar | History | Login`) + **lateral SÓ com providers** (é o "qual provider") + painel de conteúdo à direita + footer de atalhos. Modelo de 2 eixos: **aba = aspecto, lateral = provider, direita = o cruzamento**. (A lateral NÃO repete Waybar/Login — esses vivem só nas abas; redundância removida a pedido do usuário.)
5. **Escopo v1:** Dashboard (monitor: tabela + detalhe + refresh + animações), Login (lançar CLIs), Waybar (config de layout), History (placeholder com buffer em memória). `configure-models` e monitoramento persistido = futuro.
6. **Sequência de planos (escolha do usuário):** `Plano 6 (install) → Plano 7: TUI → Plano 8: dist/cutover`. A TUI precisa estar pronta **antes do cutover** (senão o binário vira canônico com `menu` ainda stub) e depende de peças do Plano 6 (waybar-integration p/ a aba Config; locator/ensureCommand p/ a aba Login).
7. **Entrypoint:** o comando `menu` (hoje stub do Plano 6 em `main.rs`) vira `tui::run_tui(&ctx)`.
8. **Runtime:** `tokio` current_thread (já é o do projeto). Sem `unwrap()`/`expect()` em produção. Snapshots via `insta` (mesmo padrão dos 58 goldens atuais).

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

## 5. Seção 4 — Escopo v1 + tasks

| Aba | v1 | Futuro |
| --- | --- | --- |
| Dashboard | tabela + detalhe (Enter) + refresh ao vivo + 4 animações | — |
| Login | lançar `claude`/`codex`/`amp login` | — |
| Waybar | providers/ordem/separador/modo (porta `configure-layout`) | `configure-models` |
| History | sparkline de buffer em memória (semente) | tokens/custo/tempo persistidos, gráficos, export |

Tasks (incremental, cada uma testável + commit):
| # | Entrega | Teste |
| --- | --- | --- |
| T1 | Scaffold `tui/` + `theme_bridge` + setup/restore + panic hook (frame vazio, abre/fecha limpo) | unit: `provider_color` vs `provider_hex` |
| T2 | `AppState`+`Action`+`update` puro + event loop (navegar abas/lista, sem dados) | unit: transições |
| T3 | Fetch via mpsc reusando `fetch_all`/`Ctx` + `FetchStatus` + aba Dashboard (tabela) | unit `update(DataFetched)` + snapshot |
| T4 | Detalhe (Enter): gauges por severidade, modelos, reset, sparkline | snapshot |
| T5 | Widgets custom (`QuotaGauge`/`ProviderList`/`Sparkline`) + cores por severidade | snapshot |
| T6 | Animações (lerp, throbber, coalesce, pulse) | unit (estado) + smoke |
| T7 | Aba Waybar Config: editar Settings via `tui-input` + salvar + aplicar (reusa Plano 6) | unit + snapshot |
| T8 | Aba Login: trait `ProviderLogin` + `RealLogin` (suspend→spawn→restore) | unit (mock) + smoke |
| T9 | Aba History (placeholder): sparkline de buffer em memória | snapshot |
| T10 | `menu` → `run_tui`; footer de atalhos + overlay help (`?`); polish + review de branch | snapshot + smoke |

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

## 9. Perguntas em aberto / minhas dúvidas (resolver no spec-review ou no plano)

1. **Glyphs nerd-font na TUI?** A barra Waybar usa um glyph nerd-font (U+F1616). Na TUI, podemos usar ícones nerd-font (NÃO são emoji) ou ficar **só** em box-drawing/block? (Default seguro proposto: só box-drawing/block, pra não depender da fonte do terminal — mas o terminal do usuário tem nerd-font.) **Decidir.**
2. **Estilo da aba ativa:** mantido `┃ativa┃`. O usuário cogitou "sublinhado/cor em vez de barras" — confirmar preferência final (o design atual usa bold+underline na cor + `┃` separador).
3. **`menu` no TTY:** hoje `agent-bar` sem args num TTY mostra help; `menu` abre a TUI. Manter assim, ou `agent-bar` sem args num TTY já abrir a TUI? (Default: manter — `menu` é o entrypoint explícito.)
4. **Sequência confirmada** (6→TUI→cutover), mas: fazer Plano 6 **inteiro** antes, ou adiantar só `waybar-integration`+locator (peças que a TUI Config/Login precisam) e intercalar? (Default: Plano 6 inteiro primeiro — mais limpo.)

## 10. Problemas/riscos AINDA NÃO totalmente trazidos à tona

1. **DADOS pro monitoramento (o norte) são LIMITADOS pela API upstream.** Hoje os providers expõem **% usado/restante + reset** (Claude/Codex) e **$ créditos/replenish** (Amp). NÃO expõem contagem de tokens nem $ gasto por período pra Claude/Codex. Então "valor gasto / tokens consumidos no tempo" pode ser **derivável só parcialmente** (ex.: variação de % ao longo do tempo + $ do Amp). **Antes de prometer o monitoramento de custo, validar o que cada API realmente dá.** Pode exigir estimativa (ex.: tokens≈f(uso%·limite_do_plano)) e/ou só Amp ter $ real. **Não-surfado até agora — é a maior incógnita do norte.**
2. **History real precisa de PERSISTÊNCIA.** O buffer em memória da v1 é só semente; histórico de verdade exige persistir amostras (ex.: append JSONL ou SQLite em `cache_dir`) num tick. É uma camada nova de dados — fora da v1, mas precisa ser desenhada quando o monitoramento virar real.
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

1. Usuário revisa este spec (gate do brainstorming).
2. Resolver as 4 perguntas em aberto (§9) e validar a incógnita de dados do monitoramento (§10.1).
3. `writing-plans` → plano de implementação da TUI (Plano 7) com as 10 tasks (§5).
4. Executar via subagent-driven (mesmo loop dos Planos 4/5: implementer→verify-fs→review→ledger; snapshots `insta` por tela; review de branch Opus no fim).
5. Lembrar: **Plano 6 (install) vem ANTES** da TUI na sequência travada.
