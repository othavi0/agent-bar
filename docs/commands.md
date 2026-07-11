# Commands

## Primary Commands

| Command | Purpose | Writes |
| --- | --- | --- |
| `agent-bar` | Print Waybar JSON for enabled providers. | Cache only when providers fetch fresh data |
| `agent-bar status` | Print the quota view in a terminal. | Cache only |
| `agent-bar menu` | Interactive TUI: boots on the first enabled provider's detail view, plus hourly/daily history, provider login, and the Config editor. | Settings and provider auth as requested |
| `agent-bar update` | Update the install: managed `~/.agent-bar` checkout, or defers to system package manager. | `~/.agent-bar`, managed Waybar files |

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
- **`menu-font`:** hidden internal command (`agent-bar menu-font`, not shown
  in `--help`) that prints `{fontFamily}\t{fontSize}` from
  `settings.menu` for `scripts/agent-bar-open-terminal` to pass through to
  terminals that support a font flag when launching the menu.

## Internal Commands (Waybar-Triggered)

You normally don't type these — generated Waybar modules wire them to click
actions (see [waybar-contract.md](waybar-contract.md)).

| Command | Trigger | Behavior |
| --- | --- | --- |
| `agent-bar menu` | Left click | Interactive TUI (also a primary command). |
| `agent-bar action-right <provider>` | Right click | Opens the TUI already focused on one provider. Requires a provider arg. |

`action-right` resolves whether the clicked provider looks connected, then opens
the interactive TUI already focused there:

- **Disconnected** — no credentials, or a quota error matching the base pattern
  (`expired` / `not logged in` / `login again` / `please login`); for Codex,
  additionally `no session data` / `no rate limit data` / `auth` / `token` →
  boots straight into the Login screen with that provider preselected.
- **Connected** — boots straight into that provider's detail view.

It requires a provider argument — the CLI parser exits non-zero without one.

## JSON Output (non-Waybar bars)

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
providers and the consumer decides what to show.

## Install And Removal

| Command | Purpose | Writes |
| --- | --- | --- |
| `agent-bar setup` | Install assets, create symlink, patch Waybar, reload Waybar. | `~/.local/bin`, `~/.config/waybar` |
| `agent-bar uninstall` | Interactive removal of managed integration and owned files. | Removes managed files/entries |
| `agent-bar remove` | Non-interactive forced removal. | Same targets as uninstall |

## `doctor`

Detect and clean leftover agent-bar artifacts in `$HOME` from previous installs.

```bash
agent-bar doctor              # interactive
agent-bar doctor --dry-run    # report without removing
agent-bar doctor --yes        # non-interactive, remove without prompting
```

## Export And Assets

These are mostly for tests, packagers, and manual integration.

| Command | Purpose |
| --- | --- |
| `agent-bar assets install --waybar-dir <path> --scripts-dir <path>` | Copy icons and terminal helper into explicit paths. |
| `agent-bar export waybar-modules --app-bin <path> --terminal-script <path>` | Print generated Waybar module JSON. |
| `agent-bar export waybar-css --icons-dir <path>` | Print generated Waybar CSS JSON. |

## Flags

| Flag | Purpose |
| --- | --- |
| `-p`, `--provider <id>` | Limit output to `claude`, `codex`, or `amp`. |
| `-r`, `--refresh` | Ignore cache and fetch fresh provider data. |
| `-t`, `--terminal` | Force terminal output mode. |
| `--format <waybar\|json>` | Output format. Default `waybar`. `json` emits the versioned contract (see below). |
| `--watch` | Stream NDJSON (one envelope per line); implies `--format json`. |
| `--interval <seconds>` | Watch poll floor (default 60). Only meaningful with `--watch`. |
| `-v`, `--verbose` | Enable diagnostic logging (stderr). |
| `-V`, `--version` | Print version and exit. |
| `-h`, `--help` | Print CLI help. |

## Update Behavior

`agent-bar update` detects the install type and updates accordingly:

- **Managed `~/.agent-bar` checkout (install.sh path):** fetches upstream, shows
  incoming commits and local changes, and after confirmation runs
  `git reset --hard <upstream>` + `git clean -fd`, and re-applies setup.
- **System install (AUR `-bin`, standalone binary):** does not self-update —
  directs you to your package manager (e.g. `paru -Syu agent-bar-bin`).
- **Development checkout:** refuses and tells you to update with `git pull`.
