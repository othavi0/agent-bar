# Provider Grok (Grok Build CLI) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Adicionar o provider `grok` ao agent-bar: status do Grok Build CLI a partir de `~/.grok` (auth + `signals.json`), com `%` de **contexto restante da sessão recente** na barra — conforme `docs/superpowers/specs/2026-07-17-grok-provider-design.md`.

**Architecture:** `GrokProvider` estende `QuotaSource`/`base_get_quota` (zero rede). `find_grok_bin` espelha `amp_cli`. Scan limitado de `sessions/**/signals.json`. `GrokQuotaExtra` para contagens do tooltip. Integração completa: registry, Waybar, TUI login, builder, ícone, docs. Settings existentes **não** auto-habilitam grok; defaults de install novo incluem via `KNOWN_PROVIDER_IDS`.

**Tech Stack:** Rust 2021, serde/serde_json, time (RFC3339), tokio tests, insta/golden existentes. Sem crates novas (walk com `std` recursivo).

## Global Constraints

- Spec canônica: `docs/superpowers/specs/2026-07-17-grok-provider-design.md` (ler §0–§14).
- **% = contexto de sessão, NÃO cota de plano** — copy de tooltip/README deve dizer “contexto”.
- **Zero rede** na v1. **Zero refresh OAuth.**
- Error strings **verbatim** (spec §4).
- `unwrap`/`expect` proibidos em produção (`deny` no crate).
- stdout limpo Waybar; logs stderr.
- Não mutar `~/.config/waybar` ao vivo; testes com temp dirs + paths injetados.
- Conventional Commits PT, subject ≤ 50 chars; zero atribuição AI.
- Gotcha RTK: um filtro posicional por `cargo test`.
- Implementers: Read antes de Edit; re-Read se Edit falhar; após outro agente, re-Read.
- Fixtures **nunca** com JWT real do dev.

## File map (estado final)

```text
src/providers/grok_cli.rs          # find_grok_bin
src/providers/grok.rs              # QuotaSource + parse auth/signals + walk
src/providers/error.rs             # GrokError
src/providers/types.rs             # GrokQuotaExtra + ProviderExtra::Grok
src/providers/extras.rs            # get_grok_extra
src/providers/mod.rs               # mod + registry
src/config.rs                      # KNOWN_PROVIDER_IDS + Paths.grok_*
src/theme.rs                       # provider_hex("grok") → Cyan
src/formatters/builders/grok.rs    # build_grok
src/formatters/builders/mod.rs
src/formatters/waybar.rs           # match "grok"
src/formatters/terminal.rs         # match "grok"
src/waybar_contract.rs             # WAYBAR_PROVIDERS + CSS icon
src/tui/login_spawn.rs             # grok login
src/tui/render/login.rs            # PROVIDERS list
icons/grok-icon.svg
tests/fixtures/grok/auth-valid.json
tests/fixtures/grok/auth-expired.json
tests/fixtures/grok/signals-recent.json
tests/fixtures/grok/signals-full.json
docs/new-provider.md, README.md
```

## Ordem

```text
T1 foundation (ids/paths/error/theme/types/extra)
  → T2 grok_cli
  → T3 provider core + fixtures + unit tests
  → T4 builder + waybar/terminal dispatch
  → T5 waybar_contract + icon
  → T6 login TUI
  → T7 docs + hardcode sweep + gate
```

---

### Task 1: Foundation — IDs, Paths, Error, Types, Theme

**Files:**
- Modify: `src/config.rs` (`KNOWN_PROVIDER_IDS`, `default_ttl_secs`, `Paths`)
- Modify: `src/providers/error.rs`
- Modify: `src/providers/types.rs` (`GrokQuotaExtra`, `ProviderExtra::Grok`)
- Modify: `src/providers/extras.rs` (`get_grok_extra` + testes)
- Modify: `src/theme.rs` (`provider_hex("grok")` + teste)

**Interfaces:**
- Produces:
  - `KNOWN_PROVIDER_IDS = ["claude","codex","amp","grok"]`
  - `default_ttl_secs("grok") -> 90`
  - `Paths { grok_home, grok_auth, … }`
  - `GrokError::{NotInstalled, NotLoggedIn, InvalidCredentials}` com strings da spec §4
  - `ProviderError::Grok(#[from] GrokError)`
  - ```rust
    #[derive(Debug, Clone, PartialEq, Serialize, Default)]
    #[serde(rename_all = "camelCase")]
    pub struct GrokQuotaExtra {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub sessions_today: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub turns_today: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub context_tokens_used: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub context_window_tokens: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub recent_model: Option<String>,
    }
    ```
  - `ProviderExtra::Grok(GrokQuotaExtra)`
  - `get_grok_extra(q) -> Option<&GrokQuotaExtra>`
  - `provider_hex("grok") == ColorToken::Cyan.hex()` (`#56b6c2`)

