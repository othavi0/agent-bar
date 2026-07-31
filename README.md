# Agent Bar

Agent Bar is an Omarchy Quattro Quickshell plugin that shows normalized quota
and reset information for Claude, Codex, Amp, and Grok.

The product is one plugin, `agent-bar.usage`. Its Quickshell UI contains compact
provider chips, a consolidated popup, Settings, connection actions, update,
and uninstall. A private Rust helper ships inside the plugin for provider
collection, cache, settings, and safe maintenance.

## What v10 shows

- One bar chip per enabled provider.
- Used or remaining percentage.
- Plan tag and typed provider states.
- Normalized quota windows and reset times.
- Loading, stale, missing CLI, unauthenticated, rate-limit, network, and
  provider-error states.
- A safe action when login, installation guidance, or retry is available.

v10 does not show session history, charts, token costs, currency, provider
spend, balances, or credits. When a connected account exposes no percentage
window, the chip shows `—`.

## Interaction

| Action | Result |
| --- | --- |
| Left click | Open that provider; click it again to close |
| Middle click | Force one refresh of all enabled providers |
| Right click | Open Settings |
| Mouse wheel on chip | No action |

The popup has a vertical icon rail, one provider view at a time, one lead
percentage window with every other window as a compact row, a usage track on
every row, content-fit height, overflow-only scrolling, complete keyboard
navigation, and active Omarchy theme tokens.

## Requirements

- Omarchy Quattro with Quickshell.
- Linux x86_64 using the GNU target.
- The provider CLIs or local provider data you want to monitor.
- `curl`, `tar`, `zstd`, and `sha256sum` for the release bootstrap.
- Omarchy's `xdg-terminal-exec` route for interactive provider login.

Agent Bar never installs provider CLIs and never handles credentials.

## Installation

Installation uses the plugin-scoped bootstrap from a published release tag:

```bash
curl -fsSLO https://raw.githubusercontent.com/othavi0/agent-bar/v10.0.0/install.sh
less install.sh
bash install.sh
```

The bootstrap installs one verified directory:

```text
~/.config/omarchy/plugins/agent-bar.usage/
```

It does not install a global executable or package. Normal update and uninstall
actions live in the plugin Settings UI.

## Settings

Settings are stored at:

```text
$XDG_CONFIG_HOME/agent-bar/settings.json
```

When `XDG_CONFIG_HOME` is unset, the default is
`$HOME/.config/agent-bar/settings.json`.

Fresh defaults:

```text
Providers: Claude, Codex, Amp, Grok
Order: Claude, Codex, Amp, Grok
Display: remaining
Refresh: 60 seconds
Notifications: enabled
```

Settings supports provider enablement/order, used versus remaining, refresh
interval, and one notification toggle.

## Private helper

The bundled helper lives at:

```text
~/.config/omarchy/plugins/agent-bar.usage/bin/agent-bar
```

Quickshell uses its strict word-based CLI and JSON schema v2. Users normally do
not need it. Diagnostic examples:

```bash
PLUGIN="$HOME/.config/omarchy/plugins/agent-bar.usage/bin/agent-bar"
"$PLUGIN" status format human
"$PLUGIN" status provider claude format json cache bypass
"$PLUGIN" doctor scan
```

See [docs/commands.md](docs/commands.md) for the recovery contract.

## Development

The repository uses Rust/Cargo and QML. No Node runtime or test toolchain is
used.

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

See [CONTRIBUTING.md](CONTRIBUTING.md) and
[docs/specs/v10/README.md](docs/specs/v10/README.md).

## Documentation

- [Product](PRODUCT.md)
- [Architecture](docs/architecture.md)
- [Commands](docs/commands.md)
- [Runtime](docs/runtime.md)
- [Omarchy integration](docs/omarchy-shell.md)
- [Troubleshooting](docs/troubleshooting.md)
- [Canonical v10 specification](docs/specs/v10/README.md)

## License

MIT. See [LICENSE](LICENSE).
