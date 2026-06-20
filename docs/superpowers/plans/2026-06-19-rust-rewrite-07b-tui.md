# Plano 07b — TUI (ratatui): monitor full-screen + custo + login + config

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development, task-by-task. Steps usam checkbox (`- [ ]`). **REGRA ABSOLUTA: ZERO emoji em qualquer código/UI** — só box-drawing, block (`█▓▒░`), sparkline braille (`▁▂▃▄▅▆▇`), formas geométricas (`●○◆┃`). Sem variation selector VS16, sem CLOCK/relógio.

**Goal:** Construir a TUI ratatui full-screen do agent-bar: abas globais (Dashboard/Waybar/History/Login) + lateral de providers + detalhe, consumindo o engine de custo (Plano 7a) pra mostrar tokens/US$/R$, com login interativo e config de Waybar (Plano 6).

**Architecture:** MVU testável — `update(&mut AppState, Action) -> Vec<Action>` PURO (sem IO), event loop `tokio::select!` (input + tick de dados + tick de animação), render por tela via `TestBackend` + snapshots `insta`. Reusa `providers::{fetch_all,get_quota_for,Ctx}`, `usage::{aggregate,records_since}`, `settings`, `theme.rs` (via `theme_bridge`), e os comandos do Plano 6 (`setup`/`waybar_integration`/`install`). Substitui o `src/tui/*.ts` (@clack) e o stub `Command::Menu`.

**Tech Stack:** ratatui 0.30.2, crossterm via `ratatui::crossterm` (re-export — NÃO adicionar crossterm direto, evita skew de versão), tui-input 0.15.3, throbber-widgets-tui 0.11.1, tui-popup 0.7.6, tachyonfx 0.25.0, tokio-util 0.7.18. tokio current_thread (já é o do projeto). insta (já dev-dep).

## Global Constraints

- **ZERO emoji** (regra do topo). Identidade compartilhada com a Waybar via `theme.rs` (One Dark + box_chars + provider_hex) — `theme_bridge` converte `ColorToken → ratatui::Color::Rgb` (NUNCA `Color::Indexed`). Um teste cruza `provider_color(id)` vs `theme::provider_hex(id)`.
- **`ratatui::crossterm`** (re-export) p/ todos os tipos de evento/terminal — NÃO declarar `crossterm` como dep direta (a stack confirmou crossterm 0.29; usar o que o ratatui 0.30.2 re-exporta evita 2 versões e mismatch de `KeyEvent`).
- **Sem `unwrap()`/`expect()` em produção** (o `main.rs`/`lib.rs` têm `deny`). Em teste é permitido. No `theme_bridge`, `u8::from_str_radix(..).unwrap_or(0)`.
- **`update` é PURO** (sem IO/spawn/relógio) → testável. IO (fetch, login spawn, save) vive no event loop / módulos de seam.
- **Snapshots `insta` por tela** via `TestBackend` (mesmo padrão dos 58 goldens). `cargo insta` NUNCA aceito cego — revisar cada snapshot (é o contrato visual). **ZERO emoji nos snapshots** (se um emoji vazar, o snapshot o pega).
- **Consome o engine 7a:** `usage::aggregate(AggregateOptions{claude_dir,codex_dir,fx_rate,amp_meta})`, `usage::records_since(opts, cutoff)`. Paths dos logs: `~/.claude/projects`, `~/.codex/sessions` (derivar do HOME; `fx_rate` de settings — adicionar `fx_rate` a settings nesta plano, T7).
- **Verificação:** `cargo test --manifest-path rust/Cargo.toml <filtro>` + `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`. **RTK:** sem `test result:`; some os `passed`; clean = `cargo clippy: No issues found`. **`cargo fmt` ANTES de `git add`.** **Read antes de Edit** (cat/sed não contam; re-Read se "string not found").
- **API ratatui 0.30 — VERIFICAR, não chutar:** o implementer DEVE consultar a doc 0.30 via `ctx7 docs /websites/rs_ratatui_0_30_0 "<pergunta>"` antes de usar um widget que não esteja explícito aqui (Table/Gauge/Sparkline/Tabs/List/Block têm nuances de builder por versão). Padrões já confirmados (0.30): `ratatui::init() -> DefaultTerminal` / `ratatui::restore()` (ou `try_init`/`try_restore` p/ Result); `terminal.draw(|frame| ...)`; `frame.render_widget(w, area)`; eventos via `ratatui::crossterm::event::{Event,KeyCode,KeyEventKind}`; async via `ratatui::crossterm::event::EventStream` + `futures::StreamExt::next`.
- **Commits:** Conventional Commits PT ≤50 chars.

