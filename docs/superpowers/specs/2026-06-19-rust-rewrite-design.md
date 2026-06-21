# Reescrita do agent-bar em Rust — Design (v2, pós-review adversarial)

**Data:** 2026-06-19
**Status:** Proposto (aguardando aprovação)
**Escopo:** Reescrever o agent-bar (hoje TypeScript/Bun, ~7.5k LOC) como um binário Rust
estático único, com paridade de comportamento e dos contratos testados.
**v2:** integra um painel adversarial de 4 lentes (fidelidade / idioma Rust / superfície / risco)
que leu o código TS real. Mudanças destacadas com ⟳.

---

## 1. Contexto e objetivos

O agent-bar é um **CLI efêmero**, não um app de UI: cada poll do Waybar sobe um processo que
busca quota de 3 providers (Claude, Codex, Amp), formata e imprime JSON no stdout, e morre. Sem
daemon (exceto `--watch`, que mantém um processo e streama NDJSON).

**Por que Rust:** (1) **velocidade** — o gargalo é o cold-start do binário `bun --compile`
(~50-100 MB, embute VFS) resubido a cada poll; um binário Rust de ~3-6 MB elimina isso;
(2) **aprender Rust idiomático**; (3) **distribuição** — binário estático real torna o wrapper npm
redundante.

**Não-objetivos (YAGNI):** features novas; Windows/macOS; GUI ou TUI full-screen (ratatui);
compat com nomes legacy (`qbar`, `antigravity`, `llm-usage` — mortos).

**Único desvio intencional da paridade ⟳ (frescor de usage):** o cold-start barato do Rust torna
polls baratos, então **desacoplamos** os dois knobs de frescor: poll interval do Waybar **120s→60s**
(barra/contagem mais responsivas, lê do cache, não bate API a mais) e **TTL per-provider** —
Claude **5 min** (preso ao rate-limit do endpoint OAuth, que dispara 429), Codex/Amp **~90s**
(subprocessos locais, sem rate-limit de rede). Ambos **configuráveis** em settings. Isso NÃO baixa
o TTL do Claude (constraint de rate-limit, não de velocidade) — separar os eixos é o ponto.

---

## 2. Decisões travadas

| # | Decisão | Escolha | Implicação |
|---|---|---|---|
| 1 | Runtime/concorrência | **Async tokio (`current_thread`) + reqwest** | Módulos puros primeiro; async só na camada de providers. |
| 2 | Modo de trabalho | **Subagent-driven** (eu codifico, você revisa) | Código idiomático + comentado; build via subagents com gate. |
| 3 | Distribuição | **AUR + cargo-binstall + install.sh; dropar npm** | Pipeline de release simplificado. |
| 4 | Migração | **Build incremental em camadas** | Ordem com testes portados como gate por camada. |

---

## 3. Contratos invioláveis (a reescrita falha se quebrar qualquer um)

Testados byte-a-byte hoje. Fonte da verdade do comportamento. ⟳ = adicionado/corrigido na v2.

### 3.1 Saída e segurança
1. **stdout limpo.** Só payload de máquina (JSON Waybar / NDJSON / view-rich do terminal). Todo
   log vai pra stderr.
2. **Strings de erro verbatim** (~13). Viram `Display` de enums `thiserror`, aseridas com
   `to_string()`. Lista canônica em §5.4.
3. **Pango byte-exact.** `<span foreground='#hex'>` **aspas simples**, `weight='bold'`, hex exato
   One Dark, separador literal ` │ `, box-drawing pesado (┏┣┃━┗●○◆), barra = 20 chars
   (`█`×⌊pct/5⌋ + `░`). Um espaço a mais quebra o CSS state.
4. **Boundary único de XML-escape.** Provider strings só escapadas em `render_pango::span()`;
   segments `raw` bulam color-wrap **e** escape. ⟳ O escape de `'` é `&#39;` (não `&apos;`) —
   detalhe testado; replicar exato.
5. **Schema JSON versionado.** `{ schemaVersion: 1, fetchedAt, providers[] }`, sem Pango, campos
   opcionais **omitidos** (não `null`); `extra` omitido se vazio.
