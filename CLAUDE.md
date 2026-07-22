# agent-bar — Agent Instructions

Monitor de quotas LLM para Waybar (Claude, Codex, Amp).
`AGENTS.md` é shim de compat Codex. **O código em `src/` é a fonte da verdade.**

## 1. Hard Rules

Quebrar qualquer uma quebra build, desktop do usuário, ou contrato de produto.

- **Rust/cargo only.** Toolchain via rustup. Sem Node, npm, bun, pnpm, yarn,
  ts-node, Deno em runtime ou testes.
- **Nunca converter `scripts/agent-bar-open-terminal` para Rust.** É helper
  Bash que abre terminal externo; permanece como script.
- **Não mutar desktop ao vivo como verificação.** Não rodar `agent-bar setup`/
  `update`/`uninstall`/`remove` sem aprovação explícita. `assets install`
  apenas em paths injetados (temp dirs, `--waybar-dir`, `XDG_*`).
- **Não hand-edit `~/.config/waybar` ou `~/.config/agent-bar` em testes.** Use
  temp dirs + flags de path + env `XDG_*`.
- **stdout limpo.** Waybar parseia stdout como JSON; logs vão para stderr
  (`logger` já faz isso). Só comandos terminal/TUI escrevem texto rico.
- **Legacy permanece morto.** Nomes `qbar`, `agent-bar-omarchy`, providers
  `antigravity` e `llm-usage`, dependência de theme-repo externo, e coupling
  com tema Omarchy foram removidos em 4.0.0. Não reintroduzir como comando,
  module ID, seletor CSS, settings key, symlink ou cache key. Menções em
  `CHANGELOG.md` são históricas e podem ficar.

## 2. Verification Matrix

Use a verificação mais estreita; só amplie se contrato compartilhado se moveu.

**Gotcha RTK:** o hook RTK reformata output do cargo — a string `test result:`
pode não aparecer. Use apenas um filtro posicional por invocação de `cargo test`.

| Área da mudança | Comando |
| --- | --- |
| Docs / instruções de agente | `git diff --check` |
| CLI parsing / help | `cargo test cli` |
| Cache | `cargo test cache` |
| Settings | `cargo test settings` |
| Config / paths | `cargo test config` |
| Um provider | `cargo test providers::<provider>` (ex: `providers::claude`) |
| `BaseProvider` orchestration | `cargo test providers::base` |
| Formatters / tooltips / segments | `cargo test formatters` |
| Golden / Waybar export contract | `cargo test --test golden` |
| Waybar contract (módulos/CSS) | `cargo test waybar_contract` |
| Waybar integration | `cargo test waybar_integration` |
| Update flow | `cargo test update` |
| Theme / colors / identity | `cargo test theme && cargo test app_identity` |
| CLI locators (Amp CLI) | `cargo test providers::amp_cli` |
| CLI locators (Grok CLI) | `cargo test providers::grok_cli` |
| Contratos Rust | `cargo clippy --all-targets -- -D warnings` |
| Mudanças amplas antes de handoff | `cargo test && cargo clippy --all-targets -- -D warnings` |

## 3. Project-Specific Rules

- **Use as constantes de identidade** (`APP_NAME`, `WAYBAR_*`,
  `TERMINAL_HELPER_NAME`, `BACKUP_SUFFIX` em `src/app_identity.rs`) em vez de
  hardcoded strings.
- **Provider error strings são contrato.** Testes assertam strings verbatim;
  alterar uma é mudança de contrato. Mantenha úteis e estáveis.
- **Nunca `unwrap()`/`expect()` em código de produção.** Estreite com guard
  explícito que propaga erro (`?` ou `anyhow::bail!`). `unwrap` esconde panics
  que precisam virar erros explícitos.
- **`ClaudeProvider` implementa `Provider` direto, não estende `BaseProvider`.**
  Codex/Amp estendem. Não force Claude no template — ele gerencia
  cache inline porque o fluxo não cabe.
- **XML-escape acontece SÓ em `render_pango.rs`.** Builders nunca escapam;
  segments `raw` bulam color-wrap E escape. Romper isso vira XSS no tooltip
  ou texto literal quebrado.
