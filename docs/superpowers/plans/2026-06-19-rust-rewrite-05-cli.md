# Plano 05 — CLI / main (plumbing) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Dar ao crate Rust uma CLI funcional (parsing, dispatch, `--watch`, notify gateado, hidden-module short-circuit, `action-right`) ligando a camada de providers+formatação já pronta — `main.rs` vira `#[tokio::main(current_thread)]`.

**Architecture:** `parse_args` é um **port fiel à mão** do `parseArgs` do TS (NÃO clap-derive — os strings de erro e a sugestão levenshtein são contrato, asseridos em `tests/cli.test.ts`). Tudo testável é função pura (parse, help, disconnect-detection, watch-line, gates de notify/hidden); `main` é o único impuro (constrói `Ctx`, faz I/O, imprime, sai). Comandos de install/TUI (menu/setup/assets/export/update/uninstall/remove/doctor) **não existem ainda** (Plano 6) → `main` os roteia a um stub claro em stderr + exit 1; o parsing deles é completo (tests exigem).

**Tech Stack:** tokio (`current_thread` + `macros` + `signal` opcional), `regex` (disconnect), std `IsTerminal`/`BrokenPipe`. Sem libs novas.

## Global Constraints

- **Contrato byte-exact do Waybar/Pango é sagrado.** Autoridade = saída do TS. REJEITAR finding de review que divergiria do TS — conferir o comportamento real do TS antes de aceitar qualquer "fix".
- **Strings de erro/help são contrato.** `tests/cli.test.ts` assere needles verbatim: `--format must be`, `--interval must be`, `--watch requires --format json`, `Did you mean 'setup'`, `Unknown command: xyzzy` + `help`, `assets install`, `waybar-modules`, `requires a value`, `--interval has no effect without --watch`, e os 2 needles do help (`Update the install (npm or managed checkout)`, `agent-bar  or  bun run start`). Copiar verbatim do TS.
- **Sem `unwrap()`/`expect()` em produção** (lib.rs E main.rs têm `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]`). Em `#[cfg(test)]` é permitido. Sem `!` non-null (N/A em Rust; não usar `.unwrap()` como atalho).
- **stdout limpo.** Só payload de máquina (JSON Waybar / NDJSON / view terminal) vai pra stdout via `println!`/`print!` em fn isolada. Todo log → `log::{warn,error,info}` (stderr). Nunca `eprintln!`/`println!` para diagnóstico fora do payload.
- **Sem estado global mutável.** `no_color` (gate ANSI) é lido do env **só no `main`** e injetado como parâmetro nas fns de formatação/help. `Clock`, `Paths`, `Settings` são injetados.
- **Usar constantes de identidade** (`crate::app_identity::{APP_NAME, APP_HIDDEN_CLASS, VERSION}`), nunca hardcode.
- **`cargo fmt` ANTES de `git add`.** Verificação: `cargo test --manifest-path rust/Cargo.toml` + `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`. Gotcha RTK: cargo vira `cargo test: N passed (K suites)`; **não há string `test result:`**; um filtro posicional só; ler bruto com `2>&1 | tail -6`.
- Commits: Conventional Commits PT, subject ≤50 chars.
- **NÃO tocar** `package.json`, `src/` (TS), docs do projeto (`README.md`/`docs/*` exceto `docs/superpowers/`). A reescrita vive só em `rust/`.

---

## File Structure

- `rust/src/cli.rs` — **novo módulo**: `Command`/`Format` enums, `CliOptions`, `CliError`, `parse_args`, `suggest_command`/`levenshtein`, `build_help`/`show_help`. Registrar `pub mod cli;` em `lib.rs`.
- `rust/src/providers/mod.rs` — **estender**: `get_provider(id) -> Option<Box<dyn Provider>>`, `get_quota_for(id, &Ctx) -> Option<ProviderQuota>` (async), `registered_provider_ids() -> Vec<&'static str>`.
- `rust/src/action_right.rs` — **novo**: `looks_disconnected(provider_id, error) -> bool` (puro), `handle_action_right(id, &Ctx, &Clock, no_color)` (async). `pub mod action_right;` em lib.rs.
- `rust/src/watch.rs` — **novo**: `build_watch_line(&AllQuotas) -> String` (puro), `start_watch(provider, interval, &Ctx)` (async, nunca retorna em caminho normal). `pub mod watch;` em lib.rs.
- `rust/src/main.rs` — **reescrever**: `#[tokio::main(flavor = "current_thread")]`, build `Ctx`, dispatch, gates, output helpers, stubs de comando não-implementado.

