# Reescrita Rust — RESUME / Handoff (ler PRIMEIRO ao retomar)

> **Propósito:** retomar a reescrita TS→Rust do agent-bar SEM perder decisões, estratégia
> ou estado, mesmo após `/compact` ou sessão nova. Este doc é o índice; os detalhes vivem no
> spec + planos commitados. Modo de trabalho: **autônomo até travar/acabar** (escolha do usuário).

## 0. Como retomar (passos para um agente fresco)

1. `git -C /home/othavio/Projects/agent-bar log --oneline -25` e `git branch --show-current` (deve ser `rust-rewrite`).
2. Ler este doc inteiro.
3. Ler o ledger vivo: `cat .superpowers/sdd/progress.md` (estado por-task, commits, decisões; é scratch git-ignored mas persiste no disco — se sumir, reconstruir do `git log`).
4. Ler o spec: `docs/superpowers/specs/2026-06-19-rust-rewrite-design.md` (28 contratos + decisões travadas).
5. Ver qual o próximo plano não-feito (§4 abaixo) e continuar o **loop subagent-driven** (§3).
6. **Não re-despachar tasks já marcadas completas no ledger.** Confiar no ledger + `git log`.

## 1. Estado atual

- **Branch:** `rust-rewrite` (corta de `master` em `1eae7ac`; docs do spec/plano estão em `master` e na branch).
- **Crate Rust vive em `rust/`** durante a migração (TS fica na raiz, rodável p/ paridade/shadow-mode; promove pra raiz no cutover do Plano 7).
- **Verificação:** `cargo test --manifest-path rust/Cargo.toml` + `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`. (RTK trunca output multi-suíte — somar suítes com `grep -Eo "[0-9]+ passed"` ou `grep "test result"`.)
- **Feito:** Planos 01 (foundation), 02 (render primitives), 03a (format/builder primitives), 03b (builders por-provider), **03c (assembly terminal/waybar + golden snapshots de paridade byte-exact)**. **180 testes** (5 suítes, golden inclusa), clippy clean. Branch @ `88c6389` (3c em review de branch Opus; se o review pedir fix, aplicar e re-stampar). **Toda a camada de formatação está completa e com paridade travada vs TS.** Próximo: **Plano 4 (providers async/tokio)**.
- **Toolchain:** Rust 1.95.0, cargo 1.95.0 (confirmado instalado).

## 2. Decisões TRAVADAS (não relitigar) + princípios invioláveis

**Decisões (do usuário):**
- Runtime: **async tokio (`current_thread`) + reqwest** — só na camada de providers (Plano 4); módulos puros são sync.
- Modo: **subagent-driven** (eu codifico via subagents, usuário revisa no fim). Código idiomático + comentado.
- Distribuição: **AUR + cargo-binstall + install.sh; DROPAR npm**.
- Migração: **incremental em camadas**, testes portados como gate por task.
- Frescor: **interval 60s + TTL per-provider** (Claude 300s, Codex/Amp 90s), configurável em settings. Único desvio intencional de paridade.

**Princípios invioláveis (críticos):**
- **Contrato byte-exact do Waybar/Pango é sagrado.** A autoridade é a saída do TS. "Rust == saída do TS".
- ⚠️ **REJEITAR findings de review que divergiriam do TS.** Já aconteceu 2×: (a) "filtrar segments vazios na barra" — o TS emite span vazio a 0%/100%, filtrar quebraria paridade; (b) "tirar max(1) do footer" — o TS usa `max(1,...)`. SEMPRE conferir o comportamento real do TS antes de aceitar um "fix" de review. Validar findings contra a fonte, não aceitar cego.
- **Sem `unwrap()`/`expect()` em produção** (enforçado por `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]` em `rust/src/lib.rs` E `rust/src/main.rs`). Em teste é permitido.
- **Sem estado global mutável.** `Paths` (config) e `Clock` (tempo) são injetados via DI, resolvidos no `main`. (Exceção planejada: cache 5s de settings em `waybar.rs`, Plano 3c — é cache de valor já-carregado, não config.)
- **stdout limpo** (só JSON/NDJSON/view-rich); logs → stderr (`log::warn!`, não `eprintln!`).
- `ProviderQuota` é **serialize-only** (cache guarda o RAW do provider, não o quota normalizado).
- **Paridade verificada:** primitivos de render conferidos byte-a-byte vs TS real (rodando `bun` num script temp). 7/8 idênticos.
- **Pendência 60.0/60 — DECIDIDA (a aplicar no 3c):** ACEITAR `60.0` (f64) e comparar o golden de JSON por VALOR parseado (`serde_json::Value`), não por string byte-exact. Modelar `remaining`/`used` como inteiro seria BUG (perde percentuais fracionários). Só o Pango é byte-exact; o JSON é contrato parseado.