- **Nunca round-trip live Waybar config via `serde_json`.**
  Os `.jsonc` têm comentários e ordem que precisam sobreviver.
  `waybar_integration.rs` patcha in-place.
- **Módulo `src/waybar/` agrupa o tier legado** (`src/waybar/contract.rs`,
  `src/waybar/integration.rs`), com re-exports em `lib.rs` para
  `crate::waybar_contract`/`crate::waybar_integration`. Os filtros
  `cargo test waybar_contract`/`cargo test waybar_integration` da matriz
  (§2) seguem em vigor — se um teste for movido pro módulo interno,
  confira que o filtro ainda casa antes de commitar.

## 4. Testing Patterns

- `#[tokio::test]` para async; `#[test]` para sync. Sem credenciais reais,
  sem CLIs vivas, sem rede, sem Waybar real. Mock via seams (traits/fn
  pointers); snapshots via `insta`.
- **Set `XDG_CONFIG_HOME` / `XDG_CACHE_HOME` ANTES de qualquer import que
  leia `src/config.rs`.** Config lê env no carregamento; setar depois não
  tem efeito.
- Restaure env e global state em `drop`/`after_each`.
- Snapshot terminal é sanitized (ANSI strip); Waybar é byte-for-byte (Pango
  importa). Atualize snapshots só quando o display contract mudar de propósito.

## 5. Workflow de Edição

1. `git status` — não toque em mudanças não-relacionadas.
2. Leia o mínimo pra entender o contrato que muda.
3. Edits focados, respeitando boundaries de módulo.
4. Verificação focada (tabela §2); amplie só se contrato se moveu.
5. Reporte o que mudou, o que verificou, e risco não-verificado.

## 6. Conventions

- Rust strict + `cargo clippy -D warnings`. `cargo fmt` aplica formatting
  (4 espaços, Rust style guide). Unused imports = erro de clippy.
- Identificadores e nomes de arquivo em inglês (snake_case). Comunicação de
  repo e commits em português. Conventional Commits, subject ≤ 50 chars.

## 7. Adicionar provider

Veja [`docs/new-provider.md`](docs/new-provider.md) para o checklist completo.
**Estenda `BaseProvider`** salvo se não couber (como Claude). Mensagem padrão
de não-logado: `` Not logged in. Open `agent-bar menu` and choose Provider login. ``

## 8. Release

Workflow `.github/workflows/publish.yml` dispara em `release: published` e
builda binário musl via `cargo-zigbuild`. Versão vem de `Cargo.toml`
(`CARGO_PKG_VERSION`). Sem `NPM_TOKEN` — distribuição via `install.sh`,
AUR e `cargo binstall`.

Para cortar release: bumpar `version` em `Cargo.toml`, atualizar
`CHANGELOG.md`, commitar, criar GitHub Release com tag `v<version>`.
**Runbook completo (passo a passo, incl. preenchimento do sha256 e push pro AUR):
[`docs/releasing.md`](docs/releasing.md).**

## 9. Pointers

- `README.md`, `CONTRIBUTING.md` — quick start e contributor workflow.
- `docs/commands.md`, `docs/runtime.md`, `docs/integration.md`,
  `docs/waybar-contract.md`, `docs/new-provider.md`, `docs/troubleshooting.md`.
- `docs/superpowers/plans/`, `docs/superpowers/specs/` — histórico de refactors
  fase 1-3 e publicação automática (contexto, não regras vigentes).
- `CHANGELOG.md` — histórico; só editar ao cortar release.

## Agent skills

### Issue tracker

Issues vivem no GitHub Issues de `othavi0/agent-bar` (via `gh` CLI). PRs
externos não são superfície de triage. Ver `docs/agents/issue-tracker.md`.

### Triage labels

Vocabulário canônico, sem overrides: `needs-triage`, `needs-info`,
`ready-for-agent`, `ready-for-human`, `wontfix`. Ver `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` + `docs/adr/` na raiz (criados lazy). Ver
`docs/agents/domain.md`.
