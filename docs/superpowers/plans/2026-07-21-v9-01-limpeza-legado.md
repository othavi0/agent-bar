# Limpeza de legado (v9 — PR 1/Seção G) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remover código morto verificado adversarialmente (Seção G do spec) antes do
contrato `windowKind` (PR 2) pousar em cima — `Command::Terminal`,
`get_all_provider_ids`, `install::ensure_amp_cli`, `AMP_INSTALL_COMMAND`
duplicada, dep `tokio-util`, `ConfigField::settings_key`, 7 variantes órfãs de
`Icon`, comentário morto em `render/shared.rs`, migração settings v2→v3
dropando `waybar.show_percentage`, e housekeeping do worktree/branch mergeados
de `feat/omarchy-settings-cli`.

**Architecture:** Sequência de remoções cirúrgicas, cada uma com baseline
verde → edit → verificação focada → commit. Zero mudança de comportamento
observável (Waybar/TUI/CLI); só reduz superfície. Task 7 é a única com um
teste genuinamente novo (TDD completo); as demais são remoção de código morto
confirmada por grep prévio (zero outros callers).

**Tech Stack:** Rust 2021 (`agent-bar` crate + bin `src/main.rs`), cargo
test/clippy, `insta` (não tocado nesta PR), git worktree.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-21-omarchy-first-popup-redesign-design.md`
  Seção G (fonte de verdade desta PR).
- Rust/cargo only; `scripts/agent-bar-open-terminal` permanece Bash (CLAUDE.md §1).
- **Nunca `unwrap()`/`expect()` em produção** — nenhuma task adiciona código
  novo de produção, só remove; se alguma remoção deixar um `unwrap` órfão
  exposto, propagar erro em vez de silenciar.
- Provider error strings = contrato verbatim — **não alterar** (nenhuma task
  toca strings de erro de provider).
- Claude continua **fora** de `BaseProvider` (não tocado nesta PR).
- XML-escape **só** em `render_pango.rs` (não tocado nesta PR).
- Sem round-trip JSONC via serde em `waybar_integration` (não tocado nesta PR).
- stdout limpo no path Waybar; logs em stderr (não tocado nesta PR).
- Não mutar desktop ao vivo (`setup`/`update`/`uninstall` sem aprovação);
  testes com temp dirs + `XDG_*`.
- Conventional Commits em PT, subject ≤ 50 chars. **Zero atribuição de AI**
  em commits/PRs.
- Gotcha RTK: **um único filtro posicional** por invocação de `cargo test`.
  `check-types` não existe; verificação = `cargo test <filtro>` +
  `cargo clippy --all-targets -- -D warnings`.
- Implementers: Read cada arquivo antes de Edit; se Edit falhar com
  `string not found`, re-Read antes de re-tentar; após git pull/checkout/
  retorno de outro agente, o Read anterior está morto — re-Read; rodar
  `cargo test` com cache limpo antes de commit se algo parecer "PASS velho".
- Esta é a **Task 1 de 5 PRs em sequência** (Entrega do spec). PRs 2-5
  (contrato `windowKind`, Widget.qml, gates de plataforma, docs) assumem que
  esta PR já landou em `master`.

## Ordem e dependências

```text
T1 Command::Terminal ──┐
T2 get_all_provider_ids ──┤
T3 install::ensure_amp_cli ──┼──► (independentes entre si; série no mesmo branch)
T4 AMP_INSTALL_COMMAND dup ──┤
T5 dep tokio-util ──┘
T6 higiene TUI (settings_key/Icon/comentário) ──► depende de T1-T5 landarem (mesmo arquivo main.rs/cli.rs intocado, mas evita conflito de diff)
T7 settings v2→v3 (show_percentage) ──► após T6 (toca tui/render + tui/update, mesma área)
T8 housekeeping worktree/branch ──► último (não toca código)
```

T1-T5 são mutuamente independentes (arquivos diferentes) mas rodam em série
no mesmo branch para manter o histórico de commits limpo e evitar qualquer
conflito incidental. T6 e T7 tocam árvores adjacentes (`tui/`) e rodam depois.
T8 é puro housekeeping de git, sem tocar `src/`.

## Produces (para as PRs 2-5 consumirem)

- `Command` (src/cli.rs) sem a variante `Terminal` — PR 2/3 não referenciam
  `Command::Terminal`.
- `waybar_contract.rs` sem `get_all_provider_ids` — nenhuma PR seguinte
  precisa dela (Seção A do spec não a menciona).
- `install.rs` sem `ensure_amp_cli`; `providers::amp_cli` sem
  `AMP_INSTALL_COMMAND` duplicada (única fonte: `app_identity::AMP_INSTALL_COMMAND`).
- `Cargo.toml`/`Cargo.lock` sem `tokio-util`.
- `settings::CURRENT_VERSION == 3`; `settings::Waybar` sem `show_percentage`
  — PR 2/3 (contrato `windowKind`, `config show` com `menuAnimations`) partem
  desta forma já enxuta do struct `Waybar`.
- `tui::state::ConfigField` sem `settings_key()`; `tui::widgets::icons::Icon`
  só com `{Ok, LoggedOut, Warn, NoToken}`.

---

### Task 1: Remover `Command::Terminal`

**Files:**
- Modify: `src/cli.rs:15` (variante do enum `Command`)
- Modify: `src/main.rs:761` (match arm), `src/main.rs:894-904` (teste
  `should_notify_false_when_command_is_terminal`)

**Interfaces:**
- Consumes: nenhum (variante sem nenhum caller de `parse_args` — só o alias
  `--terminal`/`-t` mapeia para `Command::Status`, comentário em
  `cli.rs:274` já documenta isso).
- Produces: `Command` com 17 variantes em vez de 18 (Waybar, Menu, Status,
  Help, Version, ActionRight, Setup, AssetsInstall, ExportWaybarModules,
  ExportWaybarCss, Update, Uninstall, Remove→Uninstall+yes, Doctor,
  MenuFont, ConfigShow, ConfigApply).

- [ ] **Step 1: Baseline verde**

```bash
cargo test cli
```

Expected: `test result: ok` (todos os testes de `src/cli.rs::tests` passam,
baseline limpa antes da remoção).

- [ ] **Step 2: Ajustar o teste que usa `Command::Terminal` antes de remover a variante**

Em `src/main.rs`, o teste atual (linhas 894-904):

```rust
    #[test]
    fn should_notify_false_when_command_is_terminal() {
        let s = settings_with_notify(true);
        assert!(!should_notify(
            &s,
            Command::Terminal,
            Format::Waybar,
            false,
            false
        ));
    }
