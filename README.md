# Agent Bar

Agent Bar is an Omarchy plugin that puts your AI provider quota in the
Quickshell bar. It tracks Claude, Codex, Amp, Grok, and Antigravity: one chip per
provider, a popup with every usage window, and a countdown to the next
reset.

![Agent Bar preview](preview.png)

## What you see

Each enabled provider gets a compact chip with its icon and percentage,
shown as used or remaining, your pick. Click it and the popup opens: plan
tag (like `MAX 20X`), a lead window with both the countdown and the
wall-clock reset, and every other window as a row with its own usage
track.

Windows are normalized across providers, so Claude's `Session (5h)`,
`Weekly (7d)`, and per-model windows render the same way Codex's do. A
chip shows `!` when a provider crosses the critical threshold. When a
refresh fails, the last good reading stays on the bar unchanged — the
popup notes when it was taken and nothing is marked as broken. A
connected provider with no percentage window is valid and shows `—`.

When a provider needs attention, the popup offers a safe action: opening
the login in a terminal, installation guidance, or a retry. Everything
follows your active Omarchy theme, and the popup is fully
keyboard-navigable.

| Action | Result |
| --- | --- |
| Left click | Open the popup; click again to close |
| Middle click | Force a refresh of all providers |
| Right click | Open Settings |

Agent Bar reads the local data your provider CLIs already maintain. It
never installs a CLI and never handles credentials. It also shows no
token costs, spend, or session history; percentages and resets only.

## Install

You need Omarchy with Quickshell (Quattro), Linux x86_64, `git`, and the
provider CLIs or local provider data you want to monitor. `git` is
already an Omarchy base tool, so there is nothing extra to install.

```bash
omarchy plugin add https://github.com/othavi0/omarchy-agent-bar.git
```

Omarchy asks where to place the bar widget when you enable it. If you
skip that choice, the widget defaults to the right section of the bar.
This clones one verified directory and nothing else:

```text
~/.config/omarchy/plugins/othavi0.agent-bar/
```

## Update

```bash
omarchy plugin update othavi0.agent-bar
```

If you installed Agent Bar before this release (a plain directory, not a
git checkout), the update button shows a one-time migration notice. Run:

```bash
omarchy plugin remove othavi0.agent-bar
omarchy plugin add https://github.com/othavi0/omarchy-agent-bar.git
```

Your settings, cache, and backups live outside the plugin directory and
survive the swap.

## Remove

```bash
omarchy plugin remove othavi0.agent-bar
```

Update and remove are also available as buttons in Settings.

## Settings

Right click any chip. From there you can enable, disable, and reorder
providers, switch between used and remaining, set the refresh interval
(default 60 seconds), and toggle notifications. Settings live at
`~/.config/agent-bar/settings.json`.

Update and uninstall live in Settings too. A release is cut from every
product merge, so the update check always offers the latest version.

## Development

This repository is both the plugin (root tree) and its source. See
[Architecture](docs/dev/architecture.md), [Releasing](docs/dev/releasing.md),
and [Contributing](CONTRIBUTING.md) for the build, test, and release
pipeline.

`bin/agent-bar` and `bundle.json` are release artifacts committed by CI —
never edit them by hand.

## More

- [Troubleshooting](docs/guide/troubleshooting.md)
- [Documentation index](docs/README.md)
- [Architecture](docs/dev/architecture.md)
- [Contributing](CONTRIBUTING.md)

## License

MIT. See [LICENSE](LICENSE).