6. **Classes CSS de saúde.** `ok`/`low`/`warn`/`critical`/`disconnected` dos thresholds
   (`>=60`/`>=30`/`>=10`/`<10`) sobre o `remaining` **cru** (mesmo em modo `used`).

### 3.2 Cache
7. **Atômico cross-process.** temp + `rename` (atômico no mesmo FS); guard anti-traversal
   (`^[a-zA-Z0-9_-]+$`); JSON pretty 2-espaços. ⟳ **TTL per-provider** (default: Claude 300s,
   Codex 90s, Amp 90s), configurável via `settings.cache.ttl`. ⟳ Poll interval do módulo Waybar
   = `settings.waybar.interval` (default **60s**, era 120s hardcoded).
8. ⟳ **Erros nunca são cacheados.** O fetcher só escreve no sucesso; qualquer `Err` propaga sem
   `set`. (Cobre: Claude non-200, Amp `not-logged-in` sentinel, Codex sem dados.) Próximo poll
   re-tenta fresco.
9. ⟳ **Dedup in-flight** (`HashMap<key, Shared<Future>>`) só importa em `--watch`; no poll
   one-shot é inócuo. Como o loop `--watch` é sequencial (await write → sleep), ticks não se
   sobrepõem e o dedup vira redundante — implementar só se necessário.

### 3.3 Claude ⟳ (lacuna crítica do v1)
10. **Headers obrigatórios:** `User-Agent: claude-code/2.1.179` (hardcoded — UA errado cai em
    bucket de rate-limit estrito e leva 429 persistente), `anthropic-beta: oauth-2025-04-20`.
11. **Short-circuit pré-request:** se `claudeAiOauth.expiresAt` (epoch **ms**) `<= now`, retorna
    `Token expired. ...` **sem** rede e **sem** cache.
12. **Check pós-cache:** mesmo com HTTP 200, o body pode trazer `error.error_code == "token_expired"`
    → retorna `Token expired. ...`. (Esse body **é** cacheado; o check roda depois do `get_or_fetch`.)
13. **Utilization é 0-100** (não 0-1): `remaining = 100 - round(util)`. Plano: `subscriptionType`
    + `rateLimitTier` via regex `_([0-9]+)x$` → ex. `Max 5x`. `extra_usage` renderiza só se
    `enabled && limit > 0`; `used`/`limit` em **centavos** → `$X.XX/$Y.YY`. `weeklyModels` keys:
    `Opus`/`Sonnet`/`Cowork`.

### 3.4 Codex
14. **Protocolo app-server (stdio JSON-RPC):** `initialize` (id 0, `clientInfo.{name,title,version}`)
    → `initialized` + `account/read` (id 1, ⟳ `params.refreshToken=false`) +
    `account/rateLimits/read` (id 2). Respostas fora de ordem; grace de 200 ms se id 2 chega antes
    do id 1; ⟳ **timeout externo = 4 s** (≠ 5 s do HTTP); kill no fim.
15. **Fallback:** sem app-server → maior-mtime `.jsonl` em `~/.codex/sessions/YYYY/MM/DD`
    (hoje/ontem), scan **reverso** por evento `token_count`. ⟳ Sem `Bun.Glob` → `walkdir`/`read_dir`.
16. **classifyWindow tolerante:** `fiveHour` se `|min-300|<=90`; `sevenDay` se `|min-10080|<=1440`;
    senão `other`. `None`/`<=0` → `other`.

### 3.5 Amp
17. **Descoberta do binário** (ordem): `which("amp")`, `~/.local/bin/amp`, `~/.amp/bin/amp`,
    `~/.cache/.bun/bin/amp`, `~/.bun/bin/amp`.
18. spawn `amp usage` com env `NO_COLOR=1 TERM=dumb`; **drenar stderr concorrente** (senão deadlock
    de pipe); kill garantido. Auth fail (exit≠0 ou sem `Signed in as`) → `Err` antes do cache (§3.8).

### 3.6 Notificações ⟳
19. Dispara só quando: comando `waybar` (default), `format!=json`, `!watch`, **stdout não-TTY**,
    `settings.notify.enabled`. Thresholds: `LOW_USED=90`, `CRITICAL_USED=95` (sobre %usado).
