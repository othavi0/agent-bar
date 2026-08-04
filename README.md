# Agent Bar

Agent Bar is an Omarchy Quattro Quickshell plugin that shows normalized
quota and reset information for Claude, Codex, Amp, and Grok.

The product is one plugin, `agent-bar.usage`. Its Quickshell UI contains
compact provider chips, a consolidated popup, Settings, connection actions,
update, and uninstall. A private Rust helper ships inside the plugin for
provider collection, cache, settings, and safe maintenance.

## What it shows

- One bar chip per enabled provider: icon and used or remaining percentage.
- A hover tooltip: provider name, percentage, and state on the first line;
  the active window's label with its reset countdown and local clock time
  on the second, refreshed at hover time.
- Severity cues on the chip: `!` when a ready provider crosses the critical
  threshold, an hourglass glyph when data is stale.
- Plan tag and typed provider states.
- Normalized quota windows and reset times; the lead window shows both the
  countdown and the wall-clock reset.
- Loading, stale, missing CLI, unauthenticated, rate-limit, network, and
  provider-error states.
- A safe action when login, installation guidance, or retry is available.

Agent Bar does not show session history, charts, token costs, currency,
provider spend, balances, or credits. When a connected account exposes no
percentage window, the chip shows `—`.

## Interaction

| Action | Result |
| --- | --- |
| Hover a chip | Tooltip with state and the active window's reset |
| Left click | Open that provider; click it again to close |
| Middle click | Force one refresh of all enabled providers |
| Right click | Open Settings |
| Mouse wheel on chip | No action |

While the popup is open, clicking outside it on any monitor dismisses it;
clicks landing on another monitor's chips still reach those chips.

The popup has a vertical icon rail, one provider view at a time, one lead
percentage window with every other window as a compact row, a usage track
on every row, content-fit height, overflow-only scrolling, complete
keyboard navigation, and active Omarchy theme tokens.

## Requirements

- Omarchy Quattro with Quickshell.
- Linux x86_64 using the GNU target.
- The provider CLIs or local provider data you want to monitor.
- `curl`, `tar`, `zstd`, and `sha256sum` for the release bootstrap.
- Omarchy's `xdg-terminal-exec` route for interactive provider login.

Agent Bar never installs provider CLIs and never handles credentials.

## Installation

The bootstrap installs the latest published release:

```bash
curl -fsSLO https://raw.githubusercontent.com/othavi0/agent-bar/master/install.sh
less install.sh
bash install.sh
```

Pin a specific version with `AGENT_BAR_VERSION=10.3.0 bash install.sh`.

The bootstrap installs one verified directory:

```text
~/.config/omarchy/plugins/agent-bar.usage/
```

It does not install a global executable or package. Update and uninstall
live in the plugin Settings UI; a release is cut automatically from every
product merge, so the Settings update check always offers the latest.

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
interval, and one notification toggle. Persisted settings apply from
service start.

## Private helper

The bundled helper lives at:

```text
~/.config/omarchy/plugins/agent-bar.usage/bin/agent-bar
```

Quickshell uses its strict word-based CLI and JSON schema v2. Users
normally do not need it. Diagnostic examples:

```bash
PLUGIN="$HOME/.config/omarchy/plugins/agent-bar.usage/bin/agent-bar"
"$PLUGIN" status format human
"$PLUGIN" status provider claude format json cache bypass
"$PLUGIN" doctor scan
```

See [docs/guide/commands.md](docs/guide/commands.md) for the recovery
contract.

## Development

The repository uses Rust/Cargo and QML. No Node runtime or test toolchain
is used.

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

QML verification requires the Qt6 binaries — the bare PATH tools on Arch
are silent stubs. See [CONTRIBUTING.md](CONTRIBUTING.md) for the exact
commands.

## Documentation

- [Product](PRODUCT.md)
- [Documentation index](docs/README.md)
- [Architecture](docs/dev/architecture.md)
- [Helper commands](docs/guide/commands.md)
- [Runtime](docs/guide/runtime.md)
- [Troubleshooting](docs/guide/troubleshooting.md)
- [Canonical v10 specification](docs/specs/v10/README.md)

## License

MIT. See [LICENSE](LICENSE).