## Mapa de Arquivos (`rust/src/tui/` salvo nota)

| Arquivo | Responsabilidade |
| --- | --- |
| `tui/mod.rs` | `run_tui(ctx) -> Result` + setup/restore terminal + panic hook (restaura terminal no panic) |
| `tui/theme_bridge.rs` | `to_ratatui(ColorToken) -> Color` + `provider_color(id) -> Color` |
| `tui/state.rs` | `AppState`, `Tab`, `Panel`, `Mode`, `FetchStatus`, `ProviderView`, `AnimState` |
| `tui/action.rs` | `Action` (toda entrada tipada) |
| `tui/update.rs` | `update(&mut AppState, Action) -> Vec<Action>` PURO |
| `tui/event_loop.rs` | `tokio::select!` (input + data tick + anim tick) + fetch via mpsc |
| `tui/login_spawn.rs` | trait `ProviderLogin` + `RealLogin` (suspend→spawn→restore) |
| `tui/render/mod.rs` + `dashboard.rs` `detail.rs` `config.rs` `login.rs` `history.rs` `status_bar.rs` | render por tela (puro: `State -> Frame`) |
| `tui/widgets/` | `quota_gauge.rs` `provider_list.rs` `sparkline.rs` `key_hint.rs` |
| `install.rs` (raiz) | `ensure_command`/`ensure_amp_cli` (DESCOPADO do Plano 6 — login depende) |
| `settings.rs` (mod) | + campo `fx_rate: f64` (default 5.50) |
| `main.rs` (mod) | `Command::Menu` + no-args TTY → `tui::run_tui` |

Ordem: **T1 scaffold → T2 MVU+loop → T3 dashboard+fetch → T4 detalhe → T4b custo → T5 widgets → T6 animações → T7 config+settings.fx_rate → T8 login+install.rs → T9 history → T10 wiring+polish**.

---

### Task 1: Scaffold `tui/` + Cargo deps + `theme_bridge` + setup/restore

**Files:** Create `rust/src/tui/mod.rs`, `rust/src/tui/theme_bridge.rs`; Modify `rust/Cargo.toml` (deps), `rust/src/lib.rs` (`pub mod tui;`). Test: inline.

**Interfaces produces:** `tui::run_tui(ctx: &Ctx) -> anyhow::Result<()>` (T1: abre terminal, desenha 1 frame vazio com título, sai na 1ª tecla — esqueleto); `theme_bridge::{to_ratatui, provider_color}`.

- [ ] **Step 1: Cargo deps** — adicionar em `[dependencies]` (versões EXATAS da stack verificada):
```toml
ratatui = "0.30.2"
tui-input = "0.15.3"
throbber-widgets-tui = "0.11.1"
tui-popup = "0.7.6"
tachyonfx = "0.25.0"
tokio-util = { version = "0.7", features = ["rt"] }
```
**NÃO** adicionar `crossterm` direto (usar `ratatui::crossterm`). `futures` já é dep.

