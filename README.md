![Agent Bar popup open over the Omarchy desktop](docs/media/demo.png)

# Agent Bar

An Omarchy plugin that puts your AI quota in the bar. One chip per
provider, a popup with every quota window, and reset countdowns so you
know when a limit clears. It covers Claude, Codex, Amp, and Grok.

## What you see

Each enabled provider gets a compact chip with its icon and percentage,
shown as used or remaining, your pick. Hover a chip for the active
window and its reset time. Click it and the popup opens: plan tag (like
`MAX 20X`), a lead window with both the countdown and the wall-clock
reset, and every other window as a row with its own usage track.

Windows are normalized across providers, so Claude's `Session (5h)`,
`Weekly (7d)`, and per-model windows render the same way Codex's do.
A chip shows `!` when a provider crosses the critical threshold and an
hourglass when its data is stale. A connected provider with no
percentage window is valid and shows `—`.

When a provider needs attention, the popup offers a safe action:
opening the login in a terminal, installation guidance, or a retry.
Everything follows your active Omarchy theme, and the popup is fully
keyboard-navigable.

| Action | Result |
| --- | --- |
| Hover a chip | Active window and its reset |
| Left click | Open the popup; click again to close |
| Middle click | Force a refresh of all providers |
| Right click | Open Settings |

Agent Bar reads the local data your provider CLIs already maintain. It
never installs a CLI and never handles credentials. It also shows no
token costs, spend, or session history; percentages and resets only.

## Install

You need Omarchy with Quickshell (Quattro), Linux x86_64, and the
provider CLIs or local provider data you want to monitor. The
bootstrap uses `curl`, `tar`, `zstd`, and `sha256sum`.

```bash
curl -fsSLO https://raw.githubusercontent.com/othavi0/agent-bar/master/install.sh
less install.sh
bash install.sh
```

That installs one verified directory and nothing else:

```text
~/.config/omarchy/plugins/agent-bar.usage/
```

Pin a version with `AGENT_BAR_VERSION=10.3.0 bash install.sh`.

## Settings

Right click any chip. From there you can enable, disable, and reorder
providers, switch between used and remaining, set the refresh interval
(default 60 seconds), and toggle notifications. Settings live at
`~/.config/agent-bar/settings.json`.

Update and uninstall live in Settings too. A release is cut from every
product merge, so the update check always offers the latest version.

## More

- [Troubleshooting](docs/guide/troubleshooting.md)
- [Documentation index](docs/README.md)
- [Architecture](docs/dev/architecture.md)
- [Contributing](CONTRIBUTING.md)

MIT. See [LICENSE](LICENSE).
