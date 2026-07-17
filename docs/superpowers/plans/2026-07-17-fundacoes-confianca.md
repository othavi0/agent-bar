# Fundações e confiança (trilha A) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Modularizar monólitos (Codex, TUI update, Detail) sem mudar comportamento; endurecer contratos Amp/Waybar e limpar residual TS na docs — conforme `docs/superpowers/specs/2026-07-17-fundacoes-confianca-design.md`.

**Architecture:** Strangler modular em PRs/tasks independentes A1→A4. Splits = move de código + reexports (`pub use` / `pub(crate)`), testes e snapshots idênticos. A4 adiciona fixtures, helper de print Waybar com falha barulhenta, e docs. Sem novas crates.

**Tech Stack:** Rust 2021, tokio, ratatui, insta, cargo test/clippy. Sem Node.

## Global Constraints

- Spec: `docs/superpowers/specs/2026-07-17-fundacoes-confianca-design.md` (fonte de verdade).
- Rust/cargo only; `scripts/agent-bar-open-terminal` permanece Bash (CLAUDE.md §1).
- **Nunca `unwrap()`/`expect()` em produção** — `deny(clippy::unwrap_used)` (CLAUDE.md §3). Em `#[cfg(test)]` ok.
- Provider error strings = contrato verbatim — **não alterar** (CLAUDE.md §3).
- Claude continua **fora** de `BaseProvider` (CLAUDE.md §3).
- XML-escape **só** em `render_pango.rs`.
- Sem round-trip JSONC via serde em `waybar_integration`.
- stdout limpo no path Waybar; logs em stderr.
- Não mutar desktop ao vivo (`setup`/`update`/`uninstall` sem aprovação); testes com temp dirs + `XDG_*`.
- Conventional Commits em PT, subject ≤ 50 chars.
- **Zero atribuição de AI** em commits/PRs.
- Gotcha RTK: um único filtro posicional por `cargo test`.
- Tasks A1–A3: **diff de comportamento zero** (snapshots/golden idênticos).
- Implementers: Read cada arquivo antes de Edit; se Edit falhar, re-Read; após outro agente, re-Read; rodar testes da fatia antes de declarar DONE.
- **Não tocar** mudanças alheias no worktree do usuário (amp/history/detail sujos no master original). Trabalhar em branch/worktree limpa a partir do commit da spec/plano.

## Ordem e dependências

```text
T1 Codex split  ──┐
T2 tui/update   ──┼──► T4 Amp fixtures ──► T5 Waybar print ──► T6 docs ──► T7 gate
T3 detail split ──┘
```

T1, T2, T3 são independentes entre si; na prática rodam **em série** (mesmo branch) na ordem T1→T2→T3 para evitar conflitos de merge e facilitar review. T4–T6 em série após. T7 fecha.

**Não implementar A5** (cli/update lifecycle) neste plano.

## File map (estado final)

```text
src/providers/codex/
  mod.rs           # CodexProvider, QuotaSource, Provider, reexports públicos
  types.rs         # CodexWindowRaw, CodexLimitBucket, CodexCredits, CodexRateLimits
  normalize.rs     # unix_to_iso, to_quota_window, labels, build_codex_quota, pick_*
  app_server.rs    # tipos camelCase app-server, normalize_appserver_*, run_appserver_protocol, fetch_via_appserver
  session_log.rs   # find_latest_session_file, extract_rate_limits

src/tui/update/
  mod.rs           # pub fn update; key_to_action reexport se público; dispatcher
  navigation.rs    # sidebar, screens, help, quit, key_to_action_with_state (nav parts)
  config.rs        # Config* actions, field_value_string, apply_field_edit
  login.rs         # Login*, login_selected_id
  fetch.rs         # ProviderFetched, refresh, fetch status
  history.rs       # History*, range, expand day

src/tui/render/detail/
  mod.rs           # render_detail, render_full, empty/skeleton orchestration
  layout.rs        # constraints + colapso (se extrair; senão fica em mod)
  format.rs        # LABEL_W, suffixes, derive_bar_width, truncate_name, fmt_*, model_tokens
  windows.rs       # window_line, model_window_line, window_lines
  chart.rs         # section_title, render_chart_section
  models.rs        # model_usage_line, model_lines
  extra.rs         # extra_usage_line, extra_lines
  totals.rs        # totals_line
  states.rs        # render_logged_out, render_error

src/formatters/waybar.rs  # (T5) opcional: helper serialize_waybar_line — preferir helper em main ou waybar.rs
tests/fixtures/amp/
  usage-legacy-dollars.txt
  usage-free-pct.txt
docs/architecture.md      # paths Rust, sem index.ts/notify.ts vivos
```

