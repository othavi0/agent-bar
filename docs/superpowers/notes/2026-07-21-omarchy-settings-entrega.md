# Entrega 8.5.0 — settings Omarchy + CLI simplify

Data: 2026-07-21 · Release: [v8.5.0](https://github.com/othavi0/agent-bar/releases/tag/v8.5.0) · PR: #18

## O que fizemos

1. **Brainstorming + grill** — mapa do sistema (CLI, TUI ~9k LOC, plugin QML,
   legado Waybar no Config), referência `omarchy.model-usage`, decisões 1–9
   travadas com o usuário (só Omarchy no right-click; subset essencial;
   dual-write; mesmo popup; TUI por link; Config TUI intocada; filtro QML;
   `config show|apply`; help opção A).
2. **Spec + plano** — design em
   `docs/superpowers/specs/2026-07-21-omarchy-settings-and-cli-simplify-design.md`
   e plano SDD em
   `docs/superpowers/plans/2026-07-21-omarchy-settings-and-cli-simplify.md`.
3. **Implementação SDD** (worktree `feat/omarchy-settings-cli`, implementer +
   review por task, review de branch, fix-wave):
   - `src/config_cmd.rs` — show/apply subset
   - CLI parse/dispatch + help + aliases
   - `assets/omarchy/Widget.qml` — settingsMode, filtro, teclas, dual-write
   - docs operacionais
4. **Release** — tag v8.5.0, CI musl + AUR; local validado em 8.5.0 com plugin
   e `config show` ok.

## Decisões (resumo)

| # | Decisão |
| --- | --- |
| 1 | Right-click settings **só Omarchy**; Waybar igual |
| 2 | Settings: providers, ordem, display, refresh, notify |
| 3 | Persist: CLI → settings.json; interval → shell.json |
| 4 | Mesmo popup, `settingsMode`, largura ~370 |
| 5 | TUI: CLI + link no popup |
| 6 | TUI Config **intocada** neste trabalho |
| 7 | Filtro no QML; envelope JSON de poll intacto |
| 8 | `config show` / `config apply` |
| 9 | CLI simplify A (help + hide + aliases) |

ADRs: `docs/adr/0001`–`0003`. Glossário: `CONTEXT.md`.

## O que não fizemos (de propósito)

- Podar TUI (Detail/History/Login/Config)
- Port de charts/history pro QML
- Filtrar providers no envelope `--format json`
- `config apply` recarregar Waybar
- Deletar `action-right` ou hard-cut “só Omarchy”

## Como validar no desktop

Right-click → settings; Save; chip some/volta; middle refresh; link TUI;
`agent-bar config show` reflete toggles; help sem `action-right` na vitrine.
