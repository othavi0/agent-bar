# agent-bar

Monitor de quotas LLM para a barra do desktop (Waybar e Omarchy 4 /
omarchy-shell). O binário Rust é a fonte de dados; a barra só apresenta.

## Language

### Surfaces

**Bar:**
A barra do desktop onde o usuário lê quota de relance. No Omarchy 4 é o
omarchy-shell (Quickshell); no path clássico é Waybar.
_Avoid_: status bar genérico, panel (salvo no sentido de PopupCard)

**Chip:**
Indicador compacto de um provider na Bar (ícone + percentual).
_Avoid_: module (reservado ao contrato Waybar `custom/agent-bar-*`)

**Usage popup:**
Painel nativo do omarchy-shell que mostra quotas e breakdown sem abrir
terminal. Abre no clique esquerdo do Chip.
_Avoid_: tooltip (é o equivalente rico do tooltip Waybar, mas é popup QML)

**Settings mode:**
Mesmo popup em modo de edição de configuração (clique direito no Omarchy).
Não é a TUI.
_Avoid_: settings screen, preferences window

**TUI / Menu:**
Dashboard completo em terminal (`agent-bar menu`): Detail, History, Login,
Config. Abre por comando ou pelo link “Abrir menu (TUI)” no popup.
_Avoid_: abrir a TUI no right-click Omarchy (comportamento pré-8.5.0)

### Config

**Editable settings (subset Omarchy):**
Campos que o Settings mode edita: providers habilitados, ordem, displayMode
(remaining/used), notify.enabled. Persistidos em `settings.json` via
`config apply`.
_Avoid_: “settings do shell” para esses campos

**Plugin setting:**
Config do widget no omarchy-shell (`refreshIntervalSec` e schema do
manifest). Vive no entry do plugin em `shell.json` via `updateEntryInline`.
_Avoid_: misturar com Editable settings

**Dual-write:**
No Save do Settings mode: primeiro `config apply` (settings.json); se OK,
depois `updateEntryInline` (intervalo). Ordem fixa para não gravar interval
quando o apply falha.

**Provider list canônica:**
`waybar.providers` / `waybar.provider_order` em `settings.json` — mesmo
conjunto que a TUI Config e o Waybar. No Omarchy o QML filtra chips por essa
lista; o envelope `--format json` continua completo.
_Avoid_: lista de providers só no shell.json

### Integration

**Drop-in plugin:**
Arquivos do widget escritos em
`~/.config/omarchy/plugins/agent-bar.usage/` pelo setup, embutidos no binário.
_Avoid_: `omarchy plugin add` de git como path principal

**action-right:**
Comando interno do right-click **Waybar** (abre TUI focada). Fora do help
público; o Omarchy não o usa no right-click.
_Avoid_: chamar de “menu” ou “settings”