- [ ] **Step 1: Error strings + testes**

Em `error.rs`, adicionar (mensagens **exatas**):

```rust
#[derive(Error, Debug)]
pub enum GrokError {
    #[error("Grok CLI not installed. Install from https://x.ai/cli or ensure ~/.grok/bin/grok is on PATH.")]
    NotInstalled,
    #[error("Not logged in. Open `agent-bar menu` and choose Provider login.")]
    NotLoggedIn,
    #[error("Failed to read Grok credentials.")]
    InvalidCredentials,
}
```

E `ProviderError::Grok(#[from] GrokError)`.

Teste `grok_strings_are_verbatim` no módulo de testes de `error.rs`.

- [ ] **Step 2: KNOWN_PROVIDER_IDS + Paths**

```rust
pub const KNOWN_PROVIDER_IDS: [&str; 4] = ["claude", "codex", "amp", "grok"];
```

Em `default_ttl_secs`:

```rust
"codex" | "amp" | "grok" => 90,
```

`Paths`:

```rust
pub grok_home: PathBuf,
pub grok_auth: PathBuf,
```

Em `from_env`:

```rust
let grok_home = env::var_os("GROK_HOME")
    .filter(|v| !v.is_empty())
    .map(PathBuf::from)
    .unwrap_or_else(|| home.join(".grok"));
// ...
grok_home: grok_home.clone(),
grok_auth: grok_home.join("auth.json"),
```

Atualizar **todos** os construtores de `Paths` em testes do crate (`PathBuf::new()` para os novos campos) — grep `amp_threads:` e complete.

- [ ] **Step 3: types + extras + theme**

Adicionar `GrokQuotaExtra` e variante. `get_grok_extra` espelhando `get_amp_extra`. Theme + assert.

- [ ] **Step 4: Verificar**

```bash
cargo test providers::error
cargo test providers::extras
cargo test theme
cargo test settings
cargo clippy --all-targets -- -D warnings
```

Expected: PASS (settings default agora tem 4 providers — atualizar asserts que esperam exatamente `["claude","codex","amp"]`).

- [ ] **Step 5: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat: base do provider grok (ids/paths/extra)

EOF
)"
```

---

### Task 2: `grok_cli` locator

**Files:**
- Create: `src/providers/grok_cli.rs`
- Modify: `src/providers/mod.rs` (`pub mod grok_cli;`)

**Interfaces:**
- Produces:
  - `pub fn grok_candidate_paths(home: &str) -> Vec<PathBuf>`
  - `pub fn find_grok_bin_with(home, which, exists) -> Option<PathBuf>`
  - `pub fn find_grok_bin(home: &str) -> Option<PathBuf>`
  - Candidatos (ordem): `{home}/.grok/bin/grok`, `{home}/.local/bin/grok`
  - PATH (`which("grok")`) **primeiro**, depois candidatos

Espelhar testes de `amp_cli.rs` (prefer_path, fallback, none, empty_home).

- [ ] **Step 1: Implement + testes no mesmo arquivo**

- [ ] **Step 2: Verificar**

```bash
cargo test providers::grok_cli
```

- [ ] **Step 3: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat: locator do binário grok

EOF
)"
```

---

### Task 3: Provider core (auth + signals + BaseProvider)

**Files:**
- Create: `src/providers/grok.rs`
- Create: `tests/fixtures/grok/auth-valid.json`
- Create: `tests/fixtures/grok/auth-expired.json`
- Create: `tests/fixtures/grok/signals-recent.json`
- Create: `tests/fixtures/grok/signals-full.json`
- Modify: `src/providers/mod.rs` (`pub mod grok;` + registry)