## 3. O loop subagent-driven (executar por task)

Para CADA task de um plano:
1. `scripts/task-brief <plano.md> <N>` (script em `/home/othavio/.claude/plugins/cache/superpowers-marketplace/superpowers-dev/6.0.3/skills/subagent-driven-development/scripts/`) → escreve `.superpowers/sdd/task-N-brief.md`.
2. Registrar a BASE = `git rev-parse --short HEAD` (commit antes do implementer).
3. Despachar **implementer** (`Agent`, `subagent_type: general-purpose`, `model: sonnet`): passar caminho do brief + contexto (rust/ subdir, --manifest-path, atributo no lib.rs, "rodar cargo fmt ANTES do git add", "Read antes de Edit / re-Read se falhar", report em `.superpowers/sdd/task-N-report.md`). Template: `implementer-prompt.md` do skill.
4. Ao DONE: **verificar o filesystem eu mesmo** (não confiar no report): `git log`, `git status` (tree limpa?), grep de invariantes, re-rodar `cargo test`+`clippy`. (Regra: implementer "came to rest" pode reportar DONE com trabalho no meio.)
5. `scripts/review-package <BASE> <HEAD>` → diff file. Despachar **task-reviewer** (`general-purpose`, `sonnet`) com brief+report+diff+constraints. Template: `task-reviewer-prompt.md`.
6. Findings **Critical/Important** → fix (subagent OU inline se ≤poucas linhas). **Minor → deferir** pro review final (registrar no ledger). **SEMPRE checar se o finding divergiria do TS antes de aceitar.**
7. Marcar no ledger: `P0X Task N (nome): complete (commits base..head, review clean)`.

Ao fim de cada plano: **review de branch do incremento** (`Agent`, `model: opus`, template `requesting-code-review/code-reviewer.md`) sobre `<plano-base>..HEAD`, enquadrado como "sound to build the next layer on?". Triar Minors. Aplicar julgamento crítico (Opus já sugeriu 2 fixes que quebrariam paridade).

**Gotchas de execução:** `cargo fmt` órfão (se eu commitar manualmente sem fmt, o fmt do próximo implementer suja a tree — sempre fmt antes de commitar). `.superpowers/` está gitignored (scratch). Commits: Conventional Commits PT, subject ≤50 chars.

## 4. Roadmap restante (autorar just-in-time lendo o TS)

Cada plano: ler o TS-fonte relevante → autorar plano com código exato (não chutar API; verificar) → executar via loop §3.