Interfaces consumidas pela camada já pronta (NÃO reimplementar — chamar):
- `config::{Paths::from_env() -> anyhow::Result<Paths>, now_ms() -> u64, CLAUDE_USAGE_URL, DEFAULT_INTERVAL_SECS}`
- `settings::{load(&Paths) -> Settings, Settings{waybar:Waybar{providers:Vec<String>, provider_order, display_mode:DisplayMode, signal, interval, separators, show_percentage}, notify:Notify{enabled:bool}, cache}}`
- `providers::{Ctx<'a>{client:&reqwest::Client, paths:&Paths, settings:&Settings, now_ms:u64, local_offset:UtcOffset, claude_usage_url:String, version:&'static str, home:PathBuf}, registry() -> Vec<Box<dyn Provider>>, fetch_all(&[Box<dyn Provider>], &Ctx) -> AllQuotas (async), Provider{id,name,cache_key,is_available(&Ctx) async,get_quota(&Ctx) async -> ProviderQuota}, iso_from_ms(u64) -> String}`
- `providers::types::{AllQuotas{providers:Vec<ProviderQuota>, fetched_at:String}, ProviderQuota{provider, available, error:Option<String>, primary, ...}}`
- `formatters::clock::Clock{now, local_offset, Clock::from_env()}`
- `formatters::terminal::format_for_terminal(&Clock, &AllQuotas, &Settings, DisplayMode, no_color:bool) -> String`
- `formatters::waybar::{WaybarOutput{text,tooltip,class,alt,percentage} (Serialize), format_for_waybar(&Clock, &AllQuotas, &Settings, DisplayMode) -> WaybarOutput, format_provider_for_waybar(&Clock, &ProviderQuota, &Settings, DisplayMode) -> WaybarOutput}`
- `formatters::json::to_json_string(&AllQuotas) -> Result<String, serde_json::Error>`
- `cache::invalidate(cache_dir:&Path, key:&str)`
- `notify::check_and_notify(&AllQuotas, cache_dir:&Path)` (async)
- `http::client() -> &'static reqwest::Client`
- `logger::init(verbose:bool)`
- `theme::{ColorToken, ANSI_RESET, ANSI_BOLD, box_chars::{TL,BL,LT,H,V,DOT,DIAMOND}}`

Fonte TS (ler antes de portar): `src/cli.ts`, `src/index.ts`, `src/watch.ts`, `src/action-right.ts`, `tests/cli.test.ts`.

---

### Task 1: `cli.rs` — tipos + `parse_args` + sugestão

**Files:**
- Create: `rust/src/cli.rs`
- Modify: `rust/src/lib.rs` (add `pub mod cli;`)
- Test: inline `#[cfg(test)] mod tests` em `cli.rs` (porta `tests/cli.test.ts` describe `parseArgs` + `unknown commands` + `output format flags`).

**Interfaces (Produces):**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Waybar, Terminal, Menu, Status, Help, Version, ActionRight,
    Setup, AssetsInstall, ExportWaybarModules, ExportWaybarCss,
    Update, Uninstall, Remove, Doctor,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format { Waybar, Json }

#[derive(Debug, Clone, PartialEq)]
pub struct CliOptions {
    pub command: Command,
    pub refresh: bool,
    pub provider: Option<String>,
    pub verbose: bool,
    pub format: Format,
    pub watch: bool,
    pub interval_seconds: u32,
    pub waybar_dir: Option<String>,
    pub scripts_dir: Option<String>,
    pub icons_dir: Option<String>,
    pub app_bin: Option<String>,
    pub terminal_script: Option<String>,
    pub dry_run: bool,
    pub yes: bool,
    /// Avisos não-fatais (stderr) coletados durante o parse — o caller imprime.
    pub warnings: Vec<String>,
}

/// Erro fatal de parsing — o caller imprime `message` em stderr e sai com 1.
#[derive(Debug, Clone, PartialEq)]
pub struct CliError { pub message: String }

