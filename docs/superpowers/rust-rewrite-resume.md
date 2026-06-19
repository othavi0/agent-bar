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
- **Verificação:** `cargo test --manifest-path rust/Cargo.toml` + `cargo clippy --manifest-path rust/Cargo.toml --all-targets -- -D warnings`. **Gotcha RTK (confirmado):** o RTK reformata o output do cargo para `cargo test: N passed (K suites, …)` — **NÃO existe a string `test result:`** (não grep por ela). Leia o output bruto com `... 2>&1 | tail -6` e some os `passed`. **`cargo test` aceita só UM filtro posicional** (`cargo test foo bar` dá erro de usage). Para um clean check use `cargo clippy: No issues found`.
- **Feito:** Planos 01-03c (formatação) + **04a/04b/04c/04d = PLANO 4 COMPLETO** (providers async + fan-out + notify). **292 testes** (5 suítes, 58 golden), clippy clean. Branch @ `66b54d4`. Reviews de branch Opus 04a/04b/04c/04d = "sólido/byte-exact, SIM" (zero Critical). **Toda a camada de formatação + a camada de providers async (3 providers reais: Claude HTTP, Amp subprocess, Codex app-server+fallback) + notify estão 100% prontas e byte-exact vs TS.** Próximo: **Plano 5 (CLI/main) — só plumbing, zero mudança de providers/notify**, depois 6 (install), 7 (dist/cutover). **O CHECKLIST DE INTEGRAÇÃO do Plano 5 está no ledger** (`.superpowers/sdd/progress.md`, fim — construir Ctx, fetch_all, formatação, check_and_notify gateado, --watch serializado, hidden-module short-circuit, stdout limpo).
- **04b entregou (não re-fazer):** `Ctx` ganhou `home: PathBuf`; `QuotaSource::build_quota` ganhou `&Ctx` (Amp recalcula fullAt com now atual); `providers/amp_cli.rs` (locator: `amp_candidate_paths`/`find_amp_bin_with`(seams)/`which_in_path`/`find_amp_bin` + `AMP_INSTALL_COMMAND`); `providers/amp.rs` (`parse_usage` regex byte-exact — dollar via f64 Display, replenishRate/bonus cru, fullAt guards; `run_amp_usage` spawn via `tokio::process` com `wait_with_output`(drena stderr)+`kill_on_drop`+timeout5s; `AmpProvider` impl QuotaSource(Raw=String)+Provider); `registry()=[claude,amp]`.
- **NOTAS p/ o Plano 5 (do review de branch, hard requirements):** (a) `ctx.home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()` — omitir silencia o Amp (NotInstalled em todo sistema, sem erro visível); (b) `ctx.client = http::client()` (NÃO `Client::new()` — perderia UA+anthropic-beta).
- **04a entregou (não re-fazer):** `http.rs` (cliente reqwest compartilhado via `OnceLock`, UA `claude-code/2.1.179` + default header `anthropic-beta` + timeout 5s); `providers/error.rs` (ClaudeError/CodexError/AmpError + ProviderError, strings verbatim); troca `models`/`weekly_models`→**IndexMap** (ordem do `Object.entries` do TS; `models_detailed` fica BTreeMap pq Codex re-ordena por severity; `meta` BTreeMap pq acesso por chave); `providers/base.rs` (`get_or_fetch` cache-só-em-sucesso + trait `QuotaSource{type Raw}` + `base_get_quota`); `providers/mod.rs` (`Ctx{client,paths,settings,now_ms,local_offset,claude_usage_url,version}` + `ttl_ms()`; trait `Provider{id,name,cache_key,is_available,get_quota->ProviderQuota nunca-erra}` `#[async_trait(?Send)]`; `registry()`; `fetch_all` join_all+timeout10s+1retry; `iso_from_ms`=toISOString; `#[cfg(test)] test_support{ctx_for,settings,paths_in}`); `providers/claude.rs` (`derive_claude_plan` via `OnceLock<Option<Regex>>` + `ClaudeProvider` impl `Provider` DIRETO). Testes: wiremock (HTTP) + `#[tokio::test(start_paused)]` (timeout virtual) + tempdir (Paths).
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
- ⚠️ **EXCEÇÃO APROVADA PELO USUÁRIO (Plano 5) — ordem dos providers:** o agregado Waybar (texto/tooltip/classe) + `--format json` usam ordem de REGISTRO. TS atual = `[amp, claude, codex]` (acidente do grafo de imports JS; o TS NÃO aplica `providerOrder` no agregado). Rust `registry()` = `[claude, amp, codex]`. **O usuário escolheu MANTER o Rust `[claude, amp, codex]`** — diverge do TS de propósito (a "autoridade" TS aqui é um acidente frágil). **NÃO reordenar p/ casar com o TS nem aplicar providerOrder; NÃO "consertar" em review futuro.** É decisão deliberada. (Goldens 3c montam AllQuotas à mão → não pegam a ordem real do registry.)
- ⚠️ **REJEITAR findings de review que divergiriam do TS.** Já aconteceu 2×: (a) "filtrar segments vazios na barra" — o TS emite span vazio a 0%/100%, filtrar quebraria paridade; (b) "tirar max(1) do footer" — o TS usa `max(1,...)`. SEMPRE conferir o comportamento real do TS antes de aceitar um "fix" de review. Validar findings contra a fonte, não aceitar cego.
- **Sem `unwrap()`/`expect()` em produção** (enforçado por `#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]` em `rust/src/lib.rs` E `rust/src/main.rs`). Em teste é permitido.
- **Sem estado global mutável.** `Paths` (config), `Clock` (tempo) e `no_color` (gate ANSI) são injetados via DI. As funções de assembly (`format_for_terminal`/`format_for_waybar`) são PURAS — recebem `&Settings`/`&Clock`/`no_color`, NÃO leem env/disco/relógio. (Exceção planejada: cache 5s de settings — DEFERIDA p/ o **Plano 5** (CLI hot path), não está no 3c.) Lição: um implementer leu `NO_COLOR` do env dentro de `format_for_terminal` → corrigido p/ injeção antes do review.
- **stdout limpo** (só JSON/NDJSON/view-rich); logs → stderr (`log::warn!`, não `eprintln!`). As funções `format_*` retornam String/struct; quem imprime é o CLI (Plano 5).
- `ProviderQuota` é **serialize-only** (cache guarda o RAW do provider, não o quota normalizado). `extra` é enum `ProviderExtra::{Claude,Codex,Amp}` (untagged); getters em `providers/extras.rs` com gate por `provider`-string + variante.
- **Paridade TOTAL verificada (3c):** 58 golden snapshots (`rust/tests/golden.rs`, `insta` + filters de sanitização) batem byte-a-byte com o `.snap` do TS. Única exceção: **D1** — a contagem de traços do footer `cached` no `.snap` TS é *stale* (gerado com relógio real; a sanitização troca o texto do ago por `__AGO__` mas não recalcula os traços). O Rust usa clock fixo → valor determinístico-correto. `build_footer_line` é port fiel. NÃO é bug. **Lição: o `.snap` TS pode ter staleness no footer — confiar na fórmula, não no snap nesse ponto.**
- **Pendência 60.0/60 — RESOLVIDA:** `remaining`/`used` ficam `f64` (60.0). NÃO há golden de JSON (os testes de JSON do TS usam asserts diretos, não snapshots), então não-issue. Não relitigar.
- **Ordem de maps (nota de fidelidade):** `models`/`weekly_models`/`models_detailed` são `BTreeMap` (alfabético) vs `Object.entries` (insertion-order) do TS. Bate com os golden atuais (chaves já alfabéticas; Codex re-ordena por severity). Se um dado real não-alfabético divergir nos golden → trocar p/ `IndexMap` no tipo (não no builder).

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