- [ ] **Step 2: Write failing test (`theme_bridge`)**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{provider_hex, ColorToken};
    use ratatui::style::Color;

    #[test]
    fn to_ratatui_parses_hex() {
        assert_eq!(to_ratatui(ColorToken::Green), Color::Rgb(0x98, 0xc3, 0x79));
    }
    #[test]
    fn provider_color_matches_theme_provider_hex() {
        for id in ["claude", "codex", "amp", "other"] {
            let h = provider_hex(id).trim_start_matches('#');
            let want = Color::Rgb(
                u8::from_str_radix(&h[0..2], 16).unwrap(),
                u8::from_str_radix(&h[2..4], 16).unwrap(),
                u8::from_str_radix(&h[4..6], 16).unwrap());
            assert_eq!(provider_color(id), want, "divergiu p/ {id}");
        }
    }
}
```

- [ ] **Step 3: Run → fail.** `cargo test --manifest-path rust/Cargo.toml theme_bridge 2>&1 | tail -6` (e `cargo build` p/ baixar as crates).

- [ ] **Step 4: Implement `theme_bridge.rs`**
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
        "claude" => to_ratatui(ColorToken::Orange),
        "codex" => to_ratatui(ColorToken::Green),
        "amp" => to_ratatui(ColorToken::Magenta),
        _ => to_ratatui(ColorToken::Text),
    }
}
```

- [ ] **Step 5: Implement `tui/mod.rs` esqueleto** (VERIFICAR API 0.30 via ctx7):
  - `pub async fn run_tui(ctx: &Ctx) -> anyhow::Result<()>`: instala panic hook que chama `ratatui::restore()` (senão um panic deixa o terminal quebrado); `let mut terminal = ratatui::try_init()?;` (Result, não `init()` que panica); loop: `terminal.draw(|f| f.render_widget(Block::bordered().title("agent-bar"), f.area()))?;` + lê 1 tecla via `ratatui::crossterm::event` (bloqueante por ora; o loop async vem na T2) → sai; `ratatui::try_restore()?;`. (T1 só prova que abre/fecha limpo.)
  - `pub mod theme_bridge;` + os outros módulos (state/action/etc.) declarados conforme forem criados (T1 pode declarar só theme_bridge).
  - `lib.rs`: `pub mod tui;` (alfabético — depois de `theme`, antes de `update`).

- [ ] **Step 6: Run → pass** (`theme_bridge` test) + `cargo clippy`. (`run_tui` é smoke manual; NÃO rodar TUI real em CI/teste.)

- [ ] **Step 7: `cargo fmt` + commit**
```bash
cargo fmt --manifest-path rust/Cargo.toml
git add rust/Cargo.toml rust/Cargo.lock rust/src/tui/ rust/src/lib.rs
git commit -m "feat(rust): tui scaffold + theme_bridge + deps"
```

---

### Task 2: `state.rs` + `action.rs` + `update.rs` (MVU puro) + `event_loop.rs`

**Files:** Create `tui/{state,action,update,event_loop}.rs`; Modify `tui/mod.rs`. Test: inline (transições do `update`).

**Interfaces produces:**
```rust
// state.rs
pub enum Tab { Dashboard, Waybar, History, Login }
pub enum Panel { Sidebar, Content }
pub enum Mode { List, Detail }
pub enum FetchStatus { Idle, Loading, Loaded, Failed(String) }
pub struct ProviderView { pub quota: ProviderQuota, /* gauge anim na T6 */ }
pub struct AppState {
    pub tab: Tab, pub providers: Vec<ProviderView>, pub selected: usize,
    pub mode: Mode, pub focus: Panel, pub status: FetchStatus,
    pub last_update: Option<time::OffsetDateTime>, pub should_quit: bool,
}
impl AppState { pub fn new() -> Self }
// action.rs
pub enum Action {
    Key(ratatui::crossterm::event::KeyEvent), Tick, AnimTick,
    DataFetched(crate::providers::types::AllQuotas), FetchFailed(String),
    Up, Down, OpenDetail, Back, SwitchTab(Tab), Refresh, Quit,
}
// update.rs
pub fn update(state: &mut AppState, action: Action) -> Vec<Action>; // PURO, sem IO
```

