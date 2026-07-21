# Omarchy settings nativo + simplificação da CLI

Data: 2026-07-21 · Status: aprovado em brainstorming

## Contexto

No Omarchy 4 (Quattro / PR basecamp/omarchy#6231) a barra é `omarchy-shell`
(Quickshell). O plugin `agent-bar.usage` já entrega:

| Clique | Hoje |
| --- | --- |
| Esquerdo | `PopupCard` nativo de quotas |
| Direito | terminal flutuante → `agent-bar menu` (TUI inteira) |
| Meio | refresh forçado |

O first-party `omarchy.model-usage` usa outro idioma: right-click abre o
**mesmo popup** em modo settings (toggles, numbers, save em `shell.json`).

A TUI (~9k LOC) ainda tem valor real em Detail (charts), History e Login.
A tela Config da TUI edita 7 campos, vários mortos no Omarchy (separators,
signal) e o Save sempre chama `apply_waybar_integration`.

Paralelo: a CLI pública mistura primários, internos de Waybar (`action-right`,
`export`, `assets`) e aliases redundantes (`remove` vs `uninstall`, `-t` vs
`status`), com help ainda Waybar-primary.

## Objetivos

1. **Omarchy only:** right-click → settings nativo no mesmo popup (estilo
   model-usage); left = usage mais largo; middle = refresh.
2. **Settings essenciais:** providers on/off, ordem, display remaining/used,
   refresh interval, notify on/off.
3. **CLI:** `config show` / `config apply` como bridge; help limpo; internos
   escondidos; aliases; **sem** remover suporte Waybar.
4. TUI continua via `agent-bar menu` + link no popup. TUI Config **intocada**.

## Não-objetivos

- Podar ou redesenhar Detail / History / Login / Config da TUI.
- Port de charts/history para QML.
- `--watch` como fonte do widget.
- Bump `schemaVersion` do envelope de quota; filtrar providers no JSON de poll.
- `config apply` recarregar Waybar.
- Deletar `action-right`, path Waybar, ou default waybar JSON.
- Drag-and-drop de ordem; login OAuth no popup.
- Major “Omarchy-only binary”.

## Decisões travadas

| # | Decisão |
| --- | --- |
| 1 | Right-click settings **só no plugin Omarchy**; Waybar left/right iguais. |
| 2 | Subset: providers, order, displayMode, refreshIntervalSec, notify. |
| 3 | Persist: Rust `settings.json` via CLI; interval via `updateEntryInline`. |
| 4 | Mesmo `PopupCard`, `settingsMode`; largura ~`Style.space(370)`. |
| 5 | TUI: CLI + link “Abrir menu (TUI)” no popup. |
| 6 | TUI Config sem mudanças neste trabalho. |
| 7 | `waybar.providers` canônico; filtro de chips no QML; envelope quota intacto. |
| 8 | `agent-bar config show` / `config apply`. |
| 9 | CLI simplify A: help + hide internals + aliases (não hard-cut). |

## §1 Arquitetura

```text
Widget.qml
  left  → popupOpen, settingsMode=false   (usage)
  right → openSettings()                  (show + settingsMode=true)
  mid   → refresh(true)
  Save  → config apply --json …  then  updateEntryInline(refreshIntervalSec)
  link  → helper + agent-bar menu

Binário
  settings.json  ← load / normalize / save
  config show    → subset editável
  config apply   → patch + save (sem apply_waybar_integration)
  --format json  → envelope quota INALTERADO
```

### Fonte de verdade

| Dado | Casa | Writer |
| --- | --- | --- |
| providers, providerOrder, displayMode, notify.enabled | `~/.config/agent-bar/settings.json` | `config apply` |
| refreshIntervalSec | entry do plugin em `shell.json` | `updateEntryInline` |
| quotas | cache + APIs | poll `--format json` |

### Filtro de chips

1. Boot e pós-apply: `config show` (ou stdout do apply) → `enabledIds` + order.
2. Poll de quota continua full-envelope.
3. Chips e seções usage usam só providers em `enabledIds`, na `providerOrder`.
4. Até o primeiro `show` retornar: **não filtrar** (evita flash vazio).

### Save (um botão, dois writes)

1. Normaliza draft no QML.
2. `config apply --json` (subset Rust).
3. Se exit ≠ 0 → status de erro; **não** grava interval.
4. Se OK → `updateEntryInline` com `refreshIntervalSec` (merge settings do plugin).
5. Atualiza `enabledIds`, status “Saved”, `refresh(true)`.
6. Apply OK + inline falha → “settings saved; interval not persisted”.

## §2 Contrato CLI — `config`

### Superfície

```bash
agent-bar config show [--format json]     # default json
agent-bar config apply --json '<blob>'    # ou --json - (stdin)
agent-bar config apply --file <path>      # opcional (testes)
agent-bar config --help
```

Exit: `0` ok · `1` validação, uso (args) ou IO (igual ao resto da CLI).

### `config show` — envelope

Stdout só JSON; logs em stderr.

```json
{
  "schemaVersion": 1,
  "providers": ["claude", "amp", "codex", "grok"],
  "providerOrder": ["claude", "amp", "codex", "grok"],
  "displayMode": "remaining",
  "notify": { "enabled": true }
}
```

| Campo | Tipo | Notas |
| --- | --- | --- |
| `schemaVersion` | number | Contrato **deste** comando (≠ quota schema). v1 = este subset. |
| `providers` | string[] | Habilitados, pós-normalização. |
| `providerOrder` | string[] | Ordem efetiva. |
| `displayMode` | `"remaining"` \| `"used"` | |
| `notify.enabled` | bool | |

Valores = pós-`settings::load` + normalização. **Não** inclui separators,
signal, interval Waybar, fxRate, menu, glyph, cache, windowPolicy,
`refreshIntervalSec`.

### `config apply` — input

Mesmo shape; campos **omitidos** = não mudar.

Regras:

1. `schemaVersion` ausente ou ≠ 1 → erro.
2. Campo presente → substitui (com normalização).
3. `providers: []` ou vazio após normalize → **erro** (≥ 1 known id).
4. Ids desconhecidos → drop (igual load); se sobrar vazio → erro.
5. `providerOrder`: `normalize_provider_selection`.
6. `displayMode` inválido → erro (fail loud no apply).
7. Merge: load → patch → `save` atômico.

Stdout de sucesso: **mesmo envelope do show** (estado pós-save), para o QML
atualizar `enabledIds` sem segundo show.

Stderr em erro: mensagem curta.

### Side-effects do apply

| Ação | Faz? |
| --- | --- |
| Gravar `settings.json` | Sim |
| `apply_waybar_integration` / reload Waybar | **Não** |
| Invalidar cache de quota | **Não** |
| Tocar `shell.json` | **Não** |

Desktop misto (Waybar + Omarchy): providers no `settings.json` alinham; módulos
Waybar só na próxima TUI Config / `setup`. Documentar em `docs/omarchy-shell.md`.

### Implementação

- Dispatch em `cli.rs` / `main.rs`.
- Módulo fino (`src/config_cmd.rs` ou equivalente); reusa
  `settings::load` / `save` / `normalize_provider_selection`.
- Sem UI.

## §3 QML / UX

Registro product: densidade, controles do shell, sem paleta própria.
Referência: `/usr/share/omarchy/shell/plugins/model-usage/Widget.qml`.

### Popup

| Hoje | Alvo |
| --- | --- |
| `contentWidth: Style.space(300)` | `Style.space(370)` (paridade model-usage) |
| max height ~480 | max `Style.space(560)` |

- `close()` reseta `popupOpen` **e** `settingsMode`.
- Cores: `bar.foreground`, `Color.accent`, `urgent`, dim via `Qt.darker`.

### Gestos e teclado

| Input | Comportamento |
| --- | --- |
| Left | usage; toggle open se já no mesmo modo |
| Right | `openSettings()` |
| Middle | `refresh(true)` |
| Fora / Esc | fecha + sai de settings (descarta draft) |
| `s` | usage → settings; settings → save |
| Header “← Usage” | volta usage sem salvar |
| Save | dual-write (§1) |

### Usage (ajustes)

- Largura nova.
- Rodapé: timestamp; `right-click: settings · middle: refresh`; botão texto
  **Abrir menu (TUI)** → `openTui()` (helper existente).

### Settings — seções

1. **Providers** — Toggle por known id (claude/codex/amp/grok); ↑↓ entre
   enabled; não desligar o último (status de erro).
2. **Display** — segmento Remaining | Used (`displayMode`).
3. **Alerts** — Toggle desktop notifications (`notify.enabled`).
4. **Refresh** — NumberField 30–3600 step 30 (`refreshIntervalSec`, só shell).

Vocabulário: Toggle, NumberField/stepper, botões header — padrão model-usage.
Sem TextField de lista `claude, codex`; sem drag-and-drop; sem modal extra.

### displayMode nos chips

Label do chip respeita o modo:

- `remaining` → `Math.round(primary.remaining) + "%"` (hoje).
- `used` → `used` do envelope se presente e finito; senão derivar
  `100 - remaining` quando remaining for finito.

Sem isso o setting é mentira visual no Omarchy.

### Estado QML (resumo)

```text
popupOpen, settingsMode, settingsBusy, settingsStatusText
draft: providers, providerOrder, displayMode, notifyEnabled, refreshIntervalSec
enabledIds  // pós show/apply; filtra chips + usage
```

## §4 Simplificação da CLI (opção A)

### Help humano (grupos)

```text
Usage
  (default)              Waybar JSON snapshot (generated modules)
  status                 Terminal quota view
  menu                   Interactive TUI
  config show | apply    Read/write editable settings (JSON)

Setup
  setup | update | uninstall | doctor

Machine
  --format json | --watch | -p | -r | …
```

### Esconder do help (parse **permanece**)

- `action-right` — modules Waybar gerados continuam usando.
- `menu-font` — helper Bash.
- `assets install`, `export waybar-modules`, `export waybar-css` — packager/test;
  docs de integração/packaging, não help de usuário.

### Aliases

- `--terminal` / `-t` → mesmo path que `status` (silencioso).
- `remove` → equivalente a `uninstall --yes` (comportamento forçado preservado).

### Não neste release

- Renomear default para subcomando `waybar` explícito.
- Deletar `action-right` ou unificar em `menu --provider` (ficaria opção B).
- Mudar default de output para JSON.

## §5 Erros (resumo operacional)

| Situação | Comportamento |
| --- | --- |
| show falha | Status erro; sem filtro até haver dados bons |
| apply falha | Status stderr; não grava interval |
| Process busy | Ignora segundo Save/Show |
| close durante Save | UI fecha; resultado do Process não reabre/não aplica draft fantasma |
| quota JSON inválido | `stale` (inalterado) |

## §6 Testes e verificação

| Camada | Cobertura |
| --- | --- |
| Unit config_cmd | merge, schema, empty providers, unknown ids, displayMode, notify |
| CLI parse | config show/apply; aliases remove/-t; help não lista internos |
| omarchy / golden | manifest description; se houver golden QML, right-click ≠ menu |
| Regressão | `cargo test cli`, `settings`, `omarchy_integration`; waybar_contract path modules |
| Manual desktop | chips filtrados; settings save; interval; link TUI; largura; lado a lado model-usage |

Hard rules: sem mutar desktop live nos testes; temp dirs + `--omarchy-plugins-dir` + `XDG_*`.

## §7 Docs a atualizar na implementação

- `docs/commands.md` — taxonomia nova; `config`; internos em seção própria.
- `docs/omarchy-shell.md` — clicks, settingsMode, dual-write, filtro, trade-off Waybar.
- `docs/architecture.md` — dispatch `config`; nota action-right ainda Waybar.
- `README.md` — Omarchy: right = settings; menu = TUI.
- `CHANGELOG.md` — só no corte de release.

## §8 Ordem de implementação sugerida

1. `config show` / `config apply` + testes Rust.
2. Help + aliases + hide internals + testes help/parse.
3. QML: largura, settingsMode, SettingsContent, filtro, Save, link TUI.
4. Docs.
5. Verificação manual Omarchy 4 (3 provas).

## §9 Riscos

| Risco | Mitigação |
| --- | --- |
| Widget.qml cresce muito | Components no mesmo arquivo; split se >~800 linhas de lógica nova |
| Flash de chips | Sem filtro até primeiro show |
| `remove` “sumiu” do help | Alias mantém o parse; help aponta `uninstall` |
| displayMode sem efeito | Chip label (§3) |
| User misto Waybar+Omarchy | apply não toca Waybar modules; documentado |

## Referências

- Plugin atual: `assets/omarchy/Widget.qml`, `docs/omarchy-shell.md`
- Spec plugin v1: `docs/superpowers/specs/2026-07-21-omarchy-shell-plugin-design.md`
- Settings: `src/settings.rs`; TUI Config: `src/tui/render/config.rs` (intocada)
- model-usage: `/usr/share/omarchy/shell/plugins/model-usage/Widget.qml`
- JSON quota: `docs/json-output.md` (inalterado)
