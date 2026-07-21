# v9 Docs + Preparação de Release Implementation Plan
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Atualizar toda a documentação pública (README, CONTRIBUTING, docs/*,
CLAUDE.md, CHANGELOG) para refletir o agent-bar 9.0.0 Omarchy-first, sem
cortar o release.

**Architecture:** Esta é a PR 5 (última) da sequência do spec
`docs/superpowers/specs/2026-07-21-omarchy-first-popup-redesign-design.md`.
Assume que as PRs 1-4 (limpeza de legado, contrato `windowKind`, Widget.qml
novo, gates de plataforma + `src/waybar/`) já landaram em `master` — os
símbolos que este plano cita como existentes (`platform::detect()`,
`QuotaWindow.window_kind`, `src/waybar/contract.rs`, `menuAnimations` em
`config show`, settings v3) são produto delas, não deste plano. Este plano
só edita Markdown; nenhum arquivo `.rs` muda.

**Tech Stack:** Markdown puro. Verificação via `git diff --check` (erros de
whitespace) e `grep -n`/`grep -c` (consistência de conteúdo) — não há
`cargo test` aplicável a texto.

## Global Constraints
- Prosa dos docs e commits em português; identificadores citados no texto em inglês.
- Commits: Conventional Commits em PT, subject ≤ 50 chars.
- ZERO atribuição de AI em qualquer texto (commit, PR, comentário).
- `CHANGELOG.md` só é editado ao preparar/cortar release (é exatamente o que a Task 9 faz).
- Error strings de provider são contrato — nenhuma task deste plano toca `src/`, então isso não se aplica, mas nenhum doc pode reescrever uma string de erro citada como se fosse diferente da real.
- Não cortar release (sem bump de `Cargo.toml`, sem tag, sem GitHub Release) — isso é runbook manual do dono (`docs/releasing.md`).
- Cada task só mexe no(s) arquivo(s) listado(s) em **Files** — não tocar em mudanças não relacionadas (`git status` limpo antes de cada task).

---

### Task 1: README.md — tagline e requisitos Omarchy-first

**Files:**
- Modify: `README.md:7` (tagline) e `README.md:25-32` (bloco `## Requisitos`)

**Interfaces:**
- Consumes: fatos já existentes no arquivo — instalador `install.sh`, AUR
  `agent-bar-bin` (comentado como "sai em breve"), `cargo binstall --git`,
  `agent-bar setup` (linhas 34-70, **não tocadas** por esta task).
- Produces: primeira impressão do repo alinhada a "Omarchy-first, Waybar
  tier legado" — nenhuma task futura depende do texto exato, mas a Task 9
  (CHANGELOG) referencia esta mudança na entrada 9.0.0.

**Steps:**

- [ ] **Step 1: Confirmar que o texto antigo ainda está lá (checagem que falha depois do fix)**

  Rodar:
  ```bash
  grep -n "Monitor de quota de Claude Code, OpenAI Codex, Amp e Grok Build na Waybar" README.md
  grep -n "Linux x86_64 com Waybar. Uso no Hyprland" README.md
  ```
  Esperado: as duas linhas aparecem (7 e 27) — confirma o baseline antes do
  edit.

- [ ] **Step 2: Editar a tagline (linha 7)**

  Trocar:
  ```
  Monitor de quota de Claude Code, OpenAI Codex, Amp e Grok Build na Waybar. Quota de agente é aquele recurso que você só descobre que acabou quando acabou; aqui ela fica visível na barra o tempo todo. Um binário em Rust, sem daemon: a Waybar chama, ele imprime JSON e vai embora.
  ```
  Por:
  ```
  Monitor de quota de Claude Code, OpenAI Codex, Amp e Grok Build — nativo no Omarchy 4 (omarchy-shell), com Waybar suportada como tier legado. Quota de agente é aquele recurso que você só descobre que acabou quando acabou; aqui ela fica visível na barra o tempo todo. Um binário em Rust, sem daemon: o shell chama, ele imprime JSON e vai embora.
  ```

- [ ] **Step 3: Editar o bloco `## Requisitos` (linhas 25-32)**

  Trocar:
  ```
  ## Requisitos

  - Linux x86_64 com Waybar. Uso no Hyprland, mas não tem nada específico dele aqui.
  - **Omarchy 4 (omarchy-shell)**: bar-widget plugin nativo com chips + popup — `agent-bar setup` detecta e instala. Waybar segue suportada.
  - `curl`, `tar` e `sha256sum` pro instalador.
  - Um terminal que o helper reconheça: alacritty, kitty, foot, ghostty, wezterm, ou `xdg-terminal-exec`.
  - As CLIs que você quer monitorar, instaladas. O login dá pra fazer pela própria TUI.
  - `libnotify`, se quiser notificação de quota baixa. Opcional.
  ```
  Por:
  ```
  ## Requisitos

  - Linux x86_64. **Omarchy 4 (omarchy-shell)** é o alvo principal: `agent-bar setup` detecta o omarchy-shell e instala o bar-widget plugin nativo (chips + popup) automaticamente.
  - **Waybar** — suportada como **tier legado**: funciona, recebe fix, não recebe feature nova. `agent-bar setup` integra com ela também, quando presente.
  - `curl`, `tar` e `sha256sum` pro instalador.
  - Um terminal que o helper reconheça: alacritty, kitty, foot, ghostty, wezterm, ou `xdg-terminal-exec`.
  - As CLIs que você quer monitorar, instaladas. O login dá pra fazer pela própria TUI.
  - `libnotify`, se quiser notificação de quota baixa. Opcional.
  ```

- [ ] **Step 4: Rodar e ver passar**

  ```bash
  grep -c "Omarchy 4 (omarchy-shell)" README.md
  grep -n "tier legado" README.md
  grep -n "Monitor de quota de Claude Code, OpenAI Codex, Amp e Grok Build na Waybar" README.md
  git diff --check
  ```
  Esperado: a primeira conta ≥2 (tagline + requisitos), a segunda encontra a
  linha nova do bloco Requisitos, a terceira **não retorna nada** (tagline
  antiga sumiu), `git diff --check` sem saída (sem erro de whitespace).

- [ ] **Step 5: Commit**

  ```bash
  git add README.md
  git commit -m "docs: tagline e requisitos Omarchy-first no README"
  ```

---

### Task 2: CONTRIBUTING.md — MSRV 1.88

**Files:**
- Modify: `CONTRIBUTING.md:6-12` (tabela de Prerequisites)

**Interfaces:**
- Consumes: `Cargo.toml` `rust-version = "1.88"` (já correto no manifest;
  este doc está desatualizado em relação a ele) e o comentário do próprio
  `Cargo.toml` — `# 1.88: piso real, não 1.80 — tachyonfx (via anpa) exige 1.82.`
- Produces: nenhuma task posterior depende disso; é uma correção de fato solta.

**Steps:**

- [ ] **Step 1: Confirmar o texto desatualizado**

  ```bash
  grep -n "stable (1.75+)" CONTRIBUTING.md
  ```
  Esperado: 1 ocorrência (linha 9) — confirma o baseline errado.

- [ ] **Step 2: Editar a tabela de Prerequisites**

  Trocar:
  ```
  | Tool | Minimum |
  |------|---------|
  | [Rust](https://rustup.rs) | stable (1.75+) |
  | Git | recent |

  Rust + Cargo is the only supported toolchain.
  ```
  Por:
  ```
  | Tool | Minimum |
  |------|---------|
  | [Rust](https://rustup.rs) | 1.88 (MSRV — `tachyonfx` via `anpa` requires it) |
  | Git | recent |

  Rust + Cargo is the only supported toolchain.
  ```

- [ ] **Step 3: Rodar e ver passar**

  ```bash
  grep -n "stable (1.75+)" CONTRIBUTING.md; echo "exit=$?"
  grep -n "1.88 (MSRV" CONTRIBUTING.md
  git diff --check
  ```
  Esperado: o primeiro grep não acha nada (`exit=1`), o segundo acha a linha
  nova, `git diff --check` sem saída.

- [ ] **Step 4: Commit**

  ```bash
  git add CONTRIBUTING.md
  git commit -m "docs: corrige MSRV pra 1.88 no CONTRIBUTING"
  ```

---

### Task 3: docs/waybar-contract.md — Grok + banner de tier legado

**Files:**
- Modify: `docs/waybar-contract.md:1-17` (título + seção `## Providers`)

**Interfaces:**
- Consumes: registro real de providers Waybar — `docs/waybar-contract.md`
  hoje lista só `claude`/`codex`/`amp`; `grok` já é provider registrado
  (`src/providers/grok.rs`, cache key `grok-usage`) e já aparece no módulo
  Waybar (ícone shipado desde 8.2.1), só faltava no contrato documentado.
- Produces: nenhuma task futura depende do texto exato; é a correção de um
  gap de documentação pré-existente + o banner de tier legado que a Task 9
  (CHANGELOG) e o README (Task 1) já anunciam.

**Steps:**

- [ ] **Step 1: Confirmar que `grok` está ausente do contrato documentado**

  ```bash
  grep -n '`grok`' docs/waybar-contract.md; echo "exit=$?"
  ```
  Esperado: nenhuma ocorrência (`exit=1`) — confirma o gap antes do fix.

- [ ] **Step 2: Inserir o banner de tier legado logo após o título**

  Trocar:
  ```
  # Waybar Contract

  This is the generated contract used by setup and by the export commands.

  ## Providers

  Built-in Waybar providers:

  - `claude`
  - `codex`
  - `amp`

  Generated module IDs:

  - `custom/agent-bar-claude`
  - `custom/agent-bar-codex`
  - `custom/agent-bar-amp`
  ```
  Por:
  ```
  # Waybar Contract

  > **Waybar é tier legado** a partir da 9.0.0: funciona, recebe fix, não
  > recebe feature nova. O alvo atual é o Omarchy 4 (omarchy-shell) — ver
  > [`omarchy-shell.md`](omarchy-shell.md). Este documento descreve o
  > contrato Waybar como está, para quem ainda depende dele.

  This is the generated contract used by setup and by the export commands.

  ## Providers

  Built-in Waybar providers:

  - `claude`
  - `codex`
  - `amp`
  - `grok`

  Generated module IDs:

  - `custom/agent-bar-claude`
  - `custom/agent-bar-codex`
  - `custom/agent-bar-amp`
  - `custom/agent-bar-grok`
  ```

- [ ] **Step 3: Rodar e ver passar**

  ```bash
  grep -n '`grok`' docs/waybar-contract.md
  grep -n "custom/agent-bar-grok" docs/waybar-contract.md
  grep -n "tier legado" docs/waybar-contract.md
  git diff --check
  ```
  Esperado: as três buscas encontram a linha nova cada uma; `git diff
  --check` sem saída.

- [ ] **Step 4: Commit**

  ```bash
  git add docs/waybar-contract.md
  git commit -m "docs: Grok no contrato Waybar + banner de tier legado"
  ```

---

### Task 4: docs/json-output.md — remover seção Future

**Files:**
- Modify: `docs/json-output.md:123-129` (fim do arquivo, seção `## Future`)

**Interfaces:**
- Consumes: **produto da PR 2** (`windowKind` em `QuotaWindow` +
  `docs/json-output.md` já documentando `extra` por provider —
  `AmpQuotaExtra.meta`, `GrokQuotaExtra`, `ClaudeQuotaExtra.extraUsage`,
  `CodexQuotaExtra.modelsDetailed` — conforme a Seção A do spec). Esta task
  **não escreve** esse conteúdo — só confere que já está lá e remove a
  seção `## Future` que ficou obsoleta (o "plugin Quickshell nativo" que ela
  anunciava como futuro já existe desde a 8.4.0).
- Produces: nada consome a ausência da seção; é limpeza terminal do arquivo.

**Steps:**

- [ ] **Step 1: Confirmar a pré-condição da PR 2 (bloqueante se falhar)**

  ```bash
  grep -c "windowKind" docs/json-output.md
  grep -c "AmpQuotaExtra\|GrokQuotaExtra\|ClaudeQuotaExtra\|CodexQuotaExtra" docs/json-output.md
  ```
  Esperado: ambas as contagens ≥1. **Se qualquer uma vier 0, a PR 2 ainda
  não landou o conteúdo que esta task assume — pare e reporte ao invés de
  prosseguir** (não escrever esse conteúdo aqui; é escopo da PR 2, não
  desta).

- [ ] **Step 2: Confirmar que a seção Future ainda existe**

  ```bash
  grep -n "^## Future" docs/json-output.md
  ```
  Esperado: 1 ocorrência (linha 125) — confirma o baseline antes do fix.

- [ ] **Step 3: Remover a seção Future**

  Trocar (linhas 123-129, fim do arquivo):
  ```
  `Process.command` is an argv array (no shell) — keep each argument separate.

  ## Future

  A native Omarchy Quickshell bar-widget plugin
  (`~/.config/omarchy/plugins/agent-bar/`) is a future step once Omarchy ships its
  Quickshell release (v4).
  ```
  Por:
  ```
  `Process.command` is an argv array (no shell) — keep each argument separate.
  ```

- [ ] **Step 4: Rodar e ver passar**

  ```bash
  grep -n "^## Future" docs/json-output.md; echo "exit=$?"
  tail -3 docs/json-output.md
  git diff --check
  ```
  Esperado: o grep não acha nada (`exit=1`); `tail` mostra a linha do
  `Process.command` como última linha não-vazia do arquivo; `git diff
  --check` sem saída.

- [ ] **Step 5: Commit**

  ```bash
  git add docs/json-output.md
  git commit -m "docs: remove seção Future obsoleta do json-output"
  ```

---

### Task 5: docs/troubleshooting.md — seção Grok

**Files:**
- Modify: `docs/troubleshooting.md:74-87` (bloco `### Amp`, entre ela e `##
  Reset Managed Waybar Entries`)

**Interfaces:**
- Consumes: `src/providers/grok.rs` — `ctx.paths.grok_auth` (
  `~/.grok/auth.json`), sessões em `{grok_home}/sessions/**/signals.json`
  (função `collect_signals`), CHANGELOG 8.2.0 ("Login via `grok login` na
  TUI"), e a nota de `parse_auth_entries`: token expirado não desloga
  (renovado pelo Grok CLI via refresh token; o provider é zero-rede).
- Produces: nada consome; é gap de documentação pré-existente (a seção
  "Provider Auth" cobre Claude/Codex/Amp mas nunca teve Grok, apesar do
  provider existir desde a 8.2.0).

**Steps:**

- [ ] **Step 1: Confirmar o gap**

  ```bash
  grep -n "### Grok" docs/troubleshooting.md; echo "exit=$?"
  ```
  Esperado: nenhuma ocorrência (`exit=1`).

- [ ] **Step 2: Inserir a seção Grok após `### Amp`**

  Trocar:
  ````
  ### Amp

  Install Amp with the official installer:

  ```bash
  curl -fsSL https://ampcode.com/install.sh | bash
  ```

  Then run:

  ```bash
  amp login
  ```

  ## Reset Managed Waybar Entries
  ````
  Por:
  ````
  ### Amp

  Install Amp with the official installer:

  ```bash
  curl -fsSL https://ampcode.com/install.sh | bash
  ```

  Then run:

  ```bash
  amp login
  ```

  ### Grok

  Grok Build CLI uses OAuth in `~/.grok/auth.json`; the provider itself
  makes zero network calls — it reads session `signals.json` files under
  `~/.grok/sessions/**` for context-window data. Log in with:

  ```bash
  grok login
  ```

  An access token past its `expires_at` does **not** log you out: the Grok
  CLI renews it via refresh token, and agent-bar only checks for a
  non-empty `key` in `auth.json`.

  ## Reset Managed Waybar Entries
  ````

- [ ] **Step 3: Rodar e ver passar**

  ```bash
  grep -n "### Grok" docs/troubleshooting.md
  grep -n "grok login" docs/troubleshooting.md
  grep -n "~/.grok/auth.json" docs/troubleshooting.md
  git diff --check
  ```
  Esperado: as três buscas encontram as linhas novas; `git diff --check`
  sem saída.

- [ ] **Step 4: Commit**

  ```bash
  git add docs/troubleshooting.md
  git commit -m "docs: seção Grok no troubleshooting"
  ```

---

### Task 6: docs/runtime.md — seção Grok (tabela de credenciais)

**Files:**
- Modify: `docs/runtime.md:91-100` (bloco `## Provider Credentials`)

**Interfaces:**
- Consumes: mesma fonte da Task 5 (`src/providers/grok.rs`:
  `ctx.paths.grok_auth`, `sessions/**/signals.json`).
- Produces: nada consome; a tabela hoje lista Claude/Codex/Amp mas omite
  Grok apesar dos defaults (linhas 49-50 do mesmo arquivo) já incluírem
  `grok`.

**Steps:**

- [ ] **Step 1: Confirmar o gap**

  ```bash
  grep -n "| Grok |" docs/runtime.md; echo "exit=$?"
  ```
  Esperado: nenhuma ocorrência (`exit=1`).

- [ ] **Step 2: Adicionar a linha Grok à tabela**

  Trocar:
  ```
  | Provider | Source |
  | --- | --- |
  | Claude | `~/.claude/.credentials.json` |
  | Codex | `~/.codex/auth.json`, recent `~/.codex/sessions/**` rate-limit events, or `codex app-server` |
  | Amp | official `amp` CLI |
  ```
  Por:
  ```
  | Provider | Source |
  | --- | --- |
  | Claude | `~/.claude/.credentials.json` |
  | Codex | `~/.codex/auth.json`, recent `~/.codex/sessions/**` rate-limit events, or `codex app-server` |
  | Amp | official `amp` CLI |
  | Grok | `~/.grok/auth.json`; session `signals.json` under `~/.grok/sessions/**` (read-only, no network) |
  ```

- [ ] **Step 3: Confirmar o baseline da versão do schema**

  ```bash
  grep -n "Settings schema version" docs/runtime.md
  ```
  Esperado: 1 ocorrência (linha 45) — `Settings schema version: \`2\`.` —
  confirma o baseline antes do fix.

- [ ] **Step 4: Atualizar a versão do settings schema (migração v2 → v3 do plano 01)**

  Trocar:
  ```
  Settings schema version: `2`.
  ```
  Por:
  ```
  Settings schema version: `3`.
  ```

- [ ] **Step 5: Rodar e ver passar**

  ```bash
  grep -n "| Grok |" docs/runtime.md
  grep -n "Settings schema version" docs/runtime.md
  git diff --check
  ```
  Esperado: a primeira encontra a linha Grok nova; a segunda mostra
  `Settings schema version: \`3\`.`; `git diff --check` sem saída.

- [ ] **Step 6: Commit**

  ```bash
  git add docs/runtime.md
  git commit -m "docs: linha Grok e schema v3 na tabela do runtime"
  ```

---

### Task 7: docs/architecture.md — reescrito Omarchy-first

**Files:**
- Modify: `docs/architecture.md` (arquivo inteiro, 197 linhas hoje —
  substituição completa do corpo, mantendo o mesmo título e a maioria dos
  fatos técnicos, reordenados e ampliados)

**Interfaces:**
- Consumes (produto das PRs 1-4, conforme o CONTRATO DE INTERFACES
  GLOBAIS): `src/platform.rs::{Platform, detect}`; `QuotaWindow.window_kind`
  em `src/providers/types.rs`; `classify_window`/`is_duplicate_window` em
  `src/formatters/shared.rs` (o `classify_window` interno já existe hoje —
  ver `src/formatters/shared.rs:47`; `is_duplicate_window` é novo, produto
  da PR 2); `menuAnimations` em `config show` (`src/config_cmd.rs`);
  `src/waybar/contract.rs` / `src/waybar/integration.rs` com re-exports
  (produto da PR 4). Fatos já reais hoje e preservados da versão atual:
  fluxo `main.rs` → `cli.rs` → `providers/` → `cache.rs` →
  `formatters/` → stdout; `BaseProvider` vs `ClaudeProvider` direto;
  boundary de escape só em `render_pango.rs`.
- Produces: mapa de módulos que `docs/new-provider.md` e onboarding humano
  usam como referência; nenhuma outra task deste plano depende do texto
  exato.

**Steps:**

- [ ] **Step 1: Confirmar o baseline pré-Omarchy-first**

  ```bash
  grep -n "^## Data Flow" docs/architecture.md
  grep -c "src/waybar/" docs/architecture.md; echo "exit acima deve ser 0"
  grep -c "windowKind\|window_kind" docs/architecture.md; echo "exit acima deve ser 0"
  ```
  Esperado: a primeira acha a seção atual (linha 6); as duas contagens são
  `0` — confirma que o arquivo ainda não reflete `src/waybar/` nem
  `windowKind` (produto das PRs 2/4 que este doc só passa a citar agora).

- [ ] **Step 2: Substituir o arquivo inteiro**

  Conteúdo completo novo de `docs/architecture.md`:
  ````markdown
  # Architecture

  How agent-bar turns a fetch into something on screen. Omarchy 4
  (omarchy-shell) is the primary target — `Widget.qml` renders the popup
  straight from `--format json`. Waybar remains supported as a **legacy
  tier** (works, gets fixes, no new features) via the classic Pango module
  contract. `src/` is the source of truth and wins on any disagreement.

  ## Data Flow

  Two consumers read the same provider/formatter pipeline; they diverge
  only at the very end.

  ```text
  Omarchy Widget.qml (native plugin, primary)      Waybar module (legacy tier)
     │  polls                                          │  polls (interval from settings, default 60s)
     ▼                                                  ▼
  agent-bar --format json                       agent-bar --provider <id>
     │                                                  │
     └──────────────────┬───────────────────────────────┘
                         ▼
          parse_args → CliOptions                  src/cli.rs
                         ▼
          get_quota_for(id) / get_all_quotas()      src/providers/   fan-out + timeout/retry
                         │  parallel · 10s timeout · 1 retry on timeout
                         ▼
          Provider::get_quota()
             ├─ ClaudeProvider    (implements Provider direct) src/providers/claude.rs
             └─ Codex/Amp/Grok   (extend BaseProvider)        src/providers/base.rs
                         │  is_available() gate → credentials check
                         ▼
          Quota cache  (file · 5 min TTL · cross-process)    src/cache.rs
                         │  hit → cached raw   miss → fetch_raw() → provider API/CLI → cache.set()
                         ▼
          ProviderQuota  (normalized, windowKind classified) src/providers/types.rs
                         │
             ┌───────────┴────────────┐
             ▼                        ▼
      to_json_output            formatters::waybar
      (schemaVersion, no Pango)  format_for_waybar / format_provider_for_waybar
      src/formatters/json.rs     builders → segments → render_pango() (XML escape)
             │                        │
             ▼                        ▼
      stdout NDJSON/JSON envelope    stdout Waybar JSON {text,tooltip,class}
             │
             ▼
      Widget.qml applyPayload() — collapses duplicate windows
      (same windowKind/resetsAt/remaining), renders countdown from
      resetsAt/fetchedAt, hero %, per-provider panel, extra line.
  ```

  `stdout` is reserved for the machine-readable payload the shell/Waybar
  parses; all logging goes to `stderr` via `logger`. Breaking that contract
  breaks both consumers.

  `agent-bar menu` (the TUI) and `agent-bar status` read the exact same
  `ProviderQuota` through their own formatter (`terminal.rs`) — a third,
  interactive consumer of the same pipeline, not shown above for brevity.

  ## Entry And Dispatch — `src/main.rs`

  `parse_args` (`src/cli.rs`) turns argv into `CliOptions`. `main()`
  dispatches:

  - **Subcommands** (`menu`, `setup`, `doctor`, `update`, `config
    show|apply`, `action-right`, …) are dispatched by pattern match and
    handled, then the process exits. `config show|apply` is implemented in
    `src/config_cmd.rs` (editable settings subset only, plus the read-only
    `menuAnimations` flag on `show`; no Waybar reload). `action-right`
    remains the Waybar right-click path; Omarchy uses the native popup
    settings mode instead (see [omarchy-shell.md](omarchy-shell.md)).
  - **`--watch`** hands off to `start_watch` (`src/watch.rs`) and streams
    NDJSON.
  - Otherwise it resolves quotas and prints a single payload. The default
    command is `waybar`; `status` (`-t`/`--terminal` alias) prints the ANSI
    view; `--format json` prints the versioned envelope Widget.qml
    consumes.

  `setup`, `update`, and TUI Config Save all gate their Waybar-directory
  writes behind `platform::detect()` (`src/platform.rs`) — a single
  `Platform { omarchy: bool, waybar: bool }` that composes
  `omarchy_integration::detect_omarchy_shell()` and `setup::waybar_present()`.
  On an Omarchy-only desktop, none of the three paths create
  `~/.config/waybar/` from scratch.

  Two Waybar render shapes share the formatter (legacy tier only):

  - **Single-provider module** — `agent-bar --provider <id>` →
    `formatProviderForWaybar`. This is what generated Waybar modules call
    (one module per provider). A provider disabled in settings never
    reaches the formatter — `main.rs` / gate `is_hidden_module`
    short-circuits first and prints the hidden-module payload (`class:
    agent-bar-hidden`) so Waybar collapses it.
  - **Aggregate** — `agent-bar` with no provider → `outputWaybar` /
    `formatForWaybar`, joining every enabled provider into one module.

  After Waybar output, low/critical desktop notifications fire best-effort
  (`src/notify.rs`) — only when `notify.enabled` is set (default on) and
  stdout is piped (i.e. real Waybar polling), never on an interactive
  terminal run, and never in json/terminal/watch modes.

  ## Provider Layer — `src/providers/`

  Providers are registered in `src/providers/mod.rs`. `get_all_quotas` runs
  every registered provider in parallel behind `fetch_with_retry` (10s
  timeout, one retry on timeout); a failing provider degrades to an
  `available: false` quota with an error string instead of taking down the
  bar. `get_quota_for` is the single-provider variant.

  Every provider returns a normalized **`ProviderQuota`**
  (`src/providers/types.rs`): `primary`/`secondary` quota windows (each
  `{ remaining, resetsAt, windowKind, … }`), optional `extra` (per-provider,
  pre-render data like Claude `weeklyModels`, Codex `modelsDetailed`, Amp
  credits, or Grok session/turn counts), `plan`/`account`, or an `error`.
  Everything downstream speaks this shape — formatters never see
  provider-specific API responses.

  Each provider sets `windowKind` at the source instead of downstream code
  guessing from the number: Claude sets `fiveHour`/`sevenDay` directly; Amp
  sets `daily`; Grok sets `context`; Codex classifies via
  `classify_window` (below) with no fallback — a window outside tolerance
  is `other`, not force-mapped onto `fiveHour`/`sevenDay`.

  ### `BaseProvider` vs `ClaudeProvider`

  `BaseProvider` (`src/providers/base.rs`) owns the `get_quota()`
  orchestration so concrete providers implement only what differs:

  ```text
  get_quota():
    base = build_base()
    if !is_available()       → return ProviderQuota { error: unavailable_error(), .. }
    raw = cache.get_or_fetch(cache_key, fetch_raw, 5min)  // cached
    return build_quota(raw, base)                          // pure transform
    (any error)              → return ProviderQuota { error: to_user_facing_error(e), .. }
  ```

  `CodexProvider`, `AmpProvider`, and `GrokProvider` extend it — they
  supply `is_available`, `fetch_raw`, `build_quota`, and
  `unavailable_error`, and inherit the availability gate, cache wrapper,
  and error handling.

  `ClaudeProvider` **implements `Provider` directly** and does not extend
  `BaseProvider`. Its flow doesn't fit the template: it reads
  `~/.claude/.credentials.json` for the OAuth token, distinguishes "no
  file" / "invalid file" / "no token" / "token expired" as separate
  user-facing errors, calls `cache.get_or_fetch` inline, and parses several
  quota windows (`five_hour`, `seven_day`, per-model weeklies, extra
  usage). Forcing it into `BaseProvider` would mean fighting the
  abstraction, so it manages its own cache call. Do not "normalize" it back
  into the template (see repo `CLAUDE.md`).

  From the same credentials it also reads `expiresAt` — to short-circuit a
  locally-expired access token (the API would reject it anyway, and
  agent-bar must never refresh it: the single-use refresh token races
  Claude Code) — and `rateLimitTier`, to surface the Max multiplier in the
  plan label (e.g. `Max 5x`).

  ## Window Classification & Display Dedup — `src/formatters/shared.rs`

  `classify_window(window_minutes) -> WindowKind` is the single place
  tolerance math lives (`fiveHour` = 300±90min, `sevenDay` = 10080±1440min,
  else `other`) — providers call it once at the source; QML and the TUI
  never re-derive a label from magic numbers. `format_eta`/
  `format_reset_time` render the countdown (`Xh Ym`/`Nd Nh`) and the
  localized reset clock (`Clock.local_offset`) from the same `resetsAt`
  string — the TUI's `fmt_reset` (`src/tui/render/detail/format.rs`) calls
  these instead of slicing the raw ISO string.

  `is_duplicate_window(a, b) -> bool` collapses two windows that share
  `(windowKind, resetsAt, remaining-rounded)` — the display-level fix for
  the Codex Plus duplicate-Weekly bug (the JSON envelope still emits
  `primary`/`secondary`/`models` untouched; only consumers that render
  multiple windows side by side dedup). The TUI consumes it directly; QML
  ports the same predicate in JS since it has no Rust runtime.

  ## The Quota Cache

  `src/cache.rs` is a file-based cache (`$XDG_CACHE_HOME/agent-bar/<key>.json`,
  default `~/.cache`) with a **5-minute TTL** (`CONFIG.cache.ttl_ms`) and
  **cross-process** lifetime — it survives between polls, which is what
  matters here: both Widget.qml and Waybar poll at their own configured
  interval but each poll is a *separate process*, so an in-memory cache
  would never hit. The file-based cache means most polls within a
  5-minute window read the cached response from disk and one triggers a
  fresh fetch — the provider API is hit at most once per 5-minute window
  regardless of the configured poll interval. `get_or_fetch` reads the
  cache, and on a miss runs the fetcher once, writing atomically (temp
  file + `rename`) because concurrent provider processes can write the
  same key. An in-flight dedup map deduplicates concurrent fetches
  *within* one process; failed fetches are never cached (a non-200 errors
  before `set`); cache keys are validated against path traversal.

  There is no separate settings cache — `settings::load` reads
  `settings.json` directly on every call; no in-process memoization layer
  exists.

  ## Formatter Layer — `src/formatters/`

  Quotas are rendered three ways from the same `ProviderQuota`:

  - **JSON** (`json.rs`) → versioned, Pango-free envelope
    (`{ schemaVersion, fetchedAt, providers[] }`), the contract Widget.qml
    and any other native bar consumes. See [`json-output.md`](json-output.md).
  - **Waybar** (`waybar.rs`) → `{ text, tooltip, class }` JSON, legacy
    tier. `text` is the bar percentage; `tooltip` is multi-line Pango
    built per provider; `class` is a provider-scoped compound carrying a
    health state for CSS — `agent-bar-<provider> <status>` for a single
    module, or `agent-bar` plus `<provider>-<status>` tokens in aggregate
    (see [Waybar contract](waybar-contract.md)). States:
    `ok`/`low`/`warn`/`critical`/`disconnected`.
  - **Terminal** (`terminal.rs`) → ANSI view for `status`. `action-right`
    no longer uses this path — it resolves focus and opens the TUI instead
    (see `src/action_right.rs` below).

  Builders (`formatters/builders/`) describe output as `Vec<Line>` of
  typed `Segment`s (text + color token). **`render_pango.rs` is the single
  XML-escape boundary** — `span()` / `render_pango()` are the only places
  provider data gets Pango markup, and they escape every non-`raw`
  segment. Builders never escape; a `raw` segment opts out of both
  color-wrap *and* escape and must already be safe. Routing untrusted
  provider strings around this boundary is a tooltip injection bug, so all
  Pango output goes through it. Since the Codex fallback mapping is gone,
  `formatters/builders/codex.rs` renders an `other`-kind window with a
  label built from its real duration (e.g. `"1h window"`) instead of
  assuming it must be the 5h or 7d bucket.

  ## TUI — `src/tui/`

  `agent-bar menu` shares the provider/cache/formatter layers above; its
  own render tree (`src/tui/render/`) is a fourth consumer of
  `ProviderQuota`, independent of Waybar/Omarchy. Detail's window rows use
  the same `classify_window`/`is_duplicate_window`/`format_eta`/
  `format_reset_time` helpers as Widget.qml, so the two surfaces never
  drift on labels, dedup, or timezone. Config screens hide Waybar-only
  fields (separators, signal, interval) when `platform::detect()` reports
  Omarchy-only — the same gate `setup`/`update` use.

  ## Omarchy Shell — `Widget.qml` (primary integration)

  The Omarchy 4 (omarchy-shell/Quickshell) plugin is agent-bar's
  first-class target: a native bar-widget with per-provider chips and a
  popup, driven entirely by `agent-bar --format json` and `agent-bar
  config show`/`config apply`. Titlebar actions (refresh, settings mode,
  open TUI) are real Unicode icon buttons, not text hints. Settings mode
  dual-writes: `config apply` owns `providers`/`providerOrder`/
  `displayMode`/`notify.enabled` in `settings.json`; the plugin's own
  `refreshIntervalSec` is written inline into Omarchy's `shell.json` via
  `bar.shell.updateEntryInline`. Motion (bar fill on open, refresh spin,
  hover) is gated by the read-only `menuAnimations` field `config show`
  exposes — editing it stays a `settings.json` concern. `agent-bar update`
  reinstalls the plugin whenever `detect_omarchy_shell()` is true, so the
  binary and the embedded QML never drift apart; `doctor` additionally
  checks the installed manifest version against the running binary. Full
  plugin contract, click routing, and the dual-write table:
  [omarchy-shell.md](omarchy-shell.md).

  ## Waybar — legacy tier

  Waybar keeps working — it gets fixes, not new features. Its contract
  (module IDs, CSS classes, click actions, `signal`-based refresh) lives
  in [waybar-contract.md](waybar-contract.md) and is implemented by
  `src/waybar/contract.rs` and `src/waybar/integration.rs` (grouped under
  `src/waybar/`, re-exported at the crate root as `waybar_contract` /
  `waybar_integration` so the existing `cargo test waybar_contract` /
  `cargo test waybar_integration` filters in `CLAUDE.md` keep working).
  `platform::detect()` is what lets every write path (`setup`, `update`,
  TUI Config Save) skip touching `~/.config/waybar/` entirely on an
  Omarchy-only machine.

  ## Module Map

  | File | Role |
  | --- | --- |
  | `src/main.rs` | Entry point; dispatch by command/flags. |
  | `src/cli.rs` | argv → `CliOptions`; `--help` rendering; command suggestions. |
  | `src/platform.rs` | `Platform { omarchy, waybar }` + `detect()` — the single gate `setup`/`update`/TUI-save use before touching Waybar paths. |
  | `src/config.rs` | Paths (XDG), cache TTL, API endpoints, color thresholds. |
  | `src/config_cmd.rs` | `config show` / `config apply` — editable settings subset (JSON) + read-only `menuAnimations` on `show`. |
  | `src/cache.rs` | File-based quota cache (5 min, cross-process, atomic writes). |
  | `src/providers/mod.rs` | Registration, parallel fan-out, timeout/retry. |
  | `src/providers/base.rs` | `BaseProvider` `get_quota()` orchestration. |
  | `src/providers/{claude,codex,amp,grok}.rs` | Concrete providers. Claude is direct; others extend `BaseProvider`. Each sets `windowKind` at the source. |
  | `src/providers/types.rs` | `ProviderQuota`, `QuotaWindow` (incl. `windowKind`), `Provider`, `AllQuotas`. |
  | `src/formatters/shared.rs` | `classify_window`/`WindowKind`, `is_duplicate_window`, `format_eta`/`format_reset_time`. |
  | `src/formatters/json.rs` | Versioned JSON envelope (`schemaVersion`) — the contract Widget.qml consumes. |
  | `src/formatters/waybar.rs` | Waybar JSON assembly ({text,tooltip,class}) — legacy tier. |
  | `src/formatters/render_pango.rs` | Single XML-escape boundary for Pango. |
  | `src/formatters/terminal.rs` | ANSI terminal view. |
  | `src/waybar/contract.rs` | Generated Waybar modules/CSS/asset install (re-exported as `crate::waybar_contract`). |
  | `src/waybar/integration.rs` | In-place `config.jsonc`/`style.css` patcher (re-exported as `crate::waybar_integration`). |
  | `src/notify.rs` | Best-effort low/critical desktop notifications. |
  | `src/action_right.rs` | Waybar right-click handler — resolves TUI focus (provider detail or Login) from connection state. |
  | `src/omarchy_integration.rs` | Omarchy drop-in install/remove; embeds `Widget.qml` / manifest; `detect_omarchy_shell()`. |

  ## See Also

  - [Commands](commands.md) — public CLI surface, `config`, flags, and internals.
  - [Runtime](runtime.md) — owned paths, settings, cache, credentials.
  - [Waybar contract](waybar-contract.md) — generated modules, classes, click actions (legacy tier).
  - [Omarchy shell](omarchy-shell.md) — plugin drop-in, settings popup, dual-write.
  - [JSON output](json-output.md) — `--format json` / `--watch` schema, incl. `windowKind` and `extra` shapes.
  - [New provider guide](new-provider.md) — implementing a provider on `BaseProvider`.
  ````

- [ ] **Step 3: Rodar e ver passar**

  ```bash
  grep -c "src/waybar/" docs/architecture.md
  grep -c "windowKind" docs/architecture.md
  grep -n "^## Waybar — legacy tier" docs/architecture.md
  grep -n "^## Omarchy Shell" docs/architecture.md
  git diff --check
  ```
  Esperado: as duas primeiras contagens ≥1; as duas buscas de título
  encontram a seção nova; `git diff --check` sem saída.

- [ ] **Step 4: Commit**

  ```bash
  git add docs/architecture.md
  git commit -m "docs: arquitetura reescrita Omarchy-first"
  ```

---

### Task 8: CLAUDE.md — linha `providers::grok_cli` + nota do módulo `src/waybar/`

**Files:**
- Modify: `CLAUDE.md:49` (tabela da Seção 2, logo após a linha de CLI
  locators do Amp) e `CLAUDE.md:69-71` (Seção 3, bullet sobre
  `waybar_integration.rs`)

**Interfaces:**
- Consumes: `src/providers/grok_cli.rs` (locator do binário `grok`, mesmo
  papel de `amp_cli.rs`); produto da PR 4 (`src/waybar/contract.rs` +
  `src/waybar/integration.rs` com re-exports, já citado no CONTRATO DE
  INTERFACES GLOBAIS deste plano).
- Produces: a matriz de verificação (§2) e as regras de módulo (§3) que
  todo agente futuro no repo lê antes de editar `src/waybar/` ou
  `src/providers/grok_cli.rs`.

**Steps:**

- [ ] **Step 1: Confirmar a ausência das duas linhas**

  ```bash
  grep -n "providers::grok_cli" CLAUDE.md; echo "exit=$?"
  grep -n "src/waybar/" CLAUDE.md; echo "exit=$?"
  ```
  Esperado: nenhuma das duas buscas acha nada (`exit=1` nas duas).

- [ ] **Step 2: Adicionar a linha na matriz (Seção 2)**

  Trocar:
  ```
  | CLI locators (Amp CLI) | `cargo test providers::amp_cli` |
  | Contratos Rust | `cargo clippy --all-targets -- -D warnings` |
  ```
  Por:
  ```
  | CLI locators (Amp CLI) | `cargo test providers::amp_cli` |
  | CLI locators (Grok CLI) | `cargo test providers::grok_cli` |
  | Contratos Rust | `cargo clippy --all-targets -- -D warnings` |
  ```

- [ ] **Step 3: Adicionar o bullet sobre `src/waybar/` na Seção 3**

  Trocar:
  ```
  - **Nunca round-trip live Waybar config via `serde_json`.**
    Os `.jsonc` têm comentários e ordem que precisam sobreviver.
    `waybar_integration.rs` patcha in-place.
  ```
  Por:
  ```
  - **Nunca round-trip live Waybar config via `serde_json`.**
    Os `.jsonc` têm comentários e ordem que precisam sobreviver.
    `waybar_integration.rs` patcha in-place.
  - **Módulo `src/waybar/` agrupa o tier legado** (`src/waybar/contract.rs`,
    `src/waybar/integration.rs`), com re-exports em `lib.rs` para
    `crate::waybar_contract`/`crate::waybar_integration`. Os filtros
    `cargo test waybar_contract`/`cargo test waybar_integration` da matriz
    (§2) seguem em vigor — se um teste for movido pro módulo interno,
    confira que o filtro ainda casa antes de commitar.
  ```

- [ ] **Step 4: Rodar e ver passar**

  ```bash
  grep -n "providers::grok_cli" CLAUDE.md
  grep -n "src/waybar/" CLAUDE.md
  git diff --check
  ```
  Esperado: as duas buscas encontram as linhas novas; `git diff --check`
  sem saída.

- [ ] **Step 5: Commit**

  ```bash
  git add CLAUDE.md
  git commit -m "docs: matriz grok_cli e nota do módulo waybar no CLAUDE.md"
  ```

---

### Task 9: CHANGELOG.md — entrada 9.0.0 + verificação final de consistência

**Files:**
- Modify: `CHANGELOG.md:8-10` (insere `## [9.0.0]` entre `## [Unreleased]`
  e `## [8.5.0]`)

**Interfaces:**
- Consumes: todas as decisões do spec (Seções A-G + tabela "Decisões
  travadas") e o resultado das Tasks 1-8 deste plano.
- Produces: texto que o dono usa **verbatim** no `docs/releasing.md`
  runbook quando decidir cortar o release (fora do escopo deste plano).

**Steps:**

- [ ] **Step 1: Confirmar o baseline**

  ```bash
  grep -n "^## \[9.0.0\]" CHANGELOG.md; echo "exit=$?"
  sed -n '8,10p' CHANGELOG.md
  ```
  Esperado: o grep não acha nada (`exit=1`); o `sed` mostra:
  ```
  ## [Unreleased]

  ## [8.5.0] - 2026-07-21
  ```

- [ ] **Step 2: Inserir a entrada 9.0.0**

  Trocar:
  ```
  ## [Unreleased]

  ## [8.5.0] - 2026-07-21
  ```
  Por:
  ```
  ## [Unreleased]

  ## [9.0.0] - 2026-07-21

  Redesign completo do popup (Omarchy-shell) + Waybar rebaixada a tier
  legado. Marco de produto — sem mudança de contrato JSON.

  ### Added
  - **Widget.qml redesenhado**: hero % por provider igual ao chip, título
    `agent-bar` + "há Xm" relativo, ações de topo viram botões Unicode
    reais (↻ refresh, ⚙︎ settings, ❯ abrir TUI) em vez de texto/link. Um
    cartão por provider, grade de colunas fixas (rótulo · barra · % ·
    reset), countdown (`1h 46m · 18:30` / `7d 0h · seg 16:43`) em toda
    janela. Largura `540` (antes `370`), igual nos dois modos.
  - **`extra` visível no popup**: créditos do Amp (`$X · replenish`),
    sessões/turnos/modelo do Grok, extra usage do Claude quando existir —
    antes só existiam no `--format json`, nunca chegavam à tela.
  - **Motion no popup**: barras preenchem na abertura (M1, stagger),
    `↻` gira durante o fetch (M2), hover nos botões (M4) — gated por
    `menu.animations` (exposto read-only em `config show` como
    `menuAnimations`).
  - **`windowKind`** (`"fiveHour" | "sevenDay" | "daily" | "context" |
    "other"`) em `QuotaWindow`, decidido uma vez no Rust por cada
    provider; dedup display-level (`(windowKind, resetsAt, remaining)`)
    consumido pela TUI e por Widget.qml — some o "Weekly" triplicado do
    Codex no plano Plus.
  - **Countdown e fuso local na TUI**: `fmt_reset` do Detail passa a usar
    `format_reset_time`/`format_eta` (antes fatiava o ISO em UTC cru, sem
    contagem regressiva).
  - Settings do popup ganham Providers (toggle + reordenar), Exibição
    (segmentado remaining/used + prévia ao vivo da barra) e Alertas &
    atualização (notify + intervalo) como painéis próprios.
  - **`platform::detect()`** (`src/platform.rs`) único ponto de decisão
    Omarchy/Waybar, usado por `setup`, `update` (os dois ramos) e pelo Save
    da TUI Config — nenhum dos três cria mais `~/.config/waybar/` do zero
    numa máquina Omarchy-only.
  - **`agent-bar update` reinstala o plugin Omarchy** quando o shell é
    detectado, eliminando o drift binário↔QML que antes exigia rodar
    `setup` manualmente. `doctor` ganhou checagem de versão do manifest
    instalado vs binário.
  - Módulo `src/waybar/` agrupando o contrato Waybar (antigo
    `waybar_contract.rs`/`waybar_integration.rs`) como tier legado isolado.

  ### Changed
  - **Fix do mislabeling do Codex**: `build_model_windows` não força mais
    `primary→fiveHour`/`secondary→sevenDay` quando a classificação diverge
    — uma janela fora de tolerância vira `other` com rótulo pela duração
    real (ex.: "1h window").
  - **Settings mode do popup**: salvar passa a ser **só pelo botão** — o
    atalho de teclado `s` para salvar foi removido; o rodapé de dicas em
    texto vira botões clicáveis de verdade.
  - Migração de settings **v2 → v3**: `waybar.show_percentage` é dropada
    silenciosamente e o arquivo é regravado na versão nova.
  - TUI Config esconde os campos exclusivos de Waybar (separadores,
    signal, intervalo do Waybar) quando `platform::detect()` reporta
    Omarchy-only.
  - `docs/waybar-contract.md` e `README.md` marcam Waybar como **tier
    legado**: funciona, recebe fix, não recebe feature nova.

  ### Removed
  - Legado morto: variante `Command::Terminal`,
    `waybar_contract::get_all_provider_ids`, `install::ensure_amp_cli`,
    `amp_cli::AMP_INSTALL_COMMAND` duplicada, dependência `tokio-util`,
    `ConfigField::settings_key()`, 7 variantes órfãs de `Icon`.

  ### Breaking
  - Nenhuma no contrato `--format json` — `windowKind` é aditivo,
    `schemaVersion` continua `1`.

  ## [8.5.0] - 2026-07-21
  ```

- [ ] **Step 3: Rodar e ver passar (entrada nova)**

  ```bash
  grep -n "^## \[9.0.0\]" CHANGELOG.md
  grep -c "windowKind" CHANGELOG.md
  git diff --check
  ```
  Esperado: o grep acha a linha nova; a contagem ≥1; `git diff --check`
  sem saída.

- [ ] **Step 4: Verificação final de consistência (todo o diff da PR 5)**

  ```bash
  git diff --check master
  grep -rn "show_percentage" README.md CONTRIBUTING.md docs/*.md CLAUDE.md; echo "exit acima deve ser 1 (nada encontrado)"
  grep -rn "get_all_provider_ids" README.md CONTRIBUTING.md docs/*.md CLAUDE.md; echo "exit acima deve ser 1"
  grep -rn "^## Future" README.md CONTRIBUTING.md docs/*.md; echo "exit acima deve ser 1"
  grep -c "grok" docs/waybar-contract.md docs/runtime.md docs/troubleshooting.md
  ```
  Esperado: `git diff --check master` sem saída (nenhum erro de
  whitespace em toda a PR); os três `grep -rn` de proibição não encontram
  nada (`exit=1` cada); a última linha mostra contagem ≥1 para os três
  arquivos — confirma que nenhum doc "vivo" ainda cita
  `show_percentage`/`get_all_provider_ids`/uma seção `## Future`, e que
  `grok` está presente nos três docs tocados pelas Tasks 3/5/6.

- [ ] **Step 5: Commit**

  ```bash
  git add CHANGELOG.md
  git commit -m "docs: entrada 9.0.0 no CHANGELOG"
  ```

---

### Task 10: docs/commands.md — `menuAnimations` no exemplo e na tabela de campos

**Files:**
- Modify: `docs/commands.md:33-42` (exemplo de envelope do `config show`) e
  `docs/commands.md:47-52` (tabela de campos)

**Interfaces:**
- Consumes: `menuAnimations` em `config show` (`src/config_cmd.rs`), produto
  da PR 4 conforme o CONTRATO DE INTERFACES GLOBAIS deste plano —
  read-only, reflete `Settings.menu.animations`, ignorado em `apply`.
- Produces: nada consome; é a documentação faltante de um campo já real do
  envelope.

**Steps:**

- [ ] **Step 1: Confirmar o gap**

  ```bash
  grep -n "menuAnimations" docs/commands.md; echo "exit=$?"
  ```
  Esperado: nenhuma ocorrência (`exit=1`).

- [ ] **Step 2: Adicionar `menuAnimations` ao exemplo de envelope**

  Trocar:
  ```
  {
    "schemaVersion": 1,
    "providers": ["claude", "codex", "amp", "grok"],
    "providerOrder": ["claude", "codex", "amp", "grok"],
    "displayMode": "remaining",
    "notify": { "enabled": true }
  }
  ```
  Por:
  ```
  {
    "schemaVersion": 1,
    "providers": ["claude", "codex", "amp", "grok"],
    "providerOrder": ["claude", "codex", "amp", "grok"],
    "displayMode": "remaining",
    "notify": { "enabled": true },
    "menuAnimations": true
  }
  ```

- [ ] **Step 3: Adicionar a linha na tabela de campos**

  Trocar:
  ```
  | `notify.enabled` | Desktop low/critical notifications. |
  ```
  Por:
  ```
  | `notify.enabled` | Desktop low/critical notifications. |
  | `menuAnimations` | Read-only; reflete `Settings.menu.animations`. Ignorado em `apply`. |
  ```

- [ ] **Step 4: Rodar e ver passar**

  ```bash
  grep -n "menuAnimations" docs/commands.md
  git diff --check
  ```
  Esperado: encontra as duas linhas novas (exemplo + tabela); `git diff
  --check` sem saída.

- [ ] **Step 5: Commit**

  ```bash
  git add docs/commands.md
  git commit -m "docs: menuAnimations no exemplo e tabela do commands"
  ```