- [ ] **Step 1: Write failing tests (`update` transições)**
```rust
#[test] fn down_moves_selection_and_clamps() { /* Down em 3 providers vai 0→1→2→2 (clamp) */ }
#[test] fn open_detail_then_back() { /* Mode::List →OpenDetail→ Detail →Back→ List */ }
#[test] fn switch_tab_changes_tab_resets_mode() { /* SwitchTab(Waybar) → tab=Waybar, mode=List */ }
#[test] fn data_fetched_populates_providers_and_status() { /* DataFetched → status=Loaded, providers preenchidos, last_update Some */ }
#[test] fn key_q_sets_should_quit() { /* Key('q') → Quit → should_quit=true */ }
```
*(Escrever cada um completo: construir AppState::new() com providers fake, aplicar a Action, assertar o estado. `update` mapeia `Key(KeyCode::Char('j')|Down)`→`Down`, `'k'|Up`→`Up`, `Enter`→`OpenDetail`, `Esc`→`Back`, `Left/Right/Tab`→`SwitchTab` cíclico, `'r'`→`Refresh`, `'q'`→`Quit`. `Refresh` retorna `vec![]` mas o loop dispara fetch; `update` só seta `status=Loading`.)*

- [ ] **Step 2: Run → fail.**

- [ ] **Step 3: Implement** `state.rs`/`action.rs`/`update.rs`. `update` é um `match action` que muta `state` e devolve follow-up actions (`Vec<Action>`, ex: `Quit`→push nada mas seta should_quit; `Refresh`→status=Loading + o loop fará o fetch). Navegação clampa `selected` em `providers.len().saturating_sub(1)`. `Key` é traduzido p/ as actions semânticas dentro do `update` (ou num helper `key_to_action`).

- [ ] **Step 4: Implement `event_loop.rs`** (VERIFICAR API EventStream 0.30):
  - `pub async fn run(ctx, terminal) -> Result`: cria `EventStream` (`ratatui::crossterm::event::EventStream::new()`), `data_tick = tokio::time::interval(60s)`, `anim_tick = interval(30ms)`, `mpsc::channel` p/ DataFetched. Loop:
    ```rust
    loop {
        terminal.draw(|f| render(&state, f))?;
        if state.should_quit { break; }
        tokio::select! {
            Some(Ok(ev)) = events.next() => for a in update(&mut state, Action::Key(key_of(ev))) { drain... },
            _ = data_tick.tick() => spawn_fetch(ctx, tx.clone()),
            Some(act) = rx.recv() => { update(&mut state, act); },
            _ = anim_tick.tick() => { update(&mut state, Action::AnimTick); },
        }
    }
    ```
  - `spawn_fetch`: `tokio::spawn`/`spawn_local`? current_thread → usar `tokio::task::spawn_local` OU fazer o fetch inline com `.await` no select arm (mais simples: no `data_tick` arm, `let q = fetch_all(&registry(), ctx).await; update(&mut state, Action::DataFetched(q));`). **Decisão: fetch inline no arm do tick** (current_thread-friendly, sem mpsc/spawn_local complexo) — o draw acontece no topo do loop a cada iteração. (Se o fetch bloquear o input por segundos, revisitar com spawn_local + mpsc; pro v1 inline é aceitável pq fetch tem timeout 10s e o loop redesenha após.)
  - `render(&state, f)` vem do `tui/render/` (T3+). Na T2, um render mínimo (abas + lista de nomes).

- [ ] **Step 5: Run → pass** (update tests) + clippy. `run_tui` (mod.rs) passa a chamar `event_loop::run`.

- [ ] **Step 6: commit** `feat(rust): tui MVU state/action/update + loop`

---

### Task 3: `render/` Dashboard (tabela) + fetch real

