# Commands

Public CLI surface grouped the way `--help` presents it. Internal commands
remain parseable (Waybar modules, terminal helper, packagers) but are hidden
from the human help listing.

## Usage

| Command | Purpose | Writes |
| --- | --- | --- |
| `agent-bar` | Print Waybar JSON for enabled providers. | Cache only when providers fetch fresh data |
| `agent-bar status` | Print the quota view in a terminal. | Cache only |
| `agent-bar menu` | Interactive TUI: first enabled provider detail, hourly/daily history, provider login, and the Config editor. | Settings and provider auth as requested |
| `agent-bar config show` | Print the editable settings subset as JSON. | Nothing |
| `agent-bar config apply` | Apply a JSON patch to the editable settings subset. | `~/.config/agent-bar/settings.json` |

## `config show` / `config apply`

Bridge for the Omarchy settings popup (and scripts). Schema of **this**
command is independent of the quota JSON envelope (`schemaVersion` here is
the config subset, currently `1`).

```bash
agent-bar config show
agent-bar config apply --json '{"schemaVersion":1,"providers":["claude","codex"]}'
agent-bar config apply --json -          # read blob from stdin
agent-bar config apply --file <path>     # packager / QML / tests
```

Stdout is JSON only (logs on stderr). Success stdout of `apply` is the same
envelope as `show` (post-save state). Exit: `0` ok · `1` validation, usage
(missing args), or IO — same as the rest of the CLI.

Example envelope:

```json
{
  "schemaVersion": 1,
  "providers": ["claude", "codex", "amp", "grok"],
  "providerOrder": ["claude", "codex", "amp", "grok"],
  "displayMode": "remaining",
  "notify": { "enabled": true }
}
```

| Field | Notes |
| --- | --- |
| `schemaVersion` | Required on apply; must be `1`. |
| `providers` | Enabled ids after normalize; apply with empty/unknown-only → error. |
| `providerOrder` | Effective order; unknown ids dropped. |
| `displayMode` | `"remaining"` or `"used"`. |
| `notify.enabled` | Desktop low/critical notifications. |

Omitted fields on apply are left unchanged. Not included: separators, Waybar
signal/interval, menu font, cache, glyphs, `refreshIntervalSec` (Omarchy
plugin setting — see [omarchy-shell.md](omarchy-shell.md)).

**Side-effects of apply:** writes `settings.json` only. Does **not** reload
Waybar, re-patch modules, or invalidate the quota cache.

## Setup

| Command | Purpose | Writes |
| --- | --- | --- |
| `agent-bar setup` | Install assets, create symlink, patch Waybar and/or Omarchy plugin, reload. | `~/.local/bin`, `~/.config/waybar`, Omarchy plugins dir |
| `agent-bar update` | Update the install: managed `~/.agent-bar` checkout, or defers to system package manager. | `~/.agent-bar`, managed Waybar/Omarchy files |
| `agent-bar uninstall` | Interactive removal of managed integration and owned files. | Removes managed files/entries |
| `agent-bar doctor` | Detect and clean leftover agent-bar artifacts in `$HOME` from previous installs. | Optional cleanup under `$HOME` |

```bash
agent-bar doctor              # interactive
agent-bar doctor --dry-run    # report without removing
agent-bar doctor --yes        # non-interactive, remove without prompting
```

`remove` is a quiet alias of `uninstall --yes` (forced, non-interactive). It
is not listed in `--help`; the primary removal command is `uninstall`.

## Machine (JSON / flags)

For Quickshell, Eww, Ironbar, or any consumer that renders natively and wants
raw structured data instead of the Waybar Pango envelope.

| Command | Purpose | Writes |
| --- | --- | --- |
| `agent-bar --format json` | One-shot versioned JSON envelope for all registered providers (Pango-free). | Cache only |
| `agent-bar --format json --provider <id>` | Same, single provider. | Cache only |
| `agent-bar --watch [--interval <s>]` | Stream NDJSON (one envelope per line) until killed. | Cache only |

The schema, stability policy, and a Quickshell QML example live in
[`docs/json-output.md`](json-output.md). Unlike the Waybar path, JSON output is
**not** filtered by the `waybar.providers` setting — it emits all registered
providers and the consumer decides what to show (Omarchy QML filters chips
using `config show`).

