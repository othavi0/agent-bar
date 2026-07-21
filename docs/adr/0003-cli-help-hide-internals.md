# Help público enxuto; internos ainda parseáveis

A CLI misturava primários com paths de packager/Waybar (`assets`, `export`,
`action-right`) e aliases redundantes. Decidimos **opção A**: reorganizar o
help (menu, status, config, setup, update, uninstall, doctor); esconder
internos do help mas **manter o parse** (módulos Waybar e scripts dependem);
`remove` → `uninstall --yes`; `-t` → `status`. Sem hard-cut Omarchy-only nem
deletar `action-right`.

**Considered:** (B) unificar action-right em `menu --provider`; (C) default
JSON e Waybar explícito. Deferidos para não quebrar contrato gerado.

**Status:** accepted (v8.5.0)