**Interfaces:**
- Produces:
  - `pub struct GrokProvider;`
  - `impl QuotaSource for GrokProvider` com `Raw = GrokRaw`
  - `impl Provider for GrokProvider` delegando `base_get_quota`
  - Funções puras testáveis:
    - `pub(crate) fn parse_auth_entries(bytes: &[u8], now: OffsetDateTime) -> Result<AuthView, GrokError>`
    - `pub(crate) fn context_remaining_pct(used: u64, window: u64) -> Option<f64>`
    - `pub(crate) fn collect_signals(sessions_dir: &Path, now_local_date: Date, offset: UtcOffset) -> Vec<SessionSnap>`
  - `GrokRaw` **sem** JWT:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GrokRaw {
    account: Option<String>,
    logged_in: bool,
    sessions: Vec<SessionSnap>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionSnap {
    mtime_ms: u64,
    context_tokens_used: Option<u64>,
    context_window_tokens: Option<u64>,
    primary_model_id: Option<String>,
    turn_count: u32,
}
```

**Fórmula primary (spec §3.4):**

```rust
pub(crate) fn context_remaining_pct(used: u64, window: u64) -> Option<f64> {
    if window == 0 {
        return None;
    }
    let used_pct = 100.0 * (used as f64) / (window as f64);
    Some((100.0 - used_pct).clamp(0.0, 100.0))
}
```

**Walk (spec §3.3):** recursivo std, depth max 16, max 2000 visits, só arquivos cujo `file_name() == "signals.json"`.

**Auth fixtures (sem secrets reais):**

`auth-valid.json`:
```json
{
  "https://auth.x.ai::test-client": {
    "key": "test-access-token",
    "refresh_token": "test-refresh",
    "expires_at": "2099-01-01T00:00:00Z",
    "first_name": "Test User",
    "user_id": "00000000-0000-0000-0000-000000000001",
    "auth_mode": "oidc"
  }
}
```

`auth-expired.json`: mesmo com `"expires_at": "2020-01-01T00:00:00Z"`.

`signals-recent.json`:
```json
{
  "contextTokensUsed": 50000,
  "contextWindowTokens": 500000,
  "contextWindowUsage": 10,
  "primaryModelId": "grok-4.5",
  "turnCount": 3
}
```
→ remaining = 90.0

`signals-full.json`: used=500000 window=500000 → remaining = 0.0

**Layout de fixture home em teste:**

```text
tmp/
  auth.json
  bin/grok          # arquivo vazio basta p/ is_file (ou só auth)
  sessions/proj/sid/signals.json
```

**`is_available`:** `paths.grok_auth.is_file()` OR `find_grok_bin(home).is_some()`.

**`fetch_raw`:** lê auth; se fail → Err; monta sessions; Ok(GrokRaw).

**`build_quota`:** se !logged_in → error NotLoggedIn; senão available=true, primary da sessão max mtime, extra com sessions_today/turns_today.

**Registry:**

```rust
vec![
    Box::new(claude::ClaudeProvider),
    Box::new(amp::AmpProvider),
    Box::new(codex::CodexProvider),
    Box::new(grok::GrokProvider),
]
```

- [ ] **Step 1: Testes unitários RED** (funções puras + async get_quota com tempdir)

Cobrir no mínimo:
- `context_remaining_pct(50000, 500000) == Some(90.0)`
- `context_remaining_pct(0, 0) == None`
- not installed / not logged in / expired / happy path remaining / picks recent / corrupt ignored

- [ ] **Step 2: Implementar até verde**

- [ ] **Step 3: Verificar**

```bash
cargo test providers::grok
cargo test providers::base
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat: provider grok (auth + signals)

EOF
)"
```

---

### Task 4: Builder + dispatch formatters

**Files:**
- Create: `src/formatters/builders/grok.rs`
- Modify: `src/formatters/builders/mod.rs`
- Modify: `src/formatters/waybar.rs` (match `"grok"`)
- Modify: `src/formatters/terminal.rs` (match `"grok"`)

**Interfaces:**
- Produces: `pub fn build_grok(clock: &Clock, p: &ProviderQuota, options: &BuildOptions) -> Vec<Line>`
- Cor de marca: `ColorToken::Cyan`
- Copy obrigatória em linha de contexto (texto do segment):
  - label `"contexto"` (minúsculo ok) perto da barra de primary
  - se error: mostrar error em Red
  - se available sem primary: `"sem sessões locais ainda"` (Comment/Muted)
  - footer com `build_footer_line` como outros builders
  - se `get_grok_extra`: linha `hoje  N sessões · M turns` quando Some

Espelhar estrutura de `build_generic` / header de Amp, mas com Cyan e labels de contexto.

Teste: `build_grok` render_pango contém `"contexto"` e não contém `"plano"` / `"quota de plano"`.

- [ ] **Step 1: Builder + testes**
- [ ] **Step 2: Wire waybar + terminal**

```rust
"grok" => render_pango(&build_grok(...))  // waybar
"grok" => render_ansi(&build_grok(...), no_color)  // terminal
```

- [ ] **Step 3: Verificar**

```bash
cargo test formatters::builders::grok
cargo test formatters::waybar
cargo test formatters::terminal
```

- [ ] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat: builder e dispatch Waybar do Grok

EOF
)"
```

---

### Task 5: Waybar contract + ícone