### Flags

| Flag | Purpose |
| --- | --- |
| `-p`, `--provider <id>` | Limit output to `claude`, `codex`, `amp`, or `grok`. |
| `-r`, `--refresh` | Ignore cache and fetch fresh provider data. |
| `-t`, `--terminal` | Alias of `status` (terminal quota view). |
| `--format <waybar\|json>` | Output format. Default `waybar`. `json` emits the versioned contract. |
| `--watch` | Stream NDJSON (one envelope per line); implies `--format json`. |
| `--interval <seconds>` | Watch poll floor (default 60). Only meaningful with `--watch`. |
| `-v`, `--verbose` | Enable diagnostic logging (stderr). |
| `-V`, `--version` | Print version and exit. |
| `-h`, `--help` | Print CLI help. |

## Internal (hidden from `--help`)

Still parseable for Waybar modules, the terminal helper, tests, and
packagers. Prefer the public commands above for interactive use.

### Waybar-triggered

Generated Waybar modules wire these to click actions (see
[waybar-contract.md](waybar-contract.md)). Omarchy uses native popup
settings instead of `action-right`.

| Command | Trigger | Behavior |
| --- | --- | --- |
| `agent-bar menu` | Left click (Waybar) | Interactive TUI (also a primary command). |
| `agent-bar action-right <provider>` | Right click (Waybar) | Opens the TUI already focused on one provider. Requires a provider arg. |

`action-right` resolves whether the clicked provider looks connected, then opens
the interactive TUI already focused there:

- **Disconnected** — no credentials, or a quota error matching the base pattern
  (`expired` / `not logged in` / `login again` / `please login`); for Codex,
  additionally `no session data` / `no rate limit data` / `auth` / `token` →
  boots straight into the Login screen with that provider preselected.
- **Connected** — boots straight into that provider's detail view.

It requires a provider argument — the CLI parser exits non-zero without one.

### Helper / packaging

| Command | Purpose |
| --- | --- |
| `agent-bar menu-font` | Prints `{fontFamily}\t{fontSize}` from `settings.menu` for `scripts/agent-bar-open-terminal`. |
| `agent-bar assets install --waybar-dir <path> --scripts-dir <path>` | Copy icons and terminal helper into explicit paths. |
| `agent-bar export waybar-modules --app-bin <path> --terminal-script <path>` | Print generated Waybar module JSON. |
| `agent-bar export waybar-css --icons-dir <path>` | Print generated Waybar CSS JSON. |

## `menu` Navigation

The TUI boots straight into the first enabled provider's detail view. The left
sidebar lists one entry per provider, then History, Login, and Config, next to
a right-hand content pane.

- **Keyboard:** `up`/`down` (or `j`/`k`) move the sidebar cursor, `Enter`
  activates the selected item, `h`/`g`/`w` jump directly to History/Login/
  Config from anywhere, `r` refreshes quotas, `Esc` backs out of a detail
  view, `?` toggles a help overlay listing every binding per screen, `q`
  quits.
- **Mouse:** click selects (sidebar rows, provider cards, chips), the wheel
  scrolls, and `shift`+drag selects terminal text as usual (mouse capture is
  disabled for that gesture).

On Omarchy, daily use is the native popup; open the dashboard with
`agent-bar menu` or the **Abrir menu (TUI)** link in the usage footer.

## Update Behavior

`agent-bar update` detects the install type and updates accordingly:

- **Managed `~/.agent-bar` checkout (install.sh path):** fetches upstream, shows
  incoming commits and local changes, and after confirmation runs
  `git reset --hard <upstream>` + `git clean -fd`, and re-applies setup.
- **Standalone binary** (`~/.local/bin/agent-bar` from GitHub Release / install.sh
  tarball path): downloads the latest release, replaces the binary, refreshes
  assets under the data dir **and** re-copies provider icons + terminal helper
  into the Waybar paths (`~/.config/waybar/agent-bar/icons`,
  `~/.config/waybar/scripts`). Does **not** re-patch Waybar `config.jsonc` /
  modules / CSS — use `agent-bar setup` when integration itself changed.
- **System install (AUR `-bin`):** does not self-update — directs you to your
  package manager (e.g. `paru -Syu agent-bar-bin`).
- **Development checkout:** refuses and tells you to update with `git pull`.