---

### Task 1: Split `providers/codex` (A1)

**Files:**
- Create: `src/providers/codex/mod.rs`, `types.rs`, `normalize.rs`, `app_server.rs`, `session_log.rs`
- Delete: `src/providers/codex.rs` (substituído pelo diretório `codex/`)
- Modify: nenhum import externo se `providers::codex` continuar resolvendo (Rust: `codex.rs` XOR `codex/mod.rs`)
- Test: testes que já vivem em `codex.rs` `mod tests` → `codex/mod.rs` ou arquivo do domínio

**Interfaces:**
- Produces: `pub struct CodexProvider`, `pub fn build_codex_quota(...)`, tipos `pub` já exportados hoje (`CodexRateLimits`, etc.) sob `crate::providers::codex::*`
- Consumes: `QuotaSource`, `Provider`, `Ctx`, `ProviderError`, paths em `config`/`Ctx`

**Fronteiras de move (linhas atuais de `src/providers/codex.rs` — re-Read e ajustar se drift):**

| Destino | Conteúdo (símbolos) |
| --- | --- |
| `types.rs` | `CodexWindowRaw`, `CodexLimitBucket`, `CodexCredits`, `CodexRateLimits` (~L18–57) |
| `normalize.rs` | `unix_to_iso`, `to_quota_window`, `format_bucket_label`, `place_window`, `build_model_windows`, `flatten_models`, `pick_primary`, `pick_secondary`, `build_codex_quota` (~L59–300) |
| `app_server.rs` | tipos AppServer*, `to_raw_window`, `normalize_bucket`, `normalize_appserver_rate_limits`, `write_json`, `run_appserver_protocol`, `fetch_via_appserver` (~L302–728) |
| `session_log.rs` | `find_latest_session_file`, `SessionEvent`/`SessionPayload`, `extract_rate_limits` (~L628–702) |
| `mod.rs` | `CodexProvider`, `impl QuotaSource`, `impl Provider`, `mod tests`, `pub use` dos tipos/funções públicas que testes/outros módulos usam |

- [ ] **Step 1: Baseline verde**

```bash
cargo test providers::codex
cargo test providers::base
```

Expected: PASS (registrar contagem). Se FAIL, parar e reportar BLOCKED (baseline sujo).

- [ ] **Step 2: Criar `codex/` e mover tipos**

1. Criar diretório `src/providers/codex/`.
2. Mover structs raw para `types.rs` com os mesmos derives/`pub`.
3. Em `mod.rs` temporário: `mod types; pub use types::*;` + resto ainda monólito **ou** fazer o split completo de uma vez (preferido se o agente couber o arquivo).
4. **Não** deixar `codex.rs` e `codex/` coexistirem.

- [ ] **Step 3: Mover normalize, app_server, session_log, provider**

Mover símbolos conforme tabela. Ajustar `use super::…` / `crate::providers::codex::…`. Manter `pub` exatamente onde já era `pub` (testes e callers externos). Funções que eram `pub` para testes (`build_codex_quota`, `normalize_appserver_rate_limits`, `run_appserver_protocol`, `find_latest_session_file`, `extract_rate_limits`) continuam `pub` ou `pub(crate)` se só o crate usa — **não** reduzir visibilidade se quebrar testes.

- [ ] **Step 4: Migrar `mod tests`**

Manter os mesmos asserts de string de erro e fixtures inline. Paths de teste: `cargo test providers::codex` deve continuar encontrando os testes (módulo `providers::codex::tests` ou submódulos).

- [ ] **Step 5: Verificar**

```bash
cargo test providers::codex
cargo test providers::base
cargo test --test golden
cargo clippy --all-targets -- -D warnings
```

Expected: PASS, zero mudança de snapshot golden.

- [ ] **Step 6: Commit**

```bash
git add src/providers/codex src/providers/codex.rs
git status  # confirmar só arquivos do split
git commit -m "$(cat <<'EOF'
refactor: split providers/codex em módulos

EOF
)"
```

---

### Task 2: Split `tui/update` (A2)

**Files:**
- Create: `src/tui/update/{mod,navigation,config,login,fetch,history}.rs`
- Delete: `src/tui/update.rs`
- Modify: `src/tui/mod.rs` se necessário (`pub mod update` já existe — diretório resolve)
- Test: testes em `update.rs` migram para `update/mod.rs` (ou por domínio se o teste for claramente de um handler)