**Files:**
- Modify: `src/waybar_contract.rs` (`WAYBAR_PROVIDERS` length 4 + `"grok"`)
- Create: `icons/grok-icon.svg` (SVG simples 24×24, fill atual `#c0c9d4` ou currentColor se o pipeline usar mask; copiar estrutura de `icons/amp-icon.svg` se existir)
- Atualizar `export_waybar_css` se lista de ícones for explícita
- Fix testes em `waybar_contract.rs` / `waybar_integration.rs` que listam só 3 providers

**Ícone mínimo SVG:**

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="#c0c9d4">
  <text x="12" y="17" text-anchor="middle" font-size="14" font-family="sans-serif" font-weight="700">G</text>
</svg>
```

(Se o CSS espera PNG, seguir o padrão dos outros providers no `export_waybar_css` / install assets.)

- [ ] **Step 1: WAYBAR_PROVIDERS + testes contract**
- [ ] **Step 2: Ícone + CSS path**
- [ ] **Step 3: Verificar**

```bash
cargo test waybar_contract
cargo test waybar_integration
```

- [ ] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat: módulo Waybar e ícone do Grok

EOF
)"
```

---

### Task 6: Login TUI

**Files:**
- Modify: `src/tui/login_spawn.rs` — braço `"grok"` com `find_grok_bin` + `arg("login")`
- Modify: `src/tui/render/login.rs` — `PROVIDERS` length 4 + `("grok", "Grok")`
- Modify: qualquer `login_selected_id` / índices que assumam 3 providers (grep `PROVIDERS` e `login_selected`)

Login:

```rust
"grok" => {
    let home = std::env::var("HOME").unwrap_or_default();
    let bin = find_grok_bin(&home).ok_or_else(|| {
        anyhow::anyhow!("Grok CLI não encontrado. Instale com: curl -fsSL https://x.ai/cli/install.sh | bash")
    })?;
    let status = Command::new(&bin)
        .arg("login")
        // ... inherit stdio
        .status()?;
    ...
}
```

- [ ] **Step 1: Implement**
- [ ] **Step 2: Verificar**

```bash
cargo test tui::render::login
cargo test tui::update
```

Atualizar snapshots de login se necessário (`INSTA_UPDATE=1` só se o display mudou de propósito).

- [ ] **Step 3: Commit**

```bash
git commit -m "$(cat <<'EOF'
feat: login TUI do Grok

EOF
)"
```

---

### Task 7: Docs, hardcode sweep, gate

**Files:**
- Modify: `docs/new-provider.md` — tabela de padrões + Grok
- Modify: `README.md` — bullet Grok Build CLI
- Grep final de hardcodes 3-providers (spec §11.1)
- `CHANGELOG.md` sob `[Unreleased]` (não cortar release neste plano)

**README bullet (pt-BR, estilo existente):**

```markdown
- **Grok Build** — OAuth em `~/.grok/auth.json` + `signals.json` das sessões.
  O % da barra é **contexto restante da sessão recente**, não cota de plano xAI.
```

- [ ] **Step 1: Docs**
- [ ] **Step 2: Grep e corrigir leftovers**

```bash
rg -n 'KNOWN_PROVIDER_IDS: \[&str; 3\]|WAYBAR_PROVIDERS: \[&str; 3\]|"claude", "codex", "amp"\]' src tests
```

- [ ] **Step 3: Gate**

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

Expected: all PASS.

Smoke opcional (não mutar waybar):

```bash
GROK_HOME=/path/to/fixture cargo run -- status -p grok
# ou com ~/.grok real do user
cargo run -- status -p grok
```

- [ ] **Step 4: Commit**

```bash
git commit -m "$(cat <<'EOF'
docs: Grok no README e new-provider

EOF
)"
```

---

## Spec coverage (self-review do plano)

| Spec | Task |
| --- | --- |
| §0 princípio contexto | T4 copy + T7 README |
| D1–D9 | T1–T3 |
| Paths / GROK_HOME | T1 |
| Auth / expiry / no refresh | T3 |
| signals walk + remaining formula | T3 |
| GrokQuotaExtra | T1 + T3 build |
| Error strings | T1 + T3 |
| Builder / waybar / terminal | T4 |
| Icon / WAYBAR_PROVIDERS | T5 |
| Login | T6 |
| Hardcode inventory §11.1 | T1, T5, T6, T7 |
| Fora da v1 | não implementado |

## Notas multi-agente

- Worktree limpa a partir de `master` atual.
- Um implementer por task; reviewer após cada uma.
- T3 é a task mais crítica (parse + walk); não pular fixtures.
- Settings default com 4 providers **quebra** testes que comparam lista exata de 3 — tratar em T1, não deixar para o gate.