```

`should_notify` só retorna `true` quando `command == Command::Waybar`
(`src/main.rs:44`); qualquer outra variante cobre o mesmo caminho. Trocar
para `Command::Menu` (variante já testada em outros pontos do arquivo, sem
relação com Waybar) e renomear o teste:

```rust
    #[test]
    fn should_notify_false_when_command_is_not_waybar() {
        let s = settings_with_notify(true);
        assert!(!should_notify(
            &s,
            Command::Menu,
            Format::Waybar,
            false,
            false
        ));
    }
```

- [ ] **Step 3: Remover o match arm em `main.rs`**

Em `src/main.rs:761`:

```rust
        Command::Terminal | Command::Status => {
```

vira:

```rust
        Command::Status => {
```

- [ ] **Step 4: Remover a variante do enum em `cli.rs`**

Em `src/cli.rs`, o enum `Command` (linhas 12-37) tem `Terminal` na linha 15:

```rust
pub enum Command {
    Waybar,
    Terminal,
    Menu,
```

Remove a linha `Terminal,`:

```rust
pub enum Command {
    Waybar,
    Menu,
```

- [ ] **Step 5: Verificar**

```bash
cargo test cli
```

Expected: `test result: ok`.

```bash
cargo test should_notify
```

Expected: `test result: ok` (inclui o teste renomeado no Step 2).

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: zero warnings (nenhum match não-exaustivo quebra — ambos os
matches de `Command` em `main.rs` têm `_ => {}`/wildcard).

- [ ] **Step 6: Commit**

```bash
git add src/cli.rs src/main.rs
git status  # confirmar só os 2 arquivos
git commit -m "$(cat <<'EOF'
refactor: remove Command::Terminal morto
EOF
)"
```

---

### Task 2: Remover `waybar_contract::get_all_provider_ids`

**Files:**
- Modify: `src/waybar_contract.rs:102-117` (bloco de comentário de seção +
  função `get_all_provider_ids`)

**Interfaces:**
- Consumes: nenhum caller em `src/` (confirmado por grep — só a própria
  definição).
- Produces: nada (função pública sem uso; remoção não afeta API consumida
  por outras PRs — Seção A/E do spec não a referenciam).

- [ ] **Step 1: Baseline verde**

```bash
cargo test waybar_contract
```

Expected: `test result: ok`.

- [ ] **Step 2: Remover a função**

Em `src/waybar_contract.rs`, remover o bloco (linhas 102-117):

```rust
// ---------------------------------------------------------------------------
// getAllProviderIds
// ---------------------------------------------------------------------------

/// Todos os provider ids conhecidos — built-in + registrados sem duplicatas.
/// Espelha `getAllProviderIds` do TS.
pub fn get_all_provider_ids() -> Vec<String> {
    let mut ids: Vec<String> = WAYBAR_PROVIDERS.iter().map(|s| s.to_string()).collect();
    for id in crate::providers::registered_provider_ids() {
        let id_str = id.to_string();
        if !ids.contains(&id_str) {
            ids.push(id_str);
        }
    }
    ids
}

```

(A seção `// CSS export` que vem logo depois permanece intacta.)

- [ ] **Step 3: Verificar**

```bash
cargo test waybar_contract
```

Expected: `test result: ok` (nenhum teste exercitava `get_all_provider_ids`
diretamente — grep prévio não encontrou `#[test]` referenciando-a).

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: zero warnings.

- [ ] **Step 4: Commit**

```bash
git add src/waybar_contract.rs
git commit -m "$(cat <<'EOF'
refactor: remove get_all_provider_ids sem uso
EOF
)"
```

---

### Task 3: Remover `install::ensure_amp_cli` + atualizar comentário de módulo

**Files:**
- Modify: `src/install.rs:1-9` (doc comment do módulo + import),
  `src/install.rs:28-42` (função `ensure_amp_cli`),
  `src/install.rs:76-82` (teste `ensure_amp_cli_absent_returns_false`)

**Interfaces:**
- Consumes: nenhum caller de `ensure_amp_cli` em `src/` fora do próprio
  módulo (confirmado por grep — o locator real e usado em produção é
  `providers::amp_cli::find_amp_bin`, não `install::ensure_amp_cli`).
- Produces: `install.rs` mantém `has_cmd`/`ensure_command` (usados e
  testados) — só a função de guidance do Amp sai.

- [ ] **Step 1: Baseline verde**

```bash
cargo test install
```

Expected: `test result: ok`.

- [ ] **Step 2: Remover a função e seu teste**

Em `src/install.rs`, remover a função (linhas 28-42):

```rust
/// Verifica se `amp` esta disponivel. Se ausente, loga o comando de instalacao
/// oficial e retorna `false`. Nao executa o instalador automaticamente.
///
/// Para instalacao real, o usuario deve rodar manualmente:
/// `curl -fsSL https://ampcode.com/install.sh | bash`
pub fn ensure_amp_cli() -> bool {
    if has_cmd("amp") {
        return true;
    }
    log::warn!(
        "Amp CLI não encontrado. Para instalar, rode: {}",
        AMP_INSTALL_COMMAND
    );
    false
}

```

E o teste correspondente (linhas 76-82, dentro de `mod tests`):

```rust
    #[test]
    fn ensure_amp_cli_absent_returns_false() {
        let _env = crate::test_support::env_guard();
        // Em ambiente CI/test, amp provavelmente nao esta instalado.
        // Se estiver, o test passa com true; se nao, com false. Ambos sao corretos.
        let _ = ensure_amp_cli(); // apenas verifica que nao panica
    }

```

- [ ] **Step 3: Remover o import agora não-usado**

`AMP_INSTALL_COMMAND` só era usado dentro de `ensure_amp_cli`. Remover a
linha 9:

```rust
use crate::app_identity::AMP_INSTALL_COMMAND;
```

- [ ] **Step 4: Atualizar o doc comment do módulo**

O comentário atual (linhas 1-8):

```rust
//! Verificacao de presenca de comandos no PATH. Port de `src/install.ts` +
//! a parte `ensure_amp_cli` de `src/amp-cli.ts`.
//!
//! NOTA: `ensure_amp_cli` aqui **orienta** o usuario (imprime o comando de
//! instalacao) em vez de executar `curl | bash` automaticamente. O instalador
//! interativo foi descartado por seguranca: a TUI nao deve pipe-executar codigo
//! remoto sem confirmacao explicita do usuario.
```

vira (reflete que a função foi removida por não ter caller — o locator real
é `providers::amp_cli::find_amp_bin`):

```rust
//! Verificacao de presenca de comandos no PATH. Port de `src/install.ts`.
//!
//! `ensure_amp_cli` (guidance de instalacao do Amp, port de `src/amp-cli.ts`)
//! foi removida (v9, limpeza de legado) por nao ter caller: o locator real
//! em producao e `providers::amp_cli::find_amp_bin`.
```

- [ ] **Step 5: Verificar**

```bash
cargo test install
```

Expected: `test result: ok` (`has_cmd_finds_sh`, `has_cmd_misses_nonexistent`,
`ensure_command_true_when_present`, `ensure_command_false_when_absent`
continuam passando).

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: zero warnings (import não-usado removido no Step 3 evita erro de
clippy).

- [ ] **Step 6: Commit**

```bash
git add src/install.rs
git commit -m "$(cat <<'EOF'
refactor: remove install::ensure_amp_cli sem uso
EOF
)"
```

---

### Task 4: Remover `amp_cli::AMP_INSTALL_COMMAND` duplicada

**Files:**
- Modify: `src/providers/amp_cli.rs:7-9` (comentário + const),
  `src/providers/amp_cli.rs:106-112` (teste `install_command_is_official`)

**Interfaces:**
- Consumes: `app_identity::AMP_INSTALL_COMMAND` (já existe e já é a fonte
  usada por `src/install.rs` e `src/tui/login_spawn.rs:109` — esta task só
  faz `providers::amp_cli` parar de ter a sua própria cópia).
- Produces: `providers::amp_cli` sem constante própria; único
  `AMP_INSTALL_COMMAND` do crate vive em `app_identity.rs:15`.

- [ ] **Step 1: Baseline verde**

```bash
cargo test providers::amp_cli
```

Expected: `test result: ok` (5 testes: `candidate_paths_under_home`,
`empty_home_yields_no_candidates`, `prefers_path_when_available`,
`falls_back_to_known_locations`, `none_when_unavailable`,
`install_command_is_official`).

- [ ] **Step 2: Apontar o teste para `app_identity` antes de remover a const local**

Em `src/providers/amp_cli.rs`, o teste atual (linhas 106-112):

```rust
    #[test]
    fn install_command_is_official() {
        assert_eq!(
            AMP_INSTALL_COMMAND,
            "curl -fsSL https://ampcode.com/install.sh | bash"
        );
    }
```

vira (import explícito da constante canônica; `use super::*` não traz mais
`AMP_INSTALL_COMMAND` depois do Step 3):

```rust
    #[test]
    fn install_command_is_official() {
        use crate::app_identity::AMP_INSTALL_COMMAND;
        assert_eq!(
            AMP_INSTALL_COMMAND,
            "curl -fsSL https://ampcode.com/install.sh | bash"
        );
    }
```

- [ ] **Step 3: Remover a constante duplicada**

Em `src/providers/amp_cli.rs`, remover (linhas 7-9):

```rust
/// Comando oficial de instalação (usado pelo Plano 6; contrato de display).
pub const AMP_INSTALL_COMMAND: &str = "curl -fsSL https://ampcode.com/install.sh | bash";

```

- [ ] **Step 4: Verificar**

```bash
cargo test providers::amp_cli
```

Expected: `test result: ok`.

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: zero warnings.

- [ ] **Step 5: Commit**

```bash
git add src/providers/amp_cli.rs
git commit -m "$(cat <<'EOF'
refactor: unifica AMP_INSTALL_COMMAND
EOF
)"
```

---

### Task 5: Remover dep `tokio-util`

**Files:**
- Modify: `Cargo.toml:36` (linha da dependência)
- Modify: `Cargo.lock` (regenerado por `cargo build`, não editado a mão)

**Interfaces:**
- Consumes: nada — grep prévio confirma zero `use tokio_util` em `src/`.
- Produces: `Cargo.lock` sem a entrada `tokio-util` (e suas transitivas
  exclusivas, se houver).

- [ ] **Step 1: Baseline verde**

```bash
cargo build
```

Expected: build limpo (baseline antes de tocar `Cargo.toml`).

- [ ] **Step 2: Remover a dependência**

Em `Cargo.toml`, remover a linha 36:

```toml
tokio-util = { version = "0.7", features = ["rt"] }
```

- [ ] **Step 3: Regenerar o lockfile**

```bash
cargo build
```

Expected: build limpo; `git diff Cargo.lock` mostra a remoção do pacote
`tokio-util` (e nada mais — se outras entradas mudarem de versão por
resolução de deps, parar e investigar antes de prosseguir).

- [ ] **Step 4: Verificar**

```bash
cargo test cli
```

Expected: `test result: ok` (smoke de que o crate ainda compila/roda testes
após a mudança de dependências).

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: zero warnings.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock
git status  # confirmar só Cargo.toml + Cargo.lock
git commit -m "$(cat <<'EOF'
chore: remove dependência tokio-util sem uso
EOF
)"
```

---

### Task 6: Higiene TUI — `settings_key`, ícones órfãos, comentário morto

**Files:**
- Modify: `src/tui/state.rs:119-130` (método `ConfigField::settings_key`)
- Modify: `src/tui/widgets/icons.rs` (enum `Icon`, `match` em `glyph`, teste
  `nerd_and_box_glyphs_differ_and_are_nonempty`)
- Modify: `src/tui/render/shared.rs:1-11` (doc comment do módulo)

**Interfaces:**
- Consumes: nenhum — grep prévio confirma zero callers de
  `ConfigField::settings_key()`; só 4 dos 11 variants de `Icon` têm caller
  (`Ok`, `LoggedOut`, `Warn`, `NoToken`, todos em `src/tui/render/login.rs` e
  `src/tui/render/detail/states.rs:49`).
- Produces: `Icon` com 4 variantes; `ConfigField` sem `settings_key`;
  `render/shared.rs` com doc comment que reflete o estado real (consumidor
  de `series_now` é `history.rs`, não `detail.rs` — `dashboard.rs` já não
  existe).

- [ ] **Step 1: Baseline verde**

```bash
cargo test tui::
```

Expected: `test result: ok`.

- [ ] **Step 2: Remover `ConfigField::settings_key`**

Em `src/tui/state.rs`, remover o método (linhas 119-130):

```rust
    /// Chave técnica (settings / docs) — dica no painel de ajuda.
    pub fn settings_key(self) -> &'static str {
        match self {
            ConfigField::Providers => "providers",
            ConfigField::ProviderOrder => "providerOrder",
            ConfigField::Separators => "separators",
            ConfigField::DisplayMode => "displayMode",
            ConfigField::Signal => "signal",
            ConfigField::Interval => "interval",
            ConfigField::FxRate => "fxRate",
        }
    }
```

(O método `label()` logo acima, usado pelo render, permanece intacto.)

- [ ] **Step 3: Remover as 7 variantes órfãs de `Icon` e seus braços em `glyph`**

Em `src/tui/widgets/icons.rs`, o enum atual:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Ok,
    LoggedOut,
    Warn,
    NoToken,
    Reset,
    Cost,
    History,
    Peak,
    Refresh,
    Login,
    Waybar,
}
```

vira (só as 4 variantes com caller real):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Ok,
    LoggedOut,
    Warn,
    NoToken,
}
```