pub fn parse_args(args: &[String]) -> Result<CliOptions, CliError>;
```

**Contrato de comportamento — porta fiel de `src/cli.ts:167-337` (`parseArgs`):**

- Defaults: `command=Waybar, refresh=false, verbose=false, format=Waybar, watch=false, interval_seconds=60` (`DEFAULT_INTERVAL_SECS`), restante `None`/`false`, `warnings=[]`.
- Loop por token (`switch` do TS). Comandos posicionais: `menu`/`status`/`setup`/`update`/`uninstall`/`remove`/`doctor` setam `command`. `assets` → exige próximo token `install` (senão `Err("Unknown subcommand for 'assets'. Did you mean 'assets install'?")`). `export` → próximo token `waybar-modules`|`waybar-css` (senão `Err("Unknown subcommand for 'export'. Use 'export waybar-modules' or 'export waybar-css'.")`). `action-right` → consome próximo token como `provider` (via `require_next_arg`).
- Flags: `--dry-run`→`dry_run=true`; `--yes`/`-y`→`yes=true`; `--terminal`/`-t`→`command=Terminal`; `--refresh`/`-r`→`refresh=true`; `--provider`/`-p <v>`→`provider`; `--verbose`/`-v`→`verbose=true`; `--waybar-dir`/`--scripts-dir`/`--icons-dir`/`--app-bin`/`--terminal-script <v>`→ respectivos `Option<String>`; `--help`/`-h`/`help`→`command=Help`; `--version`/`-V`→`command=Version`.
- `--format <v>`: `v` deve ser `waybar`|`json` senão `Err("Error: --format must be 'waybar' or 'json' (got '{v}')")`. Marca `format_given=true`.
- `--watch`→`watch=true`. `--interval <v>`: `v` deve ser inteiro positivo (`v.parse::<i64>()` Ok && `> 0`; rejeitar `1.5`/`abc`/`0`) senão `Err("Error: --interval must be a positive integer (got '{v}')")`. Marca `interval_given=true`.
- `require_next_arg(args, i, flag)`: se `i+1 >= len` → `Err("Error: {flag} requires a value")`. (No TS é `console.error` + `exit(1)`; aqui é `Err`.)
- `default` do switch: se `arg.starts_with('-')` → `warnings.push("Unknown option: {arg}")` (TS usa `logger.warn`; aqui empurra pra `warnings`, NÃO erra, NÃO muda command). Senão → comando desconhecido: `suggest_command(arg)` → `Some(s)` ⇒ `Err("Unknown command: {arg}. Did you mean '{s}'?")`; `None` ⇒ `Err("Unknown command: {arg}. Run '{APP_NAME} help' for available commands.")`.
- Pós-loop: se `watch` && `format_given` && `format==Waybar` → `Err("Error: --watch requires --format json")`. Se `watch` → `format=Json`. Se `interval_given` && `!watch` → `warnings.push("[agent-bar] --interval has no effect without --watch")`.
- `KNOWN_COMMANDS` (para `suggest_command`, verbatim do TS:126): `["menu","status","setup","assets","export","update","uninstall","remove","action-right","doctor","help"]`. `levenshtein` clássico (DP); `suggest_command` retorna o de menor distância se `<= 3`, senão `None`.

> Nota de design (consciente, registrar no report): `parseArgs` do TS chama `process.exit` direto; aqui modelamos como `Result<CliOptions, CliError>` + `warnings: Vec<String>` (o `main` imprime e sai). Isso preserva os strings verbatim e torna o parse 100% testável sem mockar exit. Erros fatais → `CliError`; avisos não-fatais → `warnings`.

- [ ] **Step 1:** Escrever os testes (porta de `tests/cli.test.ts`): defaults; cada comando (`menu/status/setup/assets install/export waybar-modules/export waybar-css/update/uninstall/remove/doctor/help/--help/-h/action-right claude/--version/-V`); `doctor --dry-run --yes`; cada flag (`--refresh/-r/--verbose/-v/--terminal/-t/--provider -p/--waybar-dir/--scripts-dir/--icons-dir/--app-bin/--terminal-script`); combos (`status --refresh --verbose -p claude`; `-v -r menu`); unknown-command-com-sugestão (`setip`→`Did you mean 'setup'`), sem-sugestão (`xyzzy`→`Unknown command: xyzzy` + contém `help`); subcomando faltando 2ª palavra (`assets`→contém `assets install`; `export`→contém `waybar-modules`); unknown-flag não-erra (`--unknown-flag`→Ok, command=Waybar); format defaults; `--format json`; `--watch` implica json; `--interval 30`; invalid `--format xml`; invalid `--interval abc`/`1.5`/`0`; `--watch --format waybar` erra; `--interval 30` sem watch → warning contém `--interval has no effect without --watch`. Asseres: Ok via `parse_args(..).unwrap()`; fatais via `parse_args(..).unwrap_err().message.contains(needle)`; warnings via `.unwrap().warnings.iter().any(|w| w.contains(needle))`.
- [ ] **Step 2:** Rodar → falha (módulo não existe). `cargo test --manifest-path rust/Cargo.toml cli 2>&1 | tail -6`.
- [ ] **Step 3:** Implementar `cli.rs` (tipos + `parse_args` + `levenshtein` + `suggest_command`), idiomático e comentado. `pub mod cli;` em `lib.rs` (ordem alfabética).
- [ ] **Step 4:** Rodar testes → passam. `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings` limpo. `cargo fmt --manifest-path rust/Cargo.toml`.
- [ ] **Step 5:** Commit: `feat(rust): parse_args do CLI (port fiel)`.

---

### Task 2: `cli.rs` — `show_help` (build_help)

**Files:**
- Modify: `rust/src/cli.rs` (add `build_help`/`show_help`)
- Test: inline (porta o describe `showHelp` de `tests/cli.test.ts:354`).

**Interfaces (Produces):**
```rust
/// Monta o texto de ajuda (multilinha, terminado em `\n`). `no_color` injetado.
pub fn build_help(no_color: bool) -> String;
/// Imprime `build_help` em stdout (chamado pelo main).
pub fn show_help(no_color: bool);
```

**Contrato — porta fiel de `src/cli.ts:62-116` (`showHelp`):**
- Replicar EXATAMENTE as linhas: header `┏━ {APP_NAME} v{VERSION} ━…` (largura `w=58`, repeat de `H` = `max(0, w - APP_NAME.len() - 8)`), seções `Commands`/`Waybar`/`Flags`/`Info`, footer `┗━…` (`w=58`). Usar os helpers `v()`, `label`, `cmd_line`, `opt_line`, `info_line`, `wb_line` com `COL1=22` (`padEnd` → `format!("{:<22}")`). Cores via `theme::ColorToken` + `box_chars`; **gate `no_color`**: quando `true`, todos os códigos ANSI (e `ANSI_RESET`/`ANSI_BOLD`) viram `""` — espelha o `ANSI.*` do TS que embute `NO_COLOR` (theme.ts:42-61). Texto literal (nomes/descrições) é idêntico, incl. `Update the install (npm or managed checkout)`, `Run with` → `{APP_NAME}  or  bun run start` (2 espaços), `BOX.diamond` antes de cada label.
- Mapa de cor TS→Rust: `ANSI.magenta`→`ColorToken::Magenta`, `green`→`Green`, `yellow`→`Yellow`, `orange`→`Orange`, `textBright`→`TextBright`, `muted`→`Muted`, `comment`→`Comment`, `bold`→`ANSI_BOLD`, `reset`→`ANSI_RESET`. `padEnd(22)` conta chars (não bytes) — usar `format!("{:<22}")` (ASCII nos nomes, ok).

> Nota: `cli.ts` usa um alias `ANSI.comment` para `→` em `wb_line` e `info_line`; conferir linha-a-linha vs TS. `build_help` é puro (retorna String) → testável sem capturar stdout.

- [ ] **Step 1:** Testes: `build_help(false)` contém `Update the install (npm or managed checkout)` e `agent-bar  or  bun run start`; `build_help(true)` (no_color) NÃO contém `\x1b[` (nenhum escape ANSI) mas ainda contém os 2 needles de texto.
- [ ] **Step 2:** Rodar → falha. `cargo test --manifest-path rust/Cargo.toml cli 2>&1 | tail -6`.
- [ ] **Step 3:** Implementar `build_help` + `show_help`.
- [ ] **Step 4:** Testes passam; clippy limpo; `cargo fmt`.
- [ ] **Step 5:** Commit: `feat(rust): show_help do CLI`.

---

### Task 3: `providers/mod.rs` — helpers de lookup por id

**Files:**
- Modify: `rust/src/providers/mod.rs`
- Test: inline (no `#[cfg(test)] mod tests` existente).

**Interfaces (Produces):**
```rust
/// Provider de produção por id (espelha getProvider do TS). None se desconhecido.
pub fn get_provider(id: &str) -> Option<Box<dyn Provider>>;
/// Ids dos providers registrados, na ordem do registry (espelha getRegisteredProviderIds).
pub fn registered_provider_ids() -> Vec<&'static str>;
/// Quota de um único provider (espelha getQuotaFor). None se id desconhecido.
pub async fn get_quota_for(id: &str, ctx: &Ctx<'_>) -> Option<ProviderQuota>;
```

**Contrato — espelha `src/providers/index.ts` (getProvider/getQuotaFor/getRegisteredProviderIds):**
- `get_provider(id)`: `registry().into_iter().find(|p| p.id() == id)`.
- `registered_provider_ids()`: `registry().iter().map(|p| p.id()).collect()` → `["claude","amp","codex"]` (ordem do registry).
- `get_quota_for(id, ctx)`: `match get_provider(id) { Some(p) => Some(p.get_quota(ctx).await), None => None }`.

- [ ] **Step 1:** Testes: `get_provider("claude").is_some()`, `get_provider("nope").is_none()`; `registered_provider_ids() == ["claude","amp","codex"]`; (`get_quota_for` async — testar `get_quota_for("nope", &ctx).await.is_none()` via `test_support::ctx_for` com tempdir; não chamar provider real). Usar `#[tokio::test]`.
- [ ] **Step 2:** Rodar → falha. `cargo test --manifest-path rust/Cargo.toml providers::mod 2>&1 | tail -6` (ou filtro do nome do teste).
- [ ] **Step 3:** Implementar os 3 helpers.
- [ ] **Step 4:** Testes passam; clippy limpo; `cargo fmt`.
- [ ] **Step 5:** Commit: `feat(rust): helpers get_provider/get_quota_for`.

---

### Task 4: `action_right.rs` — disconnect-detection + handler

**Files:**
- Create: `rust/src/action_right.rs`
- Modify: `rust/src/lib.rs` (`pub mod action_right;`)
- Test: inline.

**Interfaces (Produces):**
```rust
/// True se o `error` do quota indica desconexão (token expirado/sem login).
/// Espelha as 2 regexes de action-right.ts:52-56 (case-insensitive).
pub fn looks_disconnected(provider_id: &str, error: Option<&str>) -> bool;
/// Waybar right-click: login (stub no Plano 5) se indisponível/desconectado;
/// senão refresh + view terminal. Espelha handleActionRight (sem o TUI de login).
pub async fn handle_action_right(provider_id: &str, ctx: &Ctx<'_>, clock: &Clock, no_color: bool);
```

**Contrato — porta de `src/action-right.ts:28-90`:**
- `looks_disconnected`: regexes (compiladas via `OnceLock<Regex>`, pattern `(?i)`):
  - base: `r"(?i)expired|not logged in|login again|please login"`
  - codex: `r"(?i)no session data|no rate limit data|auth|token"`
  - `error` é `Some(e)` não-vazio E (`base.is_match(e)` OU (`provider_id == "codex"` E `codex.is_match(e)`)). `None`/`Some("")` → false.
- `handle_action_right`:
  1. `provider_id` vazio → `log::error!("Usage: {APP_NAME} action-right <provider>")` + `std::process::exit(1)`.
  2. `get_provider(id)` `None` → `log::error!("Unknown provider: {id}")` + **stub de wait** (`wait_enter()`), depois `return`.
  3. `available = provider.is_available(ctx).await`. Se `!available` → **login stub** (ver abaixo) + `return`.
  4. `quota = provider.get_quota(ctx).await`. Se `looks_disconnected(id, quota.error.as_deref())` → **login stub** + `return`.
  5. Senão: refresh (`cache::invalidate(&ctx.paths.cache_dir, provider.cache_key())`), `fresh = get_quota_for(id, ctx).await`; se `Some` → `println!("{}", format_for_terminal(clock, &AllQuotas{providers:vec![fresh], fetched_at: iso_from_ms(ctx.now_ms)}, ctx.settings, ctx.settings.waybar.display_mode, no_color))`; senão `log::error!("Failed to fetch {} quota", provider.name())`. Depois `wait_enter()`.
- **login stub** (TUI não portado neste plano): `eprintln!`? NÃO — stdout limpo, mas action-right roda em terminal interativo (não Waybar). Usar `log::error!("Login interativo de '{id}' ainda não disponível na reescrita Rust (TUI: pendente).")` + `wait_enter()`. Registrar como gap conhecido no report.
- `wait_enter()`: lê uma linha de `std::io::stdin()` (bloqueante) — espelha `waitEnter` do TS (mantém o popup aberto). Em Plano 5 pode ser síncrono dentro do async (chamada curta no fim); aceitável. Se preferir, `tokio::task::spawn_blocking`. Implementer decide; manter simples.

> Os intros/`p.intro`/cores `@clack/prompts` do TS NÃO são portados (dependem do TUI). Só a lógica de roteamento + a saída terminal já-pronta. O único contrato testável é `looks_disconnected` (as 2 regexes).

- [ ] **Step 1:** Testes de `looks_disconnected`: `("claude", Some("Token expired"))→true`; `("claude", Some("please login"))→true`; `("amp", Some("no rate limit data"))→false` (codex-only pattern, provider≠codex); `("codex", Some("no rate limit data"))→true`; `("codex", Some("auth failed"))→true`; `("claude", Some("network blip"))→false`; `("claude", None)→false`; `("claude", Some(""))→false`. (Case-insensitive coberto por "Token expired".)
- [ ] **Step 2:** Rodar → falha. `cargo test --manifest-path rust/Cargo.toml action_right 2>&1 | tail -6`.
- [ ] **Step 3:** Implementar `action_right.rs` (`looks_disconnected` + `handle_action_right` + `wait_enter`). `pub mod action_right;` em lib.rs.
- [ ] **Step 4:** Testes passam; clippy limpo; `cargo fmt`.
- [ ] **Step 5:** Commit: `feat(rust): action-right (disconnect + refresh)`.

---

### Task 5: `watch.rs` — NDJSON serializado

**Files:**
- Create: `rust/src/watch.rs`
- Modify: `rust/src/lib.rs` (`pub mod watch;`)
- Test: inline (`build_watch_line`).

**Interfaces (Produces):**
```rust
/// Um snapshot como uma linha NDJSON (json + "\n"). Espelha buildWatchLine.
pub fn build_watch_line(quotas: &AllQuotas) -> String;
/// Emissor NDJSON long-running. Emite já, depois a cada `interval` APÓS o write
/// anterior completar (serializado, sem overlap). Sai 0 em EPIPE. Não retorna no
/// caminho normal. Espelha startWatch.
pub async fn start_watch(provider: Option<&str>, interval: Duration, ctx: &Ctx<'_>) -> std::io::Result<()>;
```

**Contrato — porta de `src/watch.ts`:**
- `build_watch_line(quotas)`: `format!("{}\n", to_json_string(quotas).unwrap_or_default())`. (Em produção `to_json_string` não falha; `unwrap_or_default` evita panic sem `unwrap`.)
- `start_watch`:
  1. Se `provider` `Some(id)` && `get_provider(id).is_none()` → `eprintln!`? Não — `log::error!`? O TS escreve em stderr `[agent-bar] Unknown provider: {id}` + exit(1). Como é diagnóstico antes de qualquer payload, usar `log::error!("[agent-bar] Unknown provider: {id}")` + `std::process::exit(1)`. (Aviso: o teste do contrato é o comportamento, não capturável aqui; manter o texto.)
  2. Se `std::io::stdout().is_terminal()` → `log::warn!("[agent-bar] watch mode: output is NDJSON — pipe to a consumer")`.
  3. Loop serializado: `let mut tick = interval(interval); loop { tick.tick().await; let quotas = fetch(provider, ctx).await; let line = build_watch_line(&quotas); match stdout.write_all(line.as_bytes()).await.and_then(flush) { Ok ⇒ continue, Err(e) if e.kind()==BrokenPipe ⇒ exit(0), Err(e) ⇒ log::error!+exit(1) } }`. Usar `tokio::time::interval` com `MissedTickBehavior::Delay` (serializa: próximo tick só depois do write). **Emite imediatamente no 1º tick** (`interval` dispara o 1º `tick()` na hora). Usar `tokio::io::stdout()` (async, backpressure real). Rust ignora SIGPIPE por default → write em pipe fechado retorna `BrokenPipe` (não mata o processo) — exatamente o que queremos.
  4. `fetch(provider, ctx)`: `provider.map(|id| get_quota_for(id, ctx)).` → `AllQuotas{providers: quota.map(|q| vec![q]).unwrap_or_default(), fetched_at: iso_from_ms(ctx.now_ms)}` no caso single; senão `fetch_all(&registry(), ctx).await`. **Nota:** `ctx.now_ms` é fixo no startup; para o `fetched_at` avançar entre ticks, recomputar via `config::now_ms()` por tick e usar `iso_from_ms`. (O TS usa `new Date().toISOString()` a cada tick.) → no caminho single, `fetched_at: iso_from_ms(config::now_ms())`.

> A serialização por backpressure do TS (agenda próximo tick só após o write) é replicada por `interval` + `tick().await` no topo do loop com `MissedTickBehavior::Delay`. O contrato testável é `build_watch_line` (puro); o loop é integração (validar por inspeção + smoke manual fora do escopo de teste).

- [ ] **Step 1:** Teste de `build_watch_line`: dado um `AllQuotas` fixo (via `test_support` ou montado à mão), a linha termina em `\n` e é JSON válido cujo conteúdo == `to_json_string(&quotas).unwrap()` (sem o `\n`). Verificar que há exatamente um `\n` no fim.
- [ ] **Step 2:** Rodar → falha. `cargo test --manifest-path rust/Cargo.toml watch 2>&1 | tail -6`.
- [ ] **Step 3:** Implementar `watch.rs`. `pub mod watch;` em lib.rs. (`Duration` de `std::time`; `tokio::time::{interval, MissedTickBehavior}`; `tokio::io::AsyncWriteExt`.)
- [ ] **Step 4:** Testes passam; clippy limpo; `cargo fmt`.
- [ ] **Step 5:** Commit: `feat(rust): watch NDJSON serializado`.

---

### Task 6: `main.rs` — wiring, gates, dispatch

**Files:**
- Rewrite: `rust/src/main.rs`
- Test: inline (apenas os gates puros extraídos — `should_notify`, `is_hidden_module`).

**Interfaces (Produces, puros e testáveis):**
```rust
/// Notify dispara? settings.notify.enabled && cmd==Waybar && format!=Json && !watch && !stdout_tty.
fn should_notify(settings: &Settings, command: Command, format: Format, watch: bool, stdout_is_tty: bool) -> bool;
/// Módulo Waybar oculto? provider dado && format!=Json && !settings.waybar.providers.contains(id).
fn is_hidden_module(provider: &str, format: Format, settings: &Settings) -> bool;
```

**Contrato — porta de `src/index.ts:25-228`:**

`#[tokio::main(flavor = "current_thread")] async fn main()`. Manter `#![cfg_attr(not(test), deny(...))]` no topo.

1. `let raw: Vec<String> = std::env::args().skip(1).collect();`
2. `let opts = match cli::parse_args(&raw) { Ok(o) => o, Err(e) => { eprintln!("{}", e.message); std::process::exit(1); } };` — **exceção justificada ao stdout-limpo:** erros de CLI vão pra stderr via `eprintln!` (não há Waybar lendo neste caminho; espelha `console.error`). Imprimir `opts.warnings` em stderr também (`for w in &opts.warnings { eprintln!("{w}"); }`) — espelha `console.error`/`logger.warn` do TS.
3. `logger::init(opts.verbose);` (verbose→Debug, senão Warn; já feito em logger.rs). No TS, sem verbose → silent; aqui Warn é aceitável (logs vão p/ stderr).
4. `let no_color = std::env::var_os("NO_COLOR").is_some();`
5. Short-circuits cedo (na ordem do TS):
   - `Help` → `cli::show_help(no_color); exit(0)`.
   - `Version` → `println!("{}", app_identity::VERSION); exit(0)`.
   - `Menu`/`Setup`/`AssetsInstall`/`ExportWaybarModules`/`ExportWaybarCss`/`Update`/`Uninstall`/`Remove`/`Doctor` → **stub Plano 6**: `log::error!("'{cmd}' ainda não implementado na reescrita Rust (Plano 6).")` + `exit(1)`. (Mapear cada Command a um label estável.)
6. Construir `Ctx` (CHECKLIST do ledger):
   ```rust
   let paths = match Paths::from_env() { Ok(p) => p, Err(e) => { log::error!("{e}"); std::process::exit(1); } };
   let settings = settings::load(&paths);
   let clock = Clock::from_env();
   let ctx = Ctx {
       client: http::client(),                              // NÃO Client::new()
       paths: &paths,
       settings: &settings,
       now_ms: config::now_ms(),
       local_offset: clock.local_offset,
       claude_usage_url: config::CLAUDE_USAGE_URL.to_string(),
       version: app_identity::VERSION,
       home: std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default(),
   };
   ```
   (Cache 5s de settings é otimização do hot-path Waybar — o TS tem em `waybar.ts`; aqui `settings::load` é barato e o processo é one-shot → **DEFERIR** o cache 5s, registrar como Minor. O loop `--watch` recarrega via `start_watch` que usa `ctx`/registry.)
7. `ActionRight` → `action_right::handle_action_right(opts.provider.as_deref().unwrap_or(""), &ctx, &clock, no_color).await; exit(0)`.
8. `if opts.refresh { for id in opts.provider.as_deref().map(|p| vec![p]).unwrap_or_else(|| registered_provider_ids()) { if let Some(p) = get_provider(id) { cache::invalidate(&paths.cache_dir, p.cache_key()); } } log::info!("Cache invalidated"); }` (espelha index.ts:144-152).
9. `if opts.watch { watch::start_watch(opts.provider.as_deref(), Duration::from_secs(opts.interval_seconds as u64), &ctx).await?; return; }` — antes do fetch. (Tratar o `Result`: `if let Err(e) = ... { log::error!(...); exit(1) }`.)
10. Fetch:
    - `if let Some(prov) = &opts.provider {` → hidden short-circuit: `if is_hidden_module(prov, opts.format, &settings) { print_waybar(&WaybarOutput{text:"".into(),tooltip:"".into(),class:APP_HIDDEN_CLASS.into(),alt:None,percentage:None}); exit(0); }` → `let quota = match get_quota_for(prov, &ctx).await { Some(q) => q, None => { log::error!("Unknown provider: {prov}"); exit(1); } }; let quotas = AllQuotas{providers:vec![quota], fetched_at: iso_from_ms(config::now_ms())};`
    - `else {` → `let mut quotas = fetch_all(&registry(), &ctx).await; if opts.command==Waybar && opts.format!=Json { quotas.providers.retain(|p| settings.waybar.providers.iter().any(|s| s==&p.provider)); } }`
11. `if opts.format == Json { println!("{}", to_json_string(&quotas).unwrap_or_default()); exit(0); }`
12. `let mode = settings.waybar.display_mode;` Dispatch final:
    - `Terminal | Status` → `println!("{}", format_for_terminal(&clock, &quotas, &settings, mode, no_color));`
    - `_` (Waybar default):
      - `let stdout_tty = std::io::stdout().is_terminal();`
      - `if stdout_tty && raw.is_empty() { cli::show_help(no_color); }` else:
        - `if opts.provider.is_some() && quotas.providers.len() == 1 { print_waybar(&format_provider_for_waybar(&clock, &quotas.providers[0], &settings, mode)); } else { print_waybar(&format_for_waybar(&clock, &quotas, &settings, mode)); }`
        - `if should_notify(&settings, opts.command, opts.format, opts.watch, stdout_tty) { notify::check_and_notify(&quotas, &paths.cache_dir).await; }`
13. Helper de output isolado (stdout-limpo): `fn print_waybar(o: &WaybarOutput) { println!("{}", serde_json::to_string(o).unwrap_or_default()); }`. (Serialização de WaybarOutput não falha; `unwrap_or_default` evita `unwrap`.)

`should_notify` lógica: `settings.notify.enabled && matches!(command, Command::Waybar) && format != Format::Json && !watch && !stdout_is_tty`. (No TS o gate é `settings.notify?.enabled && !isTTY` DENTRO do branch default/waybar pós-output — os outros predicados são implícitos por chegar ali. Tornamos explícito p/ testabilidade.)

`is_hidden_module` lógica: `format != Format::Json && !settings.waybar.providers.iter().any(|s| s==provider)`.

> **Gaps conhecidos a registrar no report (não-bugs, escopo posterior):** (a) comandos de install/TUI são stubs (Plano 6); (b) `menu` + login interativo do action-right pendem de um TUI ainda não no roadmap — sinalizar ao usuário; (c) cache 5s de settings deferido (one-shot não precisa); (d) parity de sinal SIGTERM/SIGINT→exit 0 não portada (Rust default termina; EPIPE no watch coberto) — Minor.

- [ ] **Step 1:** Testes de `should_notify` (enabled+waybar+!json+!watch+!tty→true; tty→false; json→false; watch→false; terminal→false; notify.enabled=false→false) e `is_hidden_module` (provider fora de `waybar.providers` & !json→true; dentro→false; json→false mesmo fora). Montar `Settings` via `settings::load(&paths_in(tempdir))` ou `providers::test_support::settings()`.
- [ ] **Step 2:** Rodar → falha (fns não existem). `cargo test --manifest-path rust/Cargo.toml main 2>&1 | tail -6` (ou nome do teste; `main.rs` testes rodam no binário — usar `#[cfg(test)]` no main.rs).
- [ ] **Step 3:** Reescrever `main.rs` completo (async, dispatch, gates, helpers, stubs). Imports necessários (`std::io::IsTerminal`, `std::path::PathBuf`, `std::time::Duration`, `agent_bar::{...}`). Manter o atributo deny no topo.
- [ ] **Step 4:** `cargo test --manifest-path rust/Cargo.toml 2>&1 | tail -6` (suíte inteira verde); `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings` limpo; `cargo fmt`.
- [ ] **Step 5:** Smoke manual (não-destrutivo, só leitura): `cargo run --manifest-path rust/Cargo.toml -- --version` (imprime versão); `... -- help | head` (ajuda); `... -- --format json -p claude` (JSON, sem tocar desktop). Registrar saídas no report. Commit: `feat(rust): main async + dispatch do CLI`.

---

## Self-Review (autor)

- **Cobertura do spec:** parsing (T1) + help (T2) + lookup helpers (T3) + action-right (T4) + watch (T5) + main/gates/dispatch/notify/hidden/stubs (T6) = todos os itens do CHECKLIST do ledger (Ctx, fetch_all, formatação com Clock+no_color injetados, check_and_notify gateado, watch serializado+EPIPE, hidden-module short-circuit, stdout limpo, test_support nos testes). ✅
- **Comandos não-implementados:** parseados (tests exigem) mas roteados a stub Plano 6 — explícito, não silencioso. ✅
- **Sem placeholder:** cada task tem contrato + casos de teste verbatim do TS + assinaturas exatas. Strings de erro/help copiadas do TS. ✅
- **Consistência de tipos:** `Command`/`Format`/`CliOptions` definidos em T1 e consumidos por T6; helpers de T3 consumidos por T4/T5/T6; `looks_disconnected`/`handle_action_right` de T4; `build_watch_line`/`start_watch` de T5. ✅
- **Decisão registrada:** `parse_args` é port à mão (não clap) — justificado pelos strings de erro verbatim + levenshtein serem contrato. Sinalizar ao usuário no fechamento (desvia da nota "clap derive" do resume, mas alinhado ao princípio travado "byte-exact = TS").