**Files:** Create `tui/render/{mod,dashboard,status_bar}.rs`; Modify `event_loop.rs` (fetch real). Test: insta snapshot via TestBackend.

**Interfaces:** `render::render(state: &AppState, frame: &mut Frame)` — layout: título+abas (topo) / sidebar providers (esq) / content (dir) / status bar (rodapé). Dashboard content = `Table` (provider, uso gauge-ish, reset, custo-placeholder até T4b). VERIFICAR API `Tabs`/`Table`/`Layout` 0.30 via ctx7.

- [ ] **Step 1: Write failing snapshot test**
```rust
#[test]
fn dashboard_renders_providers_table() {
    let backend = ratatui::backend::TestBackend::new(64, 20);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut state = AppState::new();
    state.providers = vec![/* 3 ProviderView fake: claude 26%, codex 1%, amp 0% */];
    state.status = FetchStatus::Loaded;
    terminal.draw(|f| crate::tui::render::render(&state, f)).unwrap();
    insta::assert_snapshot!(terminal.backend());
}
```
*(TestBackend implementa Display → `assert_snapshot!`. Construir os ProviderView fake com `ProviderQuota` mínimos via um helper de teste.)*

- [ ] **Step 2: Run → fail** (sem render). **Step 3: Implement** `render/mod.rs` (Layout vertical: linha de abas, corpo horizontal [sidebar 13 col, content], status bar) + `dashboard.rs` (Table) + `status_bar.rs` (key hints). Cores via `theme_bridge`; abas ativa = bold+underline + `┃` separador; ZERO emoji.
- [ ] **Step 4: Run → pass** (revisar o snapshot à mão — é o contrato visual; conferir ZERO emoji, alinhamento). **Step 5: `event_loop` fetch real** (inline no data_tick arm). **Step 6: clippy + commit** `feat(rust): tui dashboard + fetch`

---

### Task 4: `render/detail.rs` — detalhe do provider (Enter)

**Files:** Create `tui/render/detail.rs`; Modify `render/mod.rs` (dispatch Mode::Detail), `update.rs` se preciso. Test: snapshot.
- Detalhe (Mode::Detail): gauges por janela (5h/wk) com cor por severidade (green≥60/yellow30-59/orange10-29/red<10), lista de modelos com mini-gauge, reset time. VERIFICAR `Gauge`/`LineGauge` 0.30. Snapshot test de um provider detalhado. **Step: test→fail→impl→pass(revisar snapshot)→commit** `feat(rust): tui detalhe do provider`

---

### Task 5: Integrar custo (engine 7a) no Dashboard + detalhe  (rótulo T4b)

**Files:** Modify `event_loop.rs` (chamar `usage::aggregate`), `state.rs` (campo `usage: Option<UsageSummary>`), `render/dashboard.rs` + `detail.rs` (coluna/painel US$/R$). Test: snapshot com usage fake.
- O loop, após/junto do fetch, chama `usage::aggregate(AggregateOptions{ claude_dir: home/.claude/projects, codex_dir: home/.codex/sessions, fx_rate: settings.fx_rate, amp_meta: <do quota Amp> })` → `state.usage`. Dashboard ganha coluna "custo" (US$ + R$); detalhe ganha breakdown por modelo (tokens + US$). **Modelo desconhecido → mostra tokens, "—" no custo** (nunca inventar). Amp → mostra `amp_dollars` ($ saldo). Snapshot com `UsageSummary` fake (claude $2.10/R$11.55, codex tokens sem custo conhecido, amp $4.19). **test→fail→impl→pass(revisar)→commit** `feat(rust): tui custo no dashboard/detalhe`

---

### Task 6: `widgets/` custom (QuotaGauge / ProviderList / Sparkline)  (rótulo T5)