E a função `glyph` atual:

```rust
pub fn glyph(icon: Icon, mode: GlyphMode) -> &'static str {
    match (icon, mode) {
        (Icon::Ok, GlyphMode::Nerd) => "\u{f00c}",
        (Icon::Ok, GlyphMode::Box) => "✓",
        (Icon::LoggedOut, GlyphMode::Nerd) => "\u{f00d}",
        (Icon::LoggedOut, GlyphMode::Box) => "✗",
        (Icon::Warn, GlyphMode::Nerd) => "\u{f071}",
        (Icon::Warn, GlyphMode::Box) => "!",
        (Icon::NoToken, GlyphMode::Nerd) => "\u{f023}",
        (Icon::NoToken, GlyphMode::Box) => "×",
        (Icon::Reset, GlyphMode::Nerd) => "\u{f017}",
        (Icon::Reset, GlyphMode::Box) => "↻",
        (Icon::Cost, GlyphMode::Nerd) => "\u{f155}",
        (Icon::Cost, GlyphMode::Box) => "$",
        (Icon::History, GlyphMode::Nerd) => "\u{f201}",
        (Icon::History, GlyphMode::Box) => "≡",
        (Icon::Peak, GlyphMode::Nerd) => "\u{f0e7}",
        (Icon::Peak, GlyphMode::Box) => "▲",
        (Icon::Refresh, GlyphMode::Nerd) => "\u{f021}",
        (Icon::Refresh, GlyphMode::Box) => "↻",
        (Icon::Login, GlyphMode::Nerd) => "\u{f090}",
        (Icon::Login, GlyphMode::Box) => "→",
        (Icon::Waybar, GlyphMode::Nerd) => "\u{f013}",
        (Icon::Waybar, GlyphMode::Box) => "⚙",
    }
}
```