20. Só em **escalação** (rank sobe); recuperar re-arma sem disparar. ⟳ **Persistir estado só após
    envio bem-sucedido** — se a notificação falha (sem daemon), no-op sem persistir (assim dispara
    na próxima vez). Estado por-provider em `notify-<provider>.json` (compact JSON, atômico).
    Dedup de alias por `(round(used), resetsAt)`.

### 3.7 CLI / dispatch ⟳
21. Default sem args = `waybar`; `isTTY && zero args` → help. Flag desconhecida = warn (não exit);
    comando desconhecido = exit 1 + "did you mean" (levenshtein ≤3).
22. **Short-circuit hidden-module:** `--provider X` + `X ∉ settings.waybar.providers` + `format!=json`
    → imprime `{"text":"","tooltip":"","class":"agent-bar-hidden"}` e exit 0, **antes** de qualquer fetch.
23. **`--watch`:** loop sequencial (await write → sleep `interval`, default 60s); EPIPE
    (`BrokenPipe`) → exit 0; outro erro de write → fatal; erro de fetch → log stderr + continua.
    Aviso a stderr se stdout é TTY.
24. **action-right disconnect** (decide login vs refresh): regex base `expired|not logged in|login
    again|please login` (i); Codex-extra `no session data|no rate limit data|auth|token` (i, só p/ codex).
25. **Ordem de providers** = `settings.waybar.providerOrder` (não `providers`). `normalize_provider_selection`:
    dedup `providers`, depois `providerOrder` = elementos atuais que sobrevivem + habilitados faltantes ao fim.
26. **`formatResetTime` usa timezone LOCAL** (não UTC). ⟳ CI roda UTC → testar com TZ pinada
    (senão passa em CI e quebra pro usuário). `time` 0.3: `UtcOffset::local_offset_at`.

### 3.8 Cores
27. ANSI gateado **só** por `NO_COLOR` (lido no start), **não** por `isTTY` — as cores fluem mesmo
    com stdout pipeado (o terminal helper consome a saída). ⟳ **Não** usar o auto-`IsTerminal` do
    owo-colors no path de render. Pango nunca é gateado (sempre emite spans).

### 3.9 SIGUSR2 ⟳
28. `pkill -SIGUSR2 waybar` (best-effort, erros ignorados) em 5 pontos: fim de setup, fim de
    uninstall, apply de configure-layout, fim de login, login-single pós-ativação. Helper compartilhado.

---

## 4. Stack de bibliotecas (final)

| Categoria | Crate | Razão |
|---|---|---|
| Runtime async | **tokio** (`rt`,`macros`,`time`,`process`,`io-util`), `current_thread` | 1 thread, init lazy; `join_all`/`timeout`/`select!`. |
| HTTP | **reqwest** `default-features=false, ["rustls-tls","json"]` | rustls puro-Rust → musl trivial. ⟳ **`Client` compartilhado** (static `OnceLock`), nunca `new()` por chamada. |
| Serialização | **serde + serde_json** | Padrão absoluto. |
| Data/hora | **time 0.3** (`serde-well-known`, `local-offset`) | `chrono` soft-deprecated; ⟳ `local-offset` p/ §3.26. |
| CLI parsing | **clap 4** (derive) | 14 comandos + "did you mean" + help. ⟳ Nested: `Export{waybar-modules,waybar-css}`, `Assets{install}`; `-t/--terminal` = flag-alias. |
| Erros | **thiserror + anyhow** | ⟳ Enums **por-provider** (`ClaudeError`/`CodexError`/`AmpError`) envoltos em `ProviderError`; `anyhow` no dispatch. |
| Async trait | **async-trait** | `dyn Provider` com `async fn` não é object-safe; boxa o future (`?Send` ok em current_thread). |
| Logging | **log + env_logger** (`Target::Stderr`) + **clap-verbosity-flag** | stderr default; leve; `try_init` nos testes. |
| XML/Pango escape | **quick-xml::escape** (ou hand-roll) | `Cow<str>`; 1 função no boundary; `'`→`&#39;`. |
| Paths XDG | **xdg 3** | Spec XDG 1:1; ⟳ resolvido no `main` e **injetado** (sem singleton). |
| Cache atômico | **tempfile** + `std::fs::rename` | Sem crate de lock. |
| Glob/dir scan | **walkdir** | ⟳ Sessões Codex `.jsonl` (substitui `Bun.Glob`). |
| Estilo terminal | **owo-colors** + **comfy-table** | ⟳ gate só `NO_COLOR` (não `IsTerminal`). |
| Tabela help/status | **comfy-table** | Alinhamento ANSI-aware. |
| TUI/prompts | **cliclack** + **ctrlc** | Port ~1:1 do `@clack/prompts`. Só em comandos interativos (não viola stdout-limpo). |
| Logo animado | **crossterm** (cursor/clear) | ⟳ `tui/logo.rs` é animação raw, **não** wrapper cliclack (6-stop RGB, 12ms/frame). |
| Notificações | ⟳ **spawn `notify-send`** (`std::process`) | Paridade exata + zero dep + ~300-600KB menor que notify-rust/zbus. |
| Testes | **insta + assert_cmd + assert_fs + predicates + wiremock + tempfile + serial_test + temp-env** | Snapshot + binário + mock HTTP + env. |
| Distribuição | **musl** + **cargo-dist** + **cargo-binstall** | Binário estático; release.yml gerado. |