- **✅ 03b — builders por-provider FEITO** (commits 1b178b5..08684ad): `providers/extras.rs` (getters), `formatters/codex_helpers.rs` + `view_model.rs` (CodexModelEntry/codex_models_from_quota/apply_codex_model_filter/CodexViewModel/resolve_codex_view_model_from), `normalize_plan_label` em shared.rs, `builders/{generic,claude,codex,amp}.rs`. Review de branch Opus: sólido. Minors deferidos no ledger (M1 str::cmp, M3 error="").
- **✅ 03c — assembly + golden FEITO** (commits 4e4b073..88c6389): T1 `HealthStatus::as_str`+`APP_BASE_CLASS`; T2 `terminal.rs::format_for_terminal(clock, quotas, settings, mode, no_color)`; T3 `waybar.rs::{WaybarOutput, format_for_waybar, format_provider_for_waybar}`; T4 `tests/golden.rs` (77 testes insta + filters de sanitização replicando o `sanitize()` do TS, Clock fixo em FIXED_FETCHED_AT). Paridade byte-exact travada (D1 footer cached = staleness do snap TS, aceito). Funções PURAS (DI). **DEFERIDO p/ Plano 5:** cache 5s settings, hidden-module short-circuit, `outputTerminal`/`outputWaybar` (stdout/console.log). insta é dev-dependency.
- **04 — providers (async/tokio entra aqui) — PRÓXIMO.** Ver detalhe abaixo. As funções de formatação (3c) consomem o `ProviderQuota`/`AllQuotas` que o Plano 4 vai PRODUZIR.
- **04 — providers (async/tokio entra aqui):** `http.rs` (static `OnceLock<reqwest::Client>` com UA `claude-code/2.1.179` + beta header), `providers/{error,base,claude,codex,amp,amp_cli,extras}.rs`, `registry`, fan-out (`join_all`+timeout 10s+1 retry), `notify.rs` (spawn `notify-send`). **Codex app-server** (JSON-RPC stdio, `tokio::process`+`select!`, grace 200ms, timeout 4s, fallback `.jsonl` via `walkdir`) é o mais difícil. Claude: expiry pré-request + check `token_expired` pós-cache. Amp: drain stderr concorrente. Testes: trait injection (não há análogo a `mock.module`); `wiremock` p/ HTTP; `tokio::io::duplex` p/ app-server. `async-trait` p/ `dyn Provider`.
- **05 — CLI:** `cli.rs` (clap derive, nested `Export`/`Assets`, alias `-t`), dispatch, `--watch` (loop sequencial backpressure, EPIPE→exit 0), `action_right` (2 regexes de disconnect), `reload_waybar` (pkill -SIGUSR2). hidden-module short-circuit ANTES de fetch.
- **06 — install:** `waybar_integration.rs` (PORTAR o scanner cirúrgico de string, NÃO usar crate de JSONC), `waybar_contract.rs` (+ asset resolution 3-vias: AGENT_BAR_ASSET_DIR / /usr/share / dev), setup/uninstall/remove, update (4-vias install-kind), doctor (deprecação + limpeza legacy npm). `agent-bar-open-terminal` fica como Bash. `install.rs` (ensure_command/ensure_amp_cli).
- **07 — dist:** target musl + perfil release (opt-level=z/lto/strip; SEM panic=abort, SEM mimalloc), cargo-dist + tarball (binário + scripts/agent-bar-open-terminal + icons/), PKGBUILD, install.sh; remover npm (`package.json`/`bun.lock`/`dist/`/`scripts/agent-bar`); promover `rust/` pra raiz; reescrever CLAUDE.md/docs; versão via `CARGO_PKG_VERSION`; check:pkgver Cargo-based.

## 5. Minors deferidos p/ o review pré-merge final (no ledger, resumo)

VERSION sem doc; main exit-branch sem teste; now_ms u128→u64; settings Eq derive + 2 testes 1-linha; render_pango 2 testes de polish; json `.get().is_none()`; classify_window boundary inferior; titlecase edge; `time/macros` em [deps] (trocar `datetime!` por `parse` nos testes p/ dropar); footer doc BL=1char; `_dashes` var morta. Nenhum é bug; todos baixo-risco.

## 6. Ponteiros

- Spec: `docs/superpowers/specs/2026-06-19-rust-rewrite-design.md` (v2, 28 contratos).
- Planos: `docs/superpowers/plans/2026-06-19-rust-rewrite-0{1,2,3a}-*.md` (+ 3b/3c/4/5/6/7 a criar).
- Ledger vivo: `.superpowers/sdd/progress.md`.
- Este doc: `docs/superpowers/rust-rewrite-resume.md`.