vira:

```rust
pub fn glyph(icon: Icon, mode: GlyphMode) -> &'static str {
    match (icon, mode) {
        (Icon::Ok, GlyphMode::Nerd) => "\u{f00c}",
        (Icon::Ok, GlyphMode::Box) => "✓",
        (Icon::LoggedOut, GlyphMode::Nerd) => "\u{f00d}",
        (Icon::LoggedOut, GlyphMode::Box) => "✗",
        (Icon::Warn, GlyphMode::Nerd) => "\u{f071}",
        (Icon::Warn, GlyphMode::Box) => "!",
        (Icon::NoToken, GlyphMode::Nerd) => "\u{f023}",
        (Icon::NoToken, GlyphMode::Box) => "×",
    }
}
```

- [ ] **Step 4: Ajustar o teste de glyphs**

Em `src/tui/widgets/icons.rs`, o teste atual itera as 11 variantes; ajustar
para as 4 restantes:

```rust
    #[test]
    fn nerd_and_box_glyphs_differ_and_are_nonempty() {
        for icon in [
            Icon::Ok,
            Icon::LoggedOut,
            Icon::Warn,
            Icon::NoToken,
        ] {
            assert!(!glyph(icon, GlyphMode::Nerd).is_empty());
            assert!(!glyph(icon, GlyphMode::Box).is_empty());
        }
        // Nerd usa PUA (>= U+E000); Box nunca:
        assert!(glyph(Icon::Ok, GlyphMode::Nerd)
            .chars()
            .all(|c| c as u32 >= 0xE000));
        assert!(glyph(Icon::Ok, GlyphMode::Box)
            .chars()
            .all(|c| (c as u32) < 0xE000));
    }
```