⟳ **Removidos vs v1:** **mimalloc** (com `current_thread` = 1 thread, a contenção multi-thread do
malloc do musl não se aplica; só adicionaria dep de C-compiler — reavaliar só com profiling) e
**panic="abort"** (quebra `catch_unwind` no `--watch` e piora UX; ganho de tamanho marginal).

**Refinamento JSONC (mantido do v1):** **portar o scanner de string** (`findMatchingBracket`),
**não** adotar `jsonc-parser` — preserva byte-a-byte e os testes transferem.

---

## 5. Arquitetura

### 5.1 Crate layout (pacote único: `lib.rs` + `main.rs` fino)

```
Cargo.toml                # perfil release: opt-level="z", lto=true, codegen-units=1, strip=true (SEM panic=abort)
build.rs                  # SYSTEM_ASSET_DIR const; CARGO_PKG_VERSION
src/
  main.rs                 # #[tokio::main(current_thread)]: resolve Paths → init log → parse → dispatch(Paths) → ExitCode
  lib.rs
  app_identity.rs         # APP_NAME, WAYBAR_*, TERMINAL_HELPER_NAME, BACKUP_SUFFIX, APP_HIDDEN_CLASS, AMP_INSTALL_COMMAND (consts)
  config.rs               # ⟳ struct Paths/Config resolvido no main e INJETADO (sem OnceLock global); consts de API; thresholds; color/status fns; KNOWN_PROVIDER_IDS; ⟳ defaults de TTL per-provider (claude 300/codex 90/amp 90) + interval 60s
  cli.rs                  # clap derive: enum Command + GlobalArgs(flatten)
  cache.rs                # cache atômico (tempfile+rename), guard, TTL, get_or_fetch (só cacheia Ok)
  settings.rs             # struct Settings (schema §5.5), load/save/normalize+auto-repair; ⟳ KNOWN_PROVIDER_IDS inline (NÃO importar waybar_contract)
  logger.rs               # env_logger → Stderr
  theme.rs                # One Dark, box-drawing, enum ColorToken; NO_COLOR bool
  http.rs                 # ⟳ static CLIENT: OnceLock<reqwest::Client> (UA + beta headers + timeout)
  providers/
    mod.rs                # trait Provider(#[async_trait]); registry(); fetch_all(join_all+timeout 10s+1 retry)
    types.rs              # ⟳ ProviderQuota = struct FLAT { provider:String, ...core..., extra:Option<ProviderExtra> }; QuotaWindow; ModelWindows
    error.rs              # ⟳ ClaudeError/CodexError/AmpError + ProviderError(#[from]); Display = strings verbatim
    base.rs               # orquestração Codex/Amp: is_available → cache.get_or_fetch(raw) → build_quota
    claude.rs             # impl direto; expiry pré + check pós-cache; http.rs client
    codex.rs              # app-server (tokio::process + select!) + fallback walkdir
    amp.rs amp_cli.rs     # spawn + drain stderr; descoberta de binário
    extras.rs             # getClaudeExtra etc. (discrimina ProviderQuota)
  formatters/
    mod.rs segments.rs    # ⟳ Segment{ text: Cow<'static,str>, color, bold, raw }
    render_pango.rs       # ÚNICO boundary de escape: span() + escape_xml()
    render_ansi.rs        # owo-colors; gate só NO_COLOR
    waybar.rs             # {text,tooltip,class}; ⟳ cache 5s de settings VIVE AQUI (static Mutex<Option<(Settings,Instant)>>)
    terminal.rs json.rs view_model.rs builders/{claude,codex,amp,generic,shared}.rs
  waybar_integration.rs   # patch cirúrgico in-place do .jsonc (scanner portado)
  waybar_contract.rs      # geração modules.jsonc/CSS; ⟳ resolução de asset 3-vias (§5.6)
  setup.rs update.rs uninstall.rs remove.rs doctor.rs install.rs   # ⟳ install.rs: ensure_command, ensure_amp_cli
  waybar_reload.rs        # ⟳ reload_waybar() = pkill -SIGUSR2 waybar (best-effort)
  watch.rs action_right.rs notify.rs
  menu.rs tui/{mod,login,login_single,configure_layout,configure_models,list_all,logo}.rs
tests/                    # integração assert_cmd; snapshots insta; golden baselines (§9)
```

