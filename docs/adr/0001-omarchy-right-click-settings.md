# Right-click Omarchy = settings nativo (não TUI)

No Omarchy 4 o right-click do chip abria a TUI inteira num terminal flutuante,
enquanto o first-party `omarchy.model-usage` usa o mesmo popup em modo settings.
Decidimos alinhar: **só no plugin Omarchy**, right-click abre Settings mode no
mesmo `PopupCard`; left = usage; middle = refresh. Waybar mantém left=menu e
right=`action-right`. A TUI continua em `agent-bar menu` e no link do popup.

**Considered:** (B) mudar Waybar junto; (C) matar/encolher a TUI no mesmo
release. Rejeitados: escopo e risco sem valor no desktop Omarchy-first.

**Status:** accepted (v8.5.0)