**Interfaces:**
- Produces: `pub fn update(state: &mut AppState, action: Action) -> Vec<Action>` (assinatura atual — re-Read em `src/tui/update.rs` ~L248–250)
- Consumes: `Action`, `AppState`, tipos em `state.rs`, `login_state`, settings

**Partição do match de `Action` (orientação — re-Read o match completo):**

| Arquivo | Actions / helpers |
| --- | --- |
| `navigation.rs` | `key_to_action_with_state` (ou split se config/login interceptam), sidebar select, `Activate`, `Back`, `ToggleHelp`, `Quit`, screen jumps h/g/w |
| `config.rs` | `field_value_string`, `apply_field_edit`, `Config*`, `SaveConfig`, `InitConfig` se tratado em update |
| `login.rs` | `login_selected_id`, `Login*` |
| `fetch.rs` | `ProviderFetched`, refresh, status de fetch, pending_focus resolution |
| `history.rs` | `ToggleHistoryRange`, expand day, scroll history |
| `mod.rs` | `pub fn update` que faz `match action` e delega; reexport de `key_to_action` se for `pub` hoje |

- [ ] **Step 1: Baseline**

```bash
cargo test tui::update
```

Expected: PASS.

- [ ] **Step 2: Extrair helpers puros primeiro**

Mover `login_selected_id`, `field_value_string`, `apply_field_edit` para os arquivos de domínio; `mod.rs` usa-os. Compilar.

- [ ] **Step 3: Extrair braços do match**

Cada arquivo exporta algo como:

```rust
pub(super) fn handle(state: &mut AppState, action: &Action) -> Option<Vec<Action>>
```

ou funções por action. O `update` em `mod.rs` permanece o único `pub fn update`. **Sem mudar semântica** de follow-up actions.

- [ ] **Step 4: Mover testes**

Preservar nomes de teste. Ajustar `use super::*` / paths.

- [ ] **Step 5: Verificar**

```bash
cargo test tui::update
cargo test tui::
cargo clippy --all-targets -- -D warnings
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git commit -m "$(cat <<'EOF'
refactor: split tui/update por domínio

EOF
)"
```

---

### Task 3: Split `tui/render/detail` (A3)

**Files:**
- Create: `src/tui/render/detail/{mod,format,windows,chart,models,extra,totals,states}.rs` (+ `layout.rs` se o colapso sair de `render_full`)
- Delete: `src/tui/render/detail.rs`
- Modify: `src/tui/render/mod.rs` — `mod detail` / `pub use detail::render_detail` se existir path explícito
- Test: `detail` snapshots em `src/tui/render/snapshots/agent_bar__tui__render__detail__*` — **byte-idênticos**

**Interfaces:**
- Produces: `pub fn render_detail(state: &AppState, frame: &mut Frame, area: Rect, hits: &mut HitMap)`
- Consumes: widgets (`gauge_spans`, `column_chart_lines`, chips), `usage::*`, `ProviderQuota`

**Partição (símbolos atuais):**

| Arquivo | Símbolos |
| --- | --- |
| `format.rs` | `LABEL_W`, `*_SUFFIX_W`, `derive_bar_width`, `truncate_name`, `model_tokens`, `provider_usage_tokens`, `find_model_usage`, `fmt_reset`, `fmt_cost_generic` |
| `windows.rs` | `window_line`, `model_window_line`, `window_lines` |
| `models.rs` | `model_usage_line`, `model_lines` |
| `chart.rs` | `section_title`, `render_chart_section` |
| `extra.rs` | `extra_usage_line`, `extra_lines` |
| `totals.rs` | `totals_line` |
| `states.rs` | `render_logged_out`, `render_error` |
| `mod.rs` | `render_full`, `render_detail`, `render_empty`, `render_skeleton`, `render_footer_chips`, `skeleton_gauge_line`, `mod tests` |

- [ ] **Step 1: Baseline + snapshot hash**

```bash
cargo test tui::render::detail
# opcional: sha256sum src/tui/render/snapshots/agent_bar__tui__render__detail__*
```

- [ ] **Step 2: Mover `format` + seções**

Criar `detail/` com moves. `pub(super)` para helpers internos; só `render_detail` (e o que `render/mod.rs` precisa) público.

- [ ] **Step 3: Verificar snapshots idênticos**

```bash
cargo test tui::render::detail
cargo test tui::render
cargo clippy --all-targets -- -D warnings
```

Expected: PASS **sem** `INSTA_UPDATE`. Se insta falhar, o split mudou render — corrigir, não atualizar snapshot.

- [ ] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
refactor: split tui/render/detail por seção

