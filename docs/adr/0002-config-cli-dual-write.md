# settings.json via CLI; interval in shell.json

Settings mode needs providers/order/display/notify (Rust logic) and
`refreshIntervalSec` (Omarchy plugin schema). We decided on a **single Rust
source** for the editable subset: `agent-bar config show` / `config apply`
read and write `~/.config/agent-bar/settings.json` with the existing
normalization. `refreshIntervalSec` stays in the plugin entry
(`updateEntryInline` → `shell.json`). QML's save performs a dual-write in
that order; apply does **not** call `apply_waybar_integration`.

**Considered:** everything in `shell.json`; QML writing `settings.json`
directly. Rejected: a second source of truth and bypassing normalization.

**Status:** accepted (v8.5.0)