### 5.2 Modelo de execução

- `#[tokio::main(flavor="current_thread")]` — 1 thread, init lazy.
- **Fan-out:** `join_all` sobre providers, cada um em `timeout(10s, ...)` + 1 retry em timeout.
- ⟳ **I/O de arquivo síncrono (`std::fs`)** — decisão **consciente, não "idiomática"**: arquivos
  <1 KB locais, leitura sub-ms; `tokio::fs` adicionaria pool de blocking por chamada. Trade-off
  documentado; se profiling mostrar stall (home em rede / disco frio), migrar pra `tokio::fs`. O
  genuinamente bloqueante (rede, subprocesso) é async.
- ⟳ **Sem config global.** `main` resolve `Paths` (XDG) e injeta na call-chain (DI) — elimina o
  hazard de env-at-import e torna tudo testável sem `OnceLock` impoluível entre testes.
- **Hidden-module short-circuit** (§3.22) roda antes de qualquer fetch.

### 5.3 Trait de provider

```rust
#[async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn cache_key(&self) -> &'static str;
    async fn is_available(&self, paths: &Paths) -> bool;
    async fn fetch(&self, ctx: &Ctx) -> Result<ProviderQuota, ProviderError>;  // Ctx = &Client + &Paths
}
pub fn registry() -> Vec<Box<dyn Provider>> { /* explícito */ }
```

- `ClaudeProvider` impl direto (fluxo próprio, cache inline). `Codex`/`Amp` via `base.rs`.
- ⟳ **`ProviderQuota` = struct flat** com `provider: String` (campo comum, **não** tag serde) +
  `extra: Option<ProviderExtra>` (`#[serde(untagged)]`). Evita as armadilhas de internally-tagged
  + `Option`/`flatten`. **Só é serializado** (output/cache-write); o **raw** por-provider é que faz
  round-trip no cache (structs tipadas por provider).

### 5.4 Erros (per-provider, verbatim) ⟳

```rust
#[derive(thiserror::Error, Debug)]
pub enum ClaudeError {
  #[error("Not logged in. Open `agent-bar menu` and choose Provider login.")] NotLoggedIn,
  #[error("Invalid credentials file")] InvalidCredentials,
  #[error("No access token")] NoAccessToken,
  #[error("Token expired. Open `agent-bar menu` and choose Provider login.")] TokenExpired,
  #[error("Request timeout")] Timeout,
  #[error("Claude API error: {0}")] Api(u16),
  #[error("Failed to fetch Claude usage")] Generic,
}
// CodexError: NotLoggedIn, NoSessionData("No session data found"),
//   NoRateLimitData("No rate limit data found (app-server + session log)"), NoQuotaWindows("No quota windows found"), Generic
// AmpError: NotInstalled("Amp CLI not installed. Right-click to install and log in."),
//   NotLoggedIn (mesma string do Claude), ParseFailed("Failed to parse usage"), Generic("Failed to fetch Amp usage")
#[derive(thiserror::Error, Debug)]
pub enum ProviderError { #[error(transparent)] Claude(#[from] ClaudeError), Codex(#[from] CodexError), Amp(#[from] AmpError) }
```