- [ ] **Step 5: Atualizar o comentário morto em `render/shared.rs`**

Em `src/tui/render/shared.rs`, o doc comment atual (linhas 1-11):

```rust
//! Helpers compartilhados entre telas de render. `series_now` era usada por
//! `dashboard.rs` (apagado na Task 11 junto com o Overview) e por
//! `detail.rs` (Task 12) — ambas as telas precisavam da MESMA âncora
//! temporal pra série real de 24h, então a lógica mora aqui em vez de
//! duplicada em cada módulo de tela; `detail.rs` continua consumindo.
//!
//! `abbrev_tokens` (formatador de tokens com ponto decimal) morou aqui até
//! o fix gate de dados reais — removido: `detail.rs` unificou pra
//! `column_chart::fmt_tokens_short` (vírgula decimal), a mesma usada pelas
//! legendas do chart e por `history.rs`, pra não ter dois formatos de
//! número na mesma tela (`719.6M` vs `719,6M`).
```

vira (o consumidor real hoje é `history.rs`, não `detail.rs`; `dashboard.rs`
já não existe no repo):

```rust
//! Helpers compartilhados entre telas de render. `series_now` calcula a
//! âncora temporal da série real de 24h; `history.rs` é quem consome hoje
//! (`render_chart_section` do chart de History).
//!
//! `abbrev_tokens` (formatador de tokens com ponto decimal) morou aqui até
//! o fix gate de dados reais — removido: `detail.rs` unificou pra
//! `column_chart::fmt_tokens_short` (vírgula decimal), a mesma usada pelas
//! legendas do chart e por `history.rs`, pra não ter dois formatos de
//! número na mesma tela (`719.6M` vs `719,6M`).
```