EOF
)"
```

---

### Task 4: Fixtures Amp (A4.1)

**Files:**
- Create: `tests/fixtures/amp/usage-legacy-dollars.txt`
- Create: `tests/fixtures/amp/usage-free-pct.txt`
- Modify: `src/providers/amp.rs` (testes leem fixtures; manter testes inline existentes **ou** migrar FULL/NEW_FORMAT para arquivo — preferir **adicionar** testes baseados em fixture sem apagar os asserts atuais, para evitar churn)

**Interfaces:**
- Consumes: `parse_usage(stdout, base, now_ms)`
- Produces: regressão CI se o texto da CLI mudar

- [ ] **Step 1: Escrever fixtures**

`tests/fixtures/amp/usage-legacy-dollars.txt` (conteúdo exato — mesmo corpus do teste `FULL` atual):

```text
Signed in as user@email.com
Amp Free: $3.50/$5.00 remaining
replenishes +$0.25/hour
+20% bonus for 5 more days
Individual credits: $10.00 remaining
```

`tests/fixtures/amp/usage-free-pct.txt` (mesmo corpus `NEW_FORMAT`):

```text
Signed in as user@email.com (nick)
Amp Free: 97% remaining today (resets daily) - https://ampcode.com/settings#amp-free
Individual credits: $4.19 remaining (replenishes automatically) - https://ampcode.com/settings
```

- [ ] **Step 2: Testes que leem do disco**

Em `src/providers/amp.rs` `mod tests`, adicionar:

```rust
fn load_fixture(name: &str) -> String {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/amp")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

#[test]
fn fixture_legacy_dollars_parses_primary() {
    let q = parse_usage(&load_fixture("usage-legacy-dollars.txt"), base(), NOW);
    assert!(q.available);
    // asserts alinhados ao teste FULL existente (remaining/account) — copiar
    // os asserts numéricos do teste que usa const FULL (re-Read amp.rs).
}

#[test]
fn fixture_free_pct_parses_primary() {
    let q = parse_usage(&load_fixture("usage-free-pct.txt"), base(), NOW);
    assert!(q.available);
    // asserts alinhados ao teste NEW_FORMAT (re-Read amp.rs).
}
```

- [ ] **Step 3: Rodar**

```bash
cargo test providers::amp
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
test: fixtures Amp legacy e free-pct

EOF
)"
```

---

### Task 5: Waybar serialize fail barulhento (A4.2)

**Files:**
- Modify: `src/main.rs` (`print_waybar` ~L54–57 e testes ~L864+)
- Optional Create/Modify: extrair helper testável — preferir em `src/formatters/waybar.rs` ou função em `main` + testes no mesmo arquivo

**Interfaces:**
- Produces: `fn format_waybar_stdout(o: &WaybarOutput) -> String` (nome livre, mas estável) que:
  1. `Ok(json)` → json
  2. `Err` → log em stderr + payload JSON mínimo válido (nunca `""`)

**Comportamento (spec §A4.2):**

- Path feliz: idêntico ao atual (`serde_json::to_string` do `WaybarOutput`).
- Path erro: **não** `exit(1)`; stdout = JSON com `text` indicando erro e `class` degradado (reutilizar padrão de módulo com erro/disconnected já usado em `formatters/waybar.rs` — re-Read `WaybarOutput` e classes).

Implementação recomendada (sem mock de serde):

```rust
/// Serializa saída Waybar. Em falha de serde, devolve payload degradado
/// (nunca string vazia) e emite o erro em `err_log`.
pub(crate) fn waybar_stdout_line(
    o: &WaybarOutput,
    err_log: &mut dyn std::io::Write,
) -> String {
    match serde_json::to_string(o) {
        Ok(s) => s,
        Err(e) => {
            let _ = writeln!(err_log, "waybar serialize failed: {e}");
            // payload mínimo: text + class. Campos extras conforme WaybarOutput.
            serde_json::json!({
                "text": "err",
                "tooltip": format!("agent-bar: serialize failed: {e}"),
                "class": "agent-bar-hidden" // ou class de erro — re-Read WaybarOutput fields
            })
            .to_string()
        }
    }
}
```

**Ajuste:** re-Read `WaybarOutput` em `src/formatters/waybar.rs` e preencher **todos** os campos obrigatórios do struct (não inventar chaves JSON se o tipo tem serde rename). Preferir construir um `WaybarOutput { … }` de fallback e serializar **esse** (se o fallback serializa, ok; se até o fallback falhar — teoricamente impossível com strings estáticas — usar literal JSON hardcoded como último recurso).

```rust
fn print_waybar(o: &WaybarOutput) {
    let mut err = std::io::stderr();
    println!("{}", waybar_stdout_line(o, &mut err));
}
```

- [ ] **Step 1: Teste do path feliz (já existe) + teste de fallback**

Como forçar `Err` de serde é difícil com `WaybarOutput` bem tipado, testar:

1. Path feliz: `waybar_stdout_line` devolve o mesmo que `serde_json::to_string`.
2. Função de fallback dedicada `waybar_error_payload(msg: &str) -> WaybarOutput` serializa e **não** é vazia; `text` não vazio; JSON parseável.

```rust
#[test]
fn waybar_error_payload_is_non_empty_json() {
    let o = waybar_error_payload("boom");
    let s = serde_json::to_string(&o).expect("fallback must serialize");
    assert!(!s.is_empty());
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert!(v.get("text").and_then(|t| t.as_str()).map(|t| !t.is_empty()) == Some(true));
}
```

- [ ] **Step 2: Implementar + ligar `print_waybar`**

- [ ] **Step 3: Verificar**

```bash
cargo test cli
cargo test --test golden
cargo test formatters::waybar
# e o módulo onde o teste do helper morar
cargo clippy --all-targets -- -D warnings
```

Expected: golden inalterado (path feliz).

- [ ] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
fix: Waybar nunca emite stdout vazio

EOF
)"
```