Comandos retornam `anyhow::Result<()>`; só a camada de provider usa `ProviderError`. Testes:
`assert_eq!(ClaudeError::NotLoggedIn.to_string(), "Not logged in. ...")`.

### 5.5 Schema de Settings ⟳ (era ausente)

`$XDG_CONFIG_HOME/agent-bar/settings.json` (JSON plain, pretty 2-espaços, atômico). Round-trip
deve preservar campos. `version=2`.

- `waybar.providers: Vec<ProviderId>` (default `[claude,codex,amp]`)
- `waybar.providerOrder: Vec<ProviderId>` (dedup subset de `providers` + faltantes ao fim)
- `waybar.separators: SeparatorStyle` (enum; inválido→default)
- `waybar.displayMode: DisplayMode` (`remaining`|`used`; inválido→default)
- `waybar.signal: Option<u8>` (1..=30; fora do range → `None`, feature off)
- `waybar.showPercentage`, `tooltip` (flags)
- `models: Option<HashMap<String, Vec<String>>>` (filtro de modelos por provider)
- `windowPolicy: Option<HashMap<String, WindowPolicy>>` (`both`|`five_hour`|`seven_day`; default `both`)
- `notify.enabled: bool` (default `true`)
- ⟳ `waybar.interval: u32` segundos (default `60`; plumbado em `export_waybar_modules` → module def)
- ⟳ `cache.ttl: HashMap<ProviderId, u32>` segundos (default `{claude:300, codex:90, amp:90}`;
  campos ausentes caem no default por-provider)

**Auto-repair:** após load+normalize, comparar `to_string` (compact) do normalizado vs do raw
parseado; se diferem, gravar normalizado (pretty). Idempotente.

### 5.6 Resolução de assets + detecção compiled-vs-dev ⟳ (era ausente)

3-vias (falha se nenhuma resolve — nunca fallback silencioso pro path errado):
1. `AGENT_BAR_ASSET_DIR` (deve ser absoluto + conter `icons/`).
2. `SYSTEM_ASSET_DIR` (`/usr/share/agent-bar`) se existir — install AUR.
3. Dev: path relativo a `CARGO_MANIFEST_DIR` (ou `current_exe()/..`).

Sem `$bunfs`: detecção compiled-vs-dev via presença de `/usr/share/agent-bar/icons` (ou const de
`build.rs`). Test seam: `AGENT_BAR_ASSET_DIR=<tmp>`.

### 5.7 Install-kind detection + update ⟳ (era ausente)

`detect_install_kind`: system (`/usr/bin/agent-bar`) > dev-git (tem `.git`) > managed-git
(`current_exe` em `~/.agent-bar`) > npm (legacy). `update`:
- **system** → mensagem ("use `paru -Syu`").
- **dev-git** → recusa ("use `git pull`").
- **managed-git** → `git fetch` + `reset --hard` + `clean -fd` + re-`setup` (⟳ sem `bun install`).
- **npm** (legacy) → aviso de deprecação ("reinstale via AUR/binstall").

⟳ **`doctor` simplificado:** com npm dropado, a lógica 4-estados (orphan/mixed/legit/none) de
`~/package.json` perde o sentido. Vira **aviso de deprecação + limpeza de artefatos legacy**
(`~/package.json` órfão do agent-bar, `~/node_modules/@noctuacore/agent-bar`, lockfiles soltos) —
sem portar a classificação completa. Detecta e remove só o que claramente é resíduo do install npm antigo.

### 5.8 TUI (cliclack) ⟳

