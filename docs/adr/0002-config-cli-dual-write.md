# settings.json via CLI; intervalo no shell.json

O Settings mode precisa de providers/order/display/notify (lógica Rust) e de
`refreshIntervalSec` (schema do plugin Omarchy). Decidimos **fonte única Rust**
para o subset editável: `agent-bar config show` / `config apply` leem e gravam
`~/.config/agent-bar/settings.json` com a normalização existente.
`refreshIntervalSec` fica no entry do plugin (`updateEntryInline` →
`shell.json`). Save do QML faz dual-write nessa ordem; apply **não** chama
`apply_waybar_integration`.

**Considered:** tudo no `shell.json`; QML escrever `settings.json` direto.
Rejeitados: segunda fonte de verdade e bypass de normalização.

**Status:** accepted (v8.5.0)
