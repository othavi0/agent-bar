# Right-click Omarchy = native settings (not TUI)

On Omarchy 4 the chip's right-click opened the entire TUI in a floating
terminal, while the first-party `omarchy.model-usage` uses the same popup in
settings mode. We decided to align: **Omarchy plugin only**, right-click opens
Settings mode in the same `PopupCard`; left = usage; middle = refresh. Waybar
keeps left=menu and right=`action-right`. The TUI remains available via
`agent-bar menu` and the popup's link.

**Considered:** (B) change Waybar too; (C) kill/shrink the TUI in the same
release. Rejected: scope and risk with no value on the Omarchy-first desktop.

**Status:** accepted (v8.5.0)