- `tui/logo.rs`: animação raw (crossterm hide-cursor → reserva N linhas → overwrite coluna-a-coluna,
  12ms/frame, gradiente 6-stop RGB; restaurar cursor via Drop + handler `ctrlc`). **Não** é cliclack.
- `configure_layout`: 4 passos (multiselect providers → re-order → separator → displayMode) →
  save → `apply_waybar_integration` → warm cache → `reload_waybar` → **sleep 8s** (espera reload) → resumo.
- `configure_models`: policy (`windowPolicy`) → multiselect de modelos (`models`) → save.
- `login`/`login_single`: spawn do CLI do provider com stdio herdado; `reload_waybar` no fim.
- `install.rs`: `ensure_command(claude/codex)`; `ensure_amp_cli` → mostra `AMP_INSTALL_COMMAND`
  (`curl -fsSL https://ampcode.com/install.sh | bash`, contrato de display) + confirm + spawn.

---

## 6. Estratégia de testes ⟳

- **Golden baseline:** **antes** da Layer 4, rodar o codebase TS e capturar saídas
  Pango/ANSI/JSON como golden files commitados → viram baseline do `insta` (senão a 1ª snapshot
  Rust pode nascer errada e travar baseline ruim).
- **Snapshot (insta):** Waybar/Pango byte-exact (sem filtro); terminal com filtro ANSI +
  sanitização de timestamp.
- **Integração (assert_cmd + assert_fs):** 14 comandos como subprocesso; FS declarativo.
- **Mock HTTP (wiremock):** server local por teste; URL injetada.
- ⟳ **Testes de provider são REESCRITOS, não "portados".** Os testes TS usam `mock.module()` (sem
  análogo Rust). Injetar traits: `Cache` (impl in-memory), `CredentialsReader`/`FileSystem`
  (Claude/Codex), spawn (Amp via trait, Codex via `tokio::io::duplex`). Estimar 3-5× mais que "portar".
- **Env (serial_test + temp-env):** com DI de `Paths` (§5.2), o hazard de OnceLock some — testes
  passam `Paths` de `tempdir`.
- **Lint:** `clippy -D warnings` + `clippy::unwrap_used`/`expect_used` deny (espelha o ban de `!`).

---

## 7. Distribuição ⟳

- **Target** `x86_64-unknown-linux-musl` (`rustup target add` + `musl-tools`).
- **Perfil release:** `opt-level="z"`, `lto=true`, `codegen-units=1`, `strip=true` (⟳ **sem**
  `panic="abort"`; **sem** mimalloc).
- **cargo-dist** gera `release.yml`. ⟳ **Tarball deve conter exatamente:** `agent-bar` (binário) +
  `scripts/agent-bar-open-terminal` (Bash, mantido verbatim) + `icons/` — espelhando o tarball atual
  (configurar `dist.toml` p/ incluir os extras).
- **AUR `agent-bar-bin`:** source → tarball musl; ⟳ **remover `options=(!strip !debug)`** (era só
  pra VFS do Bun); atualizar sha256.
- **install.sh:** baixa o binário do GitHub Releases (sem clone + `bun install`).
- ⟳ **Versão:** `env!("CARGO_PKG_VERSION")` (usada inclusive no `clientInfo.version` do Codex
  initialize); `check:pkgver` vira Cargo-based (`cargo metadata` vs PKGBUILD `pkgver`); `publish.yml`
  perde o `jq .version package.json` + o build do binário Bun.
- ⟳ **Removidos do repo no fim:** `package.json`, `bun.lock`, `dist/`, `scripts/agent-bar` (shim
  Bash do Bun). `agent-bar-open-terminal` **fica** (Bash).

---

## 8. Setup de Claude Code (project-level, versionado, sem `-g`)

- **Skills:** `apollographql/skills@rust-best-practices`, `wshobson/agents@rust-async-patterns`,
  `affaan-m/everything-claude-code@rust-testing`.
- **LSP:** `rust-analyzer` (refs + type errors pós-edit).
- **Hook PostToolUse em `*.rs`:** `cargo check` (via `update-config`) — pega erro no loop do agente.
- **CLAUDE.md:** reescrever Hard Rules / Verification Matrix pro mundo Rust; os contratos da §3
  viram as regras invioláveis.

---