- [ ] **Step 6: Verificar**

```bash
cargo test tui::widgets::icons
```

Expected: `test result: ok` (1 teste, `nerd_and_box_glyphs_differ_and_are_nonempty`).

```bash
cargo test tui::
```

Expected: `test result: ok` (nenhuma outra tela quebrou — `settings_key` e
os 7 ícones removidos não tinham caller).

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: zero warnings.

- [ ] **Step 7: Commit**

```bash
git add src/tui/state.rs src/tui/widgets/icons.rs src/tui/render/shared.rs
git commit -m "$(cat <<'EOF'
refactor: remove settings_key e ícones órfãos
EOF
)"
```

---

### Task 7: Settings v2→v3 — drop de `waybar.show_percentage`

**Files:**
- Modify: `src/settings.rs:9` (`CURRENT_VERSION`), `src/settings.rs:97`
  (campo em `Waybar`), `src/settings.rs:168` (campo em `RawWaybar`),
  `src/settings.rs:302` (uso em `normalize`), `src/settings.rs:391`
  (teste `defaults_when_no_file`, assert de `version`)
- Modify: `src/tui/render/config.rs:281`, `src/tui/render/mod.rs:296`,
  `src/tui/update/mod.rs:701`, `src/tui/update/navigation.rs:172` (fixtures
  `Waybar { .. }` com `show_percentage: true,`)