---

### Task 6: Docs residual TS (A4.3)

**Files:**
- Modify: `docs/architecture.md` (linhas que citam `index.ts`, `notify.ts` — ~L60, ~L66 e quaisquer outras)
- Grep: `docs/*.md` (exceto `docs/superpowers/**` e `CHANGELOG.md`)

- [ ] **Step 1: Localizar**

```bash
rg -n 'index\.ts|notify\.ts|src/.*\.ts' docs --glob '!superpowers/**' --glob '!**/CHANGELOG.md'
```

- [ ] **Step 2: Reescrever para paths Rust**

Exemplos de substituição:

| Antes | Depois |
| --- | --- |
| `index.ts` short-circuits | `main.rs` / gate `is_hidden_module` short-circuits |
| `src/notify.ts` | `src/notify.rs` |

Manter o significado do fluxo; não apagar seções.

- [ ] **Step 3: Verificar**

```bash
rg -n 'index\.ts|notify\.ts' docs --glob '!superpowers/**' --glob '!**/CHANGELOG.md' || true
git diff --check
```

Expected: zero hits nos docs operacionais (ou só menções históricas conscientes).

- [ ] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
docs: remove residual TS da architecture

EOF
)"
```

---

### Task 7: Gate final da trilha A

**Files:** nenhum novo — verificação ampla.

- [ ] **Step 1: Suite**

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

Expected: all PASS, clippy clean.

- [ ] **Step 2: Invariantes**

```bash
# monólitos sumiram
test ! -f src/providers/codex.rs
test -f src/providers/codex/mod.rs
test ! -f src/tui/update.rs
test -f src/tui/update/mod.rs
test ! -f src/tui/render/detail.rs
test -f src/tui/render/detail/mod.rs
test -f tests/fixtures/amp/usage-legacy-dollars.txt
test -f tests/fixtures/amp/usage-free-pct.txt
rg -n 'unwrap_or_default\(\)' src/main.rs | rg print_waybar || true
# print_waybar não deve mais usar unwrap_or_default na serialização principal
```

- [ ] **Step 3: Report**

Escrever em `.superpowers/sdd/progress.md` (ou stdout do agente) a lista de commits da branch e confirmação do gate.

- [ ] **Step 4: Commit** (só se houver fix do gate; senão sem commit)

Se o gate achar regressão, fix + commit `fix: …` e re-rodar Step 1.

---

## Self-review do plano (coverage)

| Spec | Task |
| --- | --- |
| §2 PR A1 Codex | T1 |
| §3 PR A2 tui/update | T2 |
| §4 PR A3 detail | T3 |
| §5 A4.1 Amp fixtures | T4 |
| §5 A4.2 Waybar fail | T5 |
| §5 A4.3 docs TS | T6 |
| §9 métricas / §12 | T7 |
| A5 opcional | **fora** (explícito) |
| Trilhas B/C | **fora** |

## Notas para o controller multi-agente

- Um implementer por task, em série; reviewer após cada task.
- Worktree limpa a partir de `master` (inclui commit da spec); **não** carregar uncommitted amp/history do checkout do usuário.
- Após T3, T4 pode tocar `amp.rs` — ok; se o usuário tiver WIP local em amp, isso vive só no checkout sujo dele.
- Model: implementers mecânicos (move) = tier alto o suficiente para multi-file; reviewer padrão; gate T7 + review final de branch no modelo mais capaz disponível.