## 9. Plano de migração (ordem corrigida) ⟳

Cada camada: reescrever + testes (golden/insta/assert_cmd) verdes antes de avançar.

0. **Golden snapshots:** capturar saídas Pango/ANSI/JSON do TS atual → commitar como baseline.
1. **Scaffold:** `cargo` init, `Cargo.toml` (deps + perfil), `app_identity`, `config`+`Paths`(DI),
   `theme`, `logger`, `build.rs` (versão + SYSTEM_ASSET_DIR), CI. Binário imprime versão.
2. **cache** (puro): atômico + guard + TTL + `get_or_fetch` (só Ok) + testes.
3. **settings** (puro): struct (§5.5) + normalize/auto-repair + load/save atômico; ⟳ `KNOWN_PROVIDER_IDS`
   **inline** (quebra o ciclo settings→waybar-contract→registry).
4a. **formatters puros:** segments → render_pango (boundary) → render_ansi → builders → json.
    Snapshots byte-exact vs golden. **Maior valor de teste.**
4b. **formatters c/ settings:** waybar.rs (+ cache 5s aqui) + view_model.rs. (Depende da Layer 3 —
    ⟳ separado de 4a por causa do ciclo.)
5. **providers** (async entra): http.rs (Client compartilhado) → types/error/trait/registry/base →
   Claude (expiry pré+pós, headers) → Codex (app-server select! + grace + 4s + fallback walkdir) →
   Amp (spawn + drain). Testes reescritos com traits. **notify.rs** entra no fim (depende de `extras`).
6. **CLI + dispatch:** clap (nested Export/Assets, alias -t), help, suggestions, hidden-module
   short-circuit, `--watch`, `action_right` (2 regexes), `reload_waybar`. Integração assert_cmd.
7a. **waybar_integration** (scanner portado) + testes vs snapshots.
7b. **waybar_contract** (export + asset resolution 3-vias) — depende de 7a.
7c. **setup + uninstall + remove** (interativos).
7d. **update + doctor** (install-kind 4-vias; ⟳ doctor = deprecação + limpeza de legacy npm, não 4-estados).
8. **Distribuição:** musl + cargo-dist + tarball + PKGBUILD; remover npm; reescrever CLAUDE.md/docs.

**Cutover ⟳:** o binário é monolítico (não há ship parcial TS+Rust). Antes do cutover:
**shadow-mode** — rodar o binário Rust lado-a-lado com o TS em `agent-bar waybar` e **diffar
JSON/Pango** por provider; **rollback** — manter o binário TS sob outro nome por N dias.

---

## 10. Riscos e mitigações ⟳

| Risco | Mitigação |
|---|---|
| **Codex app-server** (estado multi-turno + 2 timers que correm) | `select!` único sobre (account_rx, ratelimits_rx, grace, hard_timeout); grace **armado só após** ratelimits; `oneshot` por resposta. Testar com `tokio::io::duplex`. |
| **Amp deadlock de pipe** | `Stdio::null()` no stderr ou task de drain. |
| **Snapshot Pango drift** | Golden baseline do TS (passo 0); aspas simples/`&#39;`/hex/separador como consts. |
| **`formatResetTime` TZ** | TZ **local** via `time` `local-offset`; testar com TZ pinada (CI roda UTC → trap). |
| **JSONC scanner** sutil | Port 1:1 + testes de `waybar-integration` transferidos primeiro (TDD). |
| **Testes de provider** sem análogo a `mock.module` | Reescrever com trait injection; orçar 3-5×. |
| **Ciclos de dependência** (settings→contract; formatters→settings) | Inline de const; split 4a/4b (§9). |
| **Cutover all-or-nothing** | Shadow-mode + rollback (§9). |
| **`dyn Provider` + async** | `async-trait` (`?Send` ok em current_thread). |

---

## 11. Critério de pronto

- `cargo test` + `clippy -D warnings` verdes; todos os contratos da §3 cobertos.
- Shadow-mode: diff JSON/Pango Rust vs TS = idêntico por provider, no Waybar real do usuário (com aprovação).
- Release cortável via cargo-dist; PKGBUILD aponta pro binário Rust; npm removido.