**Files:** Create `tui/widgets/{quota_gauge,provider_list,sparkline,key_hint}.rs`; refatorar render p/ usá-los. Test: snapshot + unit (cor por severidade).
- Widgets reutilizáveis encapsulando o estilo (gauge por severidade via `theme_bridge`, lista com `highlight_symbol(" ◆ ")` + `●`/`○`, sparkline braille). Implementar via `ratatui::widgets::Widget`/`StatefulWidget` (VERIFICAR trait 0.30) ou funções que devolvem widgets configurados. Unit test: `severity_color(pct)` → token correto. **test→fail→impl→pass→commit** `feat(rust): tui widgets custom`

---

### Task 7: Animações (gauge lerp, throbber, coalesce, pulse) + glyph mode  (rótulo T6)

**Files:** Modify `state.rs` (`AnimState` por provider: `display_ratio`), `update.rs` (`AnimTick` faz `display += (target-display)*0.20`), `render/*` (throbber no Loading, pulse `●` em <10%), `settings.rs` (+ `glyph_mode: enum {Box, Nerd}` default Box). Test: unit (lerp converge) + smoke.
- A: gauge lerp (display_ratio→target a 0.20/tick 30ms). B: `tachyonfx::coalesce` 1× por refresh (VERIFICAR API tachyonfx 0.25). C: `throbber-widgets-tui` braille Cyan só enquanto `FetchStatus::Loading`. D: pulse `●` blink ~450ms quando `remaining<10`. **Glyph mode:** box-drawing default; `Nerd` opt-in usa glyphs nerd-font (degrada — se settings pede Nerd mas é a base, ainda renderiza box). **ZERO CLOCK/emoji.** Unit: lerp após N ticks aproxima target. **test→fail→impl→pass→smoke→commit** `feat(rust): tui animações + glyph mode`

---

### Task 8: Aba Waybar (config) + `settings.fx_rate`  (rótulo T7)

**Files:** Create `tui/render/config.rs`; Modify `settings.rs` (+`fx_rate: f64` default 5.50 — raw + normalize + teste), `update.rs`/`event_loop.rs` (editar+salvar). Test: settings unit + snapshot.
- `settings.rs`: adicionar `fx_rate` ao schema (raw `Option<f64>` → default 5.50; teste de default + override). Aba Waybar edita providers/ordem/separador/modo/fx_rate via `tui-input` (VERIFICAR API 0.15) → `settings::save` → aplica via `waybar_integration::apply_waybar_integration` + `setup::reload_waybar` (reusa Plano 6). Snapshot da tela de config. **test→fail→impl→pass→commit** `feat(rust): tui aba waybar config + fx_rate`

---

### Task 9: Aba Login + `install.rs` (ensure_command/ensure_amp_cli)  (rótulo T8)

**Files:** Create `rust/src/install.rs` (DESCOPADO do Plano 6), `tui/login_spawn.rs`, `tui/render/login.rs`; Modify `lib.rs`. Test: unit (mock ProviderLogin) + smoke.
- `install.rs`: porta `ensureCommand`/`ensureAmpCli` de `src/install.ts`+`src/amp-cli.ts` (verifica `claude`/`codex`/`amp` no PATH; `ensure_amp_cli` oferece o instalador `AMP_INSTALL_COMMAND`). `login_spawn.rs`: trait `ProviderLogin::launch(id) -> Result` (mockável) + `RealLogin` (suspend: `ratatui::restore()` → spawn `claude`/`codex auth login`/`amp login` com stdio herdado → re-`init` + `terminal.clear()`). Aba Login lista providers + status (logado?) + ação de login. **Também religar** o `action_right.rs::login_stub` (Plano 5) p/ chamar `login_spawn` (ou compartilhar). Unit: `update` da aba Login com mock; smoke do spawn real. **test→fail→impl→pass→smoke→commit** `feat(rust): tui login + install.rs`

---

### Task 10: Aba History REAL (tendência tokens/custo via 7a)  (rótulo T9)