**Gotchas de execução (acumulados):** (a) `cargo fmt` órfão — sempre fmt antes de commitar manualmente. (b) `.superpowers/` é gitignored (scratch). (c) Commits: Conventional Commits PT ≤50 chars. (d) RTK reformata cargo (ver §1) — sem `test result:`; um filtro posicional só. (e) Adiantar o `task-brief N+1` enquanto o reviewer da task N roda economiza round-trips (reviewer é read-only, não conflita). (f) Verificar o fs eu mesmo ao DONE (commit existe? tree limpa? grep de invariantes? suíte+clippy?) — implementers reportam "DONE" com detalhes a corrigir (ex: NO_COLOR do env). (g) `clippy::field_reassign_with_default` força `..Default::default()` em vez de reassign — implementers às vezes ajustam testes por isso (ok). (h) Em golden/insta: NUNCA `cargo insta accept` cego — o golden vem do `.snap` TS, não do output Rust; divergência inesperada → reportar, não mascarar.

## 4. Roadmap restante (autorar just-in-time lendo o TS)

Cada plano: ler o TS-fonte relevante → autorar plano com código exato (não chutar API; verificar) → executar via loop §3.

- **✅ 03b — builders por-provider FEITO** (commits 1b178b5..08684ad): `providers/extras.rs` (getters), `formatters/codex_helpers.rs` + `view_model.rs` (CodexModelEntry/codex_models_from_quota/apply_codex_model_filter/CodexViewModel/resolve_codex_view_model_from), `normalize_plan_label` em shared.rs, `builders/{generic,claude,codex,amp}.rs`. Review de branch Opus: sólido. Minors deferidos no ledger (M1 str::cmp, M3 error="").
- **✅ 03c — assembly + golden FEITO** (commits 4e4b073..88c6389): T1 `HealthStatus::as_str`+`APP_BASE_CLASS`; T2 `terminal.rs::format_for_terminal(clock, quotas, settings, mode, no_color)`; T3 `waybar.rs::{WaybarOutput, format_for_waybar, format_provider_for_waybar}`; T4 `tests/golden.rs` (77 testes insta + filters de sanitização replicando o `sanitize()` do TS, Clock fixo em FIXED_FETCHED_AT). Paridade byte-exact travada (D1 footer cached = staleness do snap TS, aceito). Funções PURAS (DI). **DEFERIDO p/ Plano 5:** cache 5s settings, hidden-module short-circuit, `outputTerminal`/`outputWaybar` (stdout/console.log). insta é dev-dependency.
- **✅ 04a — provider infra + Claude FEITO** (commits 6970494..24415c4): ver "04a entregou" em §1. Review de branch Opus: sólido. Plano: `docs/superpowers/plans/2026-06-19-rust-rewrite-04a-provider-infra.md`.
- **✅ 04b — Amp FEITO** (commits 153c987..3706d19): ver "04b entregou" em §1. Review de branch Opus: sólido. Plano: `docs/superpowers/plans/2026-06-19-rust-rewrite-04b-amp.md`.
- **✅ 04c — Codex FEITO** (commits 0e41ed1..c21a44c): `codex.rs` (tipos internos snake_case `CodexRateLimits`=Raw + app-server camelCase; `build_codex_quota`; `normalize_appserver_rate_limits`; `find_latest_session_file`+`extract_rate_limits`; `run_appserver_protocol` sobre AsyncRead/AsyncWrite testado c/ `tokio::io::duplex`; `CodexProvider`). `registry()=[claude,amp,codex]`. `buckets`=IndexMap (ordem→sufixo dedup); `models_detailed`=BTreeMap (view-model re-ordena). Reviews Opus T1/T4 + branch = sólido. Minors deferidos no ledger (incl. CodexCredits interno #[serde(default)] paralelo). Plano: `docs/superpowers/plans/2026-06-19-rust-rewrite-04c-codex.md`.
- **✅ 04d — notify FEITO** (commits bb1253b..66b54d4): `src/notify.rs` (`plan_notifications` PURO + `check_and_notify` async IO; `ProviderNotifyState.windows`=BTreeMap<String,String>; thresholds 90/95; dedup alias; no-op-sem-persist se notify-send ausente; fire-and-forget). Review Opus byte-exact. **= PLANO 4 FECHADO.** Plano: `docs/superpowers/plans/2026-06-19-rust-rewrite-04d-notify.md`.
- **05 — CLI:** `cli.rs` (clap derive, nested `Export`/`Assets`, alias `-t`), dispatch, `--watch` (loop sequencial backpressure, EPIPE→exit 0), `action_right` (2 regexes de disconnect), `reload_waybar` (pkill -SIGUSR2). hidden-module short-circuit ANTES de fetch. `main.rs` vira `#[tokio::main(current_thread)]`. **Construir o `Ctx` injetando `crate::http::client()` como `ctx.client` (NÃO `Client::new()` — perderia UA+anthropic-beta default), `now_ms`=`config::now_ms()`, `local_offset`=`UtcOffset::local_offset_at`, `claude_usage_url`=`config::CLAUDE_USAGE_URL`, `version`=`env!("CARGO_PKG_VERSION")`.** Cache 5s de settings + leitura de `NO_COLOR` do env entram aqui (o `no_color` é injetado nas funções de formatação). notify gates (stdout não-TTY, settings.notify.enabled, comando=waybar, !json, !watch) ficam aqui.
- **06 — install:** `waybar_integration.rs` (PORTAR o scanner cirúrgico de string, NÃO usar crate de JSONC), `waybar_contract.rs` (+ asset resolution 3-vias: AGENT_BAR_ASSET_DIR / /usr/share / dev), setup/uninstall/remove, update (4-vias install-kind), doctor (deprecação + limpeza legacy npm). `agent-bar-open-terminal` fica como Bash. `install.rs` (ensure_command/ensure_amp_cli).
- **07 — dist:** target musl + perfil release (opt-level=z/lto/strip; SEM panic=abort, SEM mimalloc), cargo-dist + tarball (binário + scripts/agent-bar-open-terminal + icons/), PKGBUILD, install.sh; remover npm (`package.json`/`bun.lock`/`dist/`/`scripts/agent-bar`); promover `rust/` pra raiz; reescrever CLAUDE.md/docs; versão via `CARGO_PKG_VERSION`; check:pkgver Cargo-based.

## 5. Minors deferidos p/ o review pré-merge final (autoridade = ledger `.superpowers/sdd/progress.md`)

Do 01/02/03a: VERSION sem doc; main exit-branch sem teste; now_ms u128→u64; settings Eq derive + 2 testes 1-linha; render_pango 2 testes polish; json `.get().is_none()`; classify_window boundary inferior; titlecase edge; `time/macros` em [deps]; footer doc BL=1char; `_dashes` var morta. Do 03b: `str::cmp` vs `localeCompare` no sort Codex (sem impacto ASCII); `error==Some("")` dispara ramo de erro nos 4 builders (TS `if(p.error)` é falsy — inalcançável, documentar como invariante do provider no Plano 4); `.length`(UTF-16) vs `chars().count()` em max_len. Do 03c: D1 (footer cached, staleness do snap TS — aceito); `waybar_amp_healthy_used_class` sem golden (TS não tem; não inventar). **Nenhum é bug; todos baixo-risco. O ledger tem o detalhe por-task.**

## 6. Ponteiros

- Spec: `docs/superpowers/specs/2026-06-19-rust-rewrite-design.md` (v2, 28 contratos).
- Planos commitados: `docs/superpowers/plans/2026-06-19-rust-rewrite-{01,02,03a,03b,03c}-*.md` (4/5/6/7 a criar just-in-time).
- Ledger vivo (autoridade do estado por-task): `.superpowers/sdd/progress.md`.
- Este doc: `docs/superpowers/rust-rewrite-resume.md`.
- **Docs do PROJETO** (`README.md`, `docs/architecture.md`, etc.) descrevem o **TS vigente** (canônico/publicado) — NÃO tocar até o cutover (Plano 7 reescreve). A reescrita Rust vive só em `rust/` + `docs/superpowers/`.