**Interfaces:**
- Consumes: nenhum — grep prévio confirma que `show_percentage` só é lido
  em `settings.rs` (schema) e escrito nas 4 fixtures de teste acima; nenhum
  formatter/render usa o valor para decidir exibição (o nome sobreviveu do
  design pré-v8, nunca ligado a comportamento).
- Produces: `Settings::version == 3` por padrão; `Waybar` (schema tipado e
  JSON serializado por `config show`) sem `showPercentage`. Auto-repair
  existente em `settings::load` (linhas 344-348, compara `norm_value` com
  `raw_value` e resalva) já cobre a migração — nenhuma lógica condicional
  por versão é necessária.

- [ ] **Step 1: Baseline verde**

```bash
cargo test settings
```

Expected: `test result: ok`.

- [ ] **Step 2: Escrever o teste que falha — migração dropa a chave**

Em `src/settings.rs`, dentro de `mod tests`, adicionar (após
`fn keeps_valid_signal_and_separator`, antes de
`fn provider_selection_filters_dedups_and_orders`):

```rust
    #[test]
    fn v2_settings_drops_show_percentage_on_load() {
        let dir = tempdir().unwrap();
        let p = paths_in(dir.path());
        std::fs::create_dir_all(&p.config_dir).unwrap();
        std::fs::write(
            p.settings_file(),
            r#"{"version":2,"waybar":{"providers":["claude"],"showPercentage":false,
                "separators":"gap","providerOrder":["claude"],"displayMode":"remaining",
                "interval":60}}"#,
        )
        .unwrap();

        let s = load(&p);
        assert_eq!(s.version, CURRENT_VERSION);

        // Auto-repair já regravou o arquivo sem a chave legada.
        let saved = std::fs::read_to_string(p.settings_file()).unwrap();
        assert!(
            !saved.contains("showPercentage"),
            "showPercentage deveria ter sido dropada no re-save: {saved}"
        );
    }
```

- [ ] **Step 3: Rodar e ver falhar**

```bash
cargo test v2_settings_drops_show_percentage_on_load
```

Expected: FAIL — `showPercentage` ainda está no arquivo resalvo, porque
hoje `Waybar`/`RawWaybar` ainda declaram o campo (o auto-repair preserva o
valor em vez de dropá-lo).

- [ ] **Step 4: Implementação mínima — remover o campo do schema**

Em `src/settings.rs`, bump da versão (linha 9):

```rust
pub const CURRENT_VERSION: u32 = 2;
```

vira:

```rust
pub const CURRENT_VERSION: u32 = 3;
```

Remover o campo de `Waybar` (linha 97):

```rust
pub struct Waybar {
    pub providers: Vec<String>,
    pub show_percentage: bool,
    pub separators: SeparatorStyle,
```

vira:

```rust
pub struct Waybar {
    pub providers: Vec<String>,
    pub separators: SeparatorStyle,
```

Remover o campo de `RawWaybar` (linha 168):

```rust
struct RawWaybar {
    providers: Option<Vec<String>>,
    show_percentage: Option<bool>,
    separators: Option<String>,
```

vira:

```rust
struct RawWaybar {
    providers: Option<Vec<String>>,
    separators: Option<String>,
```

Remover o uso em `normalize()` (linha 302):

```rust
        waybar: Waybar {
            providers,
            show_percentage: rw.show_percentage.unwrap_or(true),
            separators,
```

vira:

```rust
        waybar: Waybar {
            providers,
            separators,
```

Atualizar o assert de versão em `defaults_when_no_file` (linha 391):

```rust
        assert_eq!(s.version, 2);
```

vira:

```rust
        assert_eq!(s.version, 3);
```

- [ ] **Step 5: Corrigir as 4 fixtures que ainda setam o campo removido**

Em `src/tui/render/config.rs:281`, `src/tui/render/mod.rs:296`,
`src/tui/update/mod.rs:701` (todas com o mesmo formato — literal
`Waybar { .. }` de teste) remover a linha:

```rust
                show_percentage: true,
```

E em `src/tui/update/navigation.rs:172` (indentação diferente, dentro do
literal `crate::settings::Waybar { .. }`):

```rust
                        show_percentage: true,
```

remover também.

- [ ] **Step 6: Rodar e ver passar**

```bash
cargo test v2_settings_drops_show_percentage_on_load
```

Expected: `test result: ok`.

```bash
cargo test settings
```

Expected: `test result: ok` (inclui `defaults_when_no_file` com
`version == 3`).

```bash
cargo test tui::
```

Expected: `test result: ok` (as 4 fixtures compilam sem `show_percentage`).

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: zero warnings.

- [ ] **Step 7: Commit**

```bash
git add src/settings.rs src/tui/render/config.rs src/tui/render/mod.rs src/tui/update/mod.rs src/tui/update/navigation.rs
git commit -m "$(cat <<'EOF'
feat: settings v3 dropa waybar.show_percentage
EOF
)"
```

---

### Task 8: Housekeeping — worktree e branch mergeados

**Files:** nenhum em `src/` — só estado de git (worktree + branches local/remota).

**Interfaces:** nenhuma (não toca código).

Pré-condição já confirmada nesta investigação: `feat/omarchy-settings-cli`
foi mergeada via PR #18 (`e616f59 Merge pull request #18 from
othavi0/feat/omarchy-settings-cli`), o worktree
`.worktrees/omarchy-settings-cli` está limpo (`git status --short` vazio) e
`git branch --merged master` lista a branch. Seguro remover.

- [ ] **Step 1: Confirmar pré-condição (idempotente, só leitura)**

```bash
git -C .worktrees/omarchy-settings-cli status --short
git branch --merged master | grep omarchy-settings-cli
```

Expected: primeiro comando sem saída (working tree limpa); segundo comando
lista `feat/omarchy-settings-cli`. Se qualquer um divergir (mudanças não
commitadas, ou branch não mergeada), **parar e reportar BLOCKED** — não
prosseguir com a remoção.

- [ ] **Step 2: Remover o worktree**

```bash
git worktree remove .worktrees/omarchy-settings-cli
```

Expected: comando silencioso (sem erro); `git worktree list` não lista mais
o path.

- [ ] **Step 3: Remover a branch local**

```bash
git branch -d feat/omarchy-settings-cli
```

Expected: `Deleted branch feat/omarchy-settings-cli (was 163b1b4)`. `-d`
(minúsculo, não `-D`) recusa se a branch não estiver mergeada — confirmação
adicional de segurança.

- [ ] **Step 4: Remover a branch remota**

```bash
git push origin --delete feat/omarchy-settings-cli
```

Expected: ` - [deleted]         feat/omarchy-settings-cli`.

- [ ] **Step 5: Verificar**

```bash
git worktree list
git branch -a --list "*omarchy-settings-cli*"
```

Expected: nenhuma linha menciona `omarchy-settings-cli` em nenhum dos dois
comandos.

- [ ] **Step 6: Sem commit de código** — esta task não gera diff em `src/`;
  nada a commitar além do estado de git já refletido em `git worktree
  list`/`git branch -a` (não há arquivo para `git add`).

---

## Gate final da PR 1

- [ ] **Rodar a verificação ampla antes de abrir PR/mergear**

```bash
cargo test
```

Expected: `test result: ok` em todos os targets (lib + bin `agent-bar`).

```bash
cargo clippy --all-targets -- -D warnings
```

Expected: zero warnings.

```bash
git status --short
```

Expected: working tree limpa (todas as 7 tasks de código já commitadas
individualmente; Task 8 não gera diff de arquivo).

- [ ] **Confirmar zero regressão de contrato**

```bash
cargo test --test golden
cargo test waybar_contract
```

Expected: `test result: ok` — nenhuma das remoções desta PR toca o payload
Waybar/golden (só código morto saiu).

Após o gate verde, invocar
`superpowers:finishing-a-development-branch` para decidir merge/PR — não
mergear/abrir PR sem esse checkpoint.