**Files:** Create `tui/render/history.rs`; Modify `event_loop.rs` (carregar `records_since`), `state.rs`. Test: snapshot.
- Usa `usage::records_since(opts, cutoff)` (7a) → bucket por dia (`ts.date()`) → sparkline/`BarChart` de tokens ou custo por dia, por provider. Janela selecionável (hoje/7d/all). VERIFICAR `Sparkline`/`BarChart` 0.30. Snapshot com records fake em 3 dias. **test→fail→impl→pass(revisar)→commit** `feat(rust): tui aba history (tendência real)`

---

### Task 11: Wiring (`menu`/no-args → TUI) + footer + help overlay + polish  (rótulo T10)

**Files:** Modify `main.rs` (`Command::Menu` + no-args TTY → `tui::run_tui`), `tui/render/status_bar.rs` + help overlay (`?` via `tui-popup`). Test: snapshot do help + smoke end-to-end.
- `main.rs`: o arm `Command::Menu` chama `tui::run_tui(&ctx).await` (substitui o stub); ALÉM disso, no dispatch final, `agent-bar` sem args + stdout TTY → abre a TUI (em vez de help) — manter `--help`/`help` p/ ajuda. (O Ctx já existe no main.) Footer de atalhos contextual + overlay de ajuda (`?`) via `tui-popup` (VERIFICAR API 0.7). Polish geral. Snapshot do help overlay. **Smoke end-to-end:** rodar a TUI real, navegar abas, abrir detalhe, ver custo, sair limpo (terminal restaurado). **test→fail→impl→pass→smoke→commit** `feat(rust): tui wiring + help + polish`

---

## Self-Review (autor)

**1. Spec coverage (§4b + §5 tasks):** Dashboard+detalhe+anim → T3/T4/T6; custo US$/R$ → T4b; widgets → T5; config → T7; login → T8; History real (records_since) → T9; menu/no-args→TUI → T10; theme compartilhado → T1; MVU testável → T2. install.rs (login) → T8. fx_rate → T7. Glyph mode → T6. ✓
**2. Placeholder scan:** os render/widgets dizem "VERIFICAR API 0.30 via ctx7" — é diretiva consciente (não chutar API externa), não TODO escondido; os tipos puros (state/action/update/theme_bridge) têm código exato. As partes visuais são validadas por snapshot.
**3. Type consistency:** `AppState`/`Action`/`Tab`/`FetchStatus`/`ProviderView` definidos T2, consumidos T3-T10. `usage::{aggregate,records_since,AggregateOptions,UsageSummary}` (do 7a) consumidos T4b/T9. `theme_bridge` T1. `settings.fx_rate` T7 (consumido T4b — **nota:** T4b usa `settings.fx_rate` que só existe após T7; ORDEM: ou mover fx_rate p/ antes do T4b, ou T4b usa default 5.50 hardcoded até T7. **Decisão:** T4b usa `fx_rate` via um default const 5.50 até T7 ligar settings; documentar). 

**Riscos p/ o reviewer / execução:**
- **R1 — API ratatui 0.30:** versão recente; tutoriais antigos divergem. T1 estabelece o baseline compilável; implementers DEVEM verificar via ctx7. Se uma crate da stack não compilar com 0.30 (ex tachyonfx/tui-popup), a T que a usa adapta (a animação/popup é incremental, não bloqueia o core).
- **R2 — fetch inline no current_thread** (T2): se o fetch (até 10s) travar o input, revisitar com `spawn_local`+mpsc. Aceitável no v1.
- **R3 — snapshots visuais:** revisar CADA um à mão (ZERO emoji, alinhamento). Nunca `insta accept` cego.
- **R4 — ordem fx_rate** (T4b antes de T7): T4b usa default 5.50; T7 liga o settings. Sem bloqueio.
- **R5 — login spawn** suspende/restaura o terminal: testar smoke real (o mock cobre a lógica, não o terminal).
