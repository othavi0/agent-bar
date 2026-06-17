# agent-bar — Agent Instructions

Monitor de quotas LLM para Waybar (Claude, Codex, Amp).
`AGENTS.md` é shim de compat Codex. **O código em `src/` é a fonte da verdade.**

## 1. Hard Rules

Quebrar qualquer uma quebra build, desktop do usuário, ou contrato de produto.

- **Bun only.** Sem Node, npm, pnpm, yarn, ts-node, Deno em runtime ou testes.
- **Nunca rodar `bun ./scripts/agent-bar`.** É shim Bash; rode `./scripts/agent-bar`
  ou `bun run start`. Bun interpreta o shim como JS e falha.
- **Nunca converter shims em `scripts/` para TypeScript.**
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

| Área da mudança | Comando |
| --- | --- |
| Docs / instruções de agente | `git diff --check` |
| CLI parsing / help | `bun test tests/cli.test.ts` |
| Cache | `bun test tests/cache.test.ts` |
| Settings | `bun test tests/settings.test.ts` |
| Um provider | `bun test tests/providers/<provider>.test.ts` (Codex inclui também `codex-appserver.test.ts`) |
| `BaseProvider` orchestration | `bun test tests/providers/base.test.ts` |
| Formatters / tooltips / classes | `bun test tests/formatters.test.ts tests/formatters-snapshot.test.ts tests/formatters-segments.test.ts` |
| Waybar export contract | `bun test tests/waybar-contract.test.ts` |
| Update flow | `bun test tests/update.test.ts` |
| `package.json` `files`/`bin`/release | `bun test tests/package.test.ts` |
| Theme / colors / identity | `bun test tests/theme.test.ts tests/colors.test.ts tests/config.test.ts tests/app-identity.test.ts` |
| CLI locators | `bun test tests/amp-cli.test.ts` |
| Contratos TypeScript | `bun run typecheck` |
| Mudanças amplas antes de handoff | `bun test && bun run typecheck && bun run lint` |

## 3. Project-Specific Rules

- **Use as constantes de identidade** (`APP_NAME`, `WAYBAR_*`,
  `TERMINAL_HELPER_NAME`, `BACKUP_SUFFIX` em `src/app-identity.ts`) em vez de
  hardcoded strings.
- **Provider error strings são contrato.** Testes assertam strings verbatim;
  alterar uma é mudança de contrato. Mantenha úteis e estáveis.
- **Nunca `!` non-null assertion.** Estreite com guard explícito que `throw`a.
  *Why:* biome desativa `noNonNullAssertion` (linter permite), mas o projeto
  bane. `!` esconde nulls que precisam virar erros explícitos.
- **`ClaudeProvider` implementa `Provider` direto, não estende `BaseProvider`.**
  Codex/Amp estendem. Não force Claude no template — ele gerencia
  cache inline porque o fluxo não cabe.
- **XML-escape acontece SÓ em `render-pango.ts`.** Builders nunca escapam;
  segments `raw` bulam color-wrap E escape. Romper isso vira XSS no tooltip
  ou texto literal quebrado.
- **Nunca round-trip live Waybar config via `JSON.parse`/`JSON.stringify`.**
  Os `.jsonc` têm comentários e ordem que precisam sobreviver. `waybar-integration.ts`
  patcha in-place.
- **`waybar.ts` cacheia settings 5s** porque Waybar pulla em interval apertado.

## 4. Testing Patterns

- `bun:test`. Sem credenciais reais, sem CLIs vivas, sem rede, sem Waybar real.
  Mock `fs`, `fetch`, `spawn`, dados de app-server/session.
- **Set `XDG_CONFIG_HOME` / `XDG_CACHE_HOME` ANTES de importar `src/config.ts`
  ou qualquer módulo que o importe.** Config lê env no import-time; setar
  depois não tem efeito.
- Restaure env e global state em `afterEach`.
- Snapshot terminal é sanitized (ANSI strip); Waybar é byte-for-byte (Pango
  importa). Atualize snapshots só quando o display contract mudar de propósito.

## 5. Workflow de Edição

1. `git status` — não toque em mudanças não-relacionadas.
2. Leia o mínimo pra entender o contrato que muda.
3. Edits focados, respeitando boundaries de módulo.
4. Verificação focada (tabela §2); amplie só se contrato se moveu.
5. Reporte o que mudou, o que verificou, e risco não-verificado.

## 6. Conventions

- TypeScript strict, ESM only. Biome aplica formatting (2 espaços, single
  quote, 120 cols, unused import = erro).
- Identificadores e nomes de arquivo em inglês. Comunicação de repo e
  commits em português. Conventional Commits, subject ≤ 50 chars.

## 7. Adicionar provider

Veja [`docs/new-provider.md`](docs/new-provider.md) para o checklist completo.
**Estenda `BaseProvider`** salvo se não couber (como Claude). Mensagem padrão
de não-logado: `` Not logged in. Open `agent-bar menu` and choose Provider login. ``

## 8. Release

Workflow `.github/workflows/publish.yml` dispara em `release: published` e roda
`release:check` + `publish:npm`. Precisa do secret **`NPM_TOKEN`** (npm automation
token — bypassa 2FA, requisito do CI).

Para cortar release: bumpar `package.json` version, atualizar `CHANGELOG.md`,
commitar, criar GitHub Release com tag `v<version>`.

## 9. Pointers

- `README.md`, `CONTRIBUTING.md` — quick start e contributor workflow.
- `docs/commands.md`, `docs/runtime.md`, `docs/integration.md`,
  `docs/waybar-contract.md`, `docs/new-provider.md`, `docs/troubleshooting.md`.
- `docs/superpowers/plans/`, `docs/superpowers/specs/` — histórico de refactors
  fase 1-3 e publicação automática (contexto, não regras vigentes).
- `CHANGELOG.md` — histórico; só editar ao cortar release.
