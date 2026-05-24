# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- `agent-bar doctor` command: detects and cleans `@noctuacore/agent-bar`
  leftovers (`package.json`, lockfiles, `node_modules/@noctuacore/`) in `$HOME`
  caused by `bun add` / `npm i` without `-g`.
- `setup` now warns when `$HOME` has leftover install artifacts and points to
  `agent-bar doctor`.
- Bin shim (`scripts/agent-bar`) now detects install pollution in `$HOME` on
  every invocation and prints a warning suggesting `agent-bar doctor`. Warns at
  most once per hour per UID (cached in `$XDG_RUNTIME_DIR`) so Waybar logs stay
  clean.
- `install.sh` hosted installer: zero-pollution install path via
  `curl -fsSL .../install.sh | bash`. Clones to `~/.agent-bar`, installs deps,
  and optionally runs `agent-bar setup`. Adopts the curl|bash pattern used by
  bun, deno, rustup, uv, and other serious CLI tools.

### Changed
- README now promotes the hosted install script as the primary install path.
  `bun add -g` remains documented as an alternative with explicit warning about
  the `-g` flag.
- Documentation refresh: `CONTRIBUTING.md` rewritten in English and trimmed,
  with a new "Dev install" section explaining how to wire a local checkout
  straight into Waybar. `docs/runtime.md`, `docs/integration.md`,
  `docs/commands.md`, and `docs/troubleshooting.md` updated to drop the
  outdated "legacy" label on `~/.agent-bar`, reflect `install.sh` as the
  primary install path, and document `$HOME` pollution handling.

### Removed
- `preinstall` script from `package.json` — Bun does not execute lifecycle
  scripts of dependencies by default, so the guard was silent theater. Replaced
  by a Bash-level detector in the bin shim that runs on every invocation
  regardless of package manager.

## [4.0.2] - 2026-05-19

### Changed

- `agent-bar update` agora detecta instalações npm/Bun e atualiza o pacote
  global com `bun add -g`, em vez de tratar apenas o checkout legado
  `~/.agent-bar`.

### Fixed

- Logo da TUI exibia `QBAR` (nome antigo do projeto) ao abrir o menu. Substituído pela block-art `AGENT BAR`.

## [4.0.0] - 2026-05-15

### Added

- Setting `waybar.displayMode` (`remaining` | `used`) com toggle via TUI Configure Layout. Quando `used`, percentuais e barra refletem quota consumida (0% = nada usado, 100% = esgotado); cores e classes CSS continuam baseadas em saúde. Default: `remaining` (comportamento anterior preservado).

### Changed

- Renamed the project to `agent-bar` (previously `qbar`, then `agent-bar-omarchy`). Runtime state now lives under `~/.config/agent-bar` and `~/.cache/agent-bar`; Waybar module IDs use the `agent-bar` namespace.

### Removed

- Removed the `qbar` and `agent-bar-omarchy` compatibility layer entirely:
  legacy identity constants, settings/cache path migration, Waybar legacy-asset
  cleanup, the `agent-bar-omarchy` CLI symlink and `bin` alias, and the `snippets/`
  manual examples.

### Breaking

- The `agent-bar-omarchy` command no longer exists. Installations still using the
  old name must reinstall as `agent-bar`; old settings/cache under the previous
  names are not migrated.

## [3.0.0] - 2026-03-27

### Added

- Amp provider with free/credits monitoring and SVG icon
- Interactive Waybar layout configuration via `qbar setup`
- Per-provider model selection with `Configure Models`
- Window policies for quota display (both, five_hour, seven_day)
- Settings schema versioning with validation and atomic writes
- Bun dependency check at startup
- Cache management improvements with configurable TTL (5 min default)
- Codex app-server integration with dynamic window labels
- Auto-activate provider in Waybar after login
- Right-click action shows full provider info

### Changed

- Removed Antigravity provider in favor of direct Claude/Codex/Amp integration
- Streamlined cache invalidation across providers
- Updated Waybar integration to flat-onedark theme
- Improved CLI help output with better formatting
- Simplified provider integration architecture

### Fixed

- Waybar module rendering and provider toggle behavior
- Amp icon display and tooltip tree connectors
- Cache invalidation now properly deletes stale entries
- Action-right routing for provider-specific actions

## [2.0.0] - 2026-02-09

### Added

- Complete TypeScript rewrite with Bun runtime
- Interactive TUI menu with clack/prompts
- Provider architecture: Claude, Codex, Antigravity as pluggable modules
- `qbar setup` for automated Waybar configuration (config.jsonc + style.css)
- `qbar uninstall` to cleanly remove all integration files
- `qbar update` command for self-update
- Beautiful `--help` UI matching hover/status style
- Smart context detection: shows help in interactive terminal, JSON in Waybar
- Extra Usage support with timeline visualization
- Separate Waybar modules per provider with PNG icons via CSS
- Rich Catppuccin-themed tooltips with model grouping
- Provider login/logout flows with automatic Waybar refresh
- Antigravity native OAuth login and token auto-refresh
- Per-module visual separators (pill, gap, bare, glass, shadow, none)
- Ora spinner for refresh actions
- Disconnected state indicator with red icon

### Changed

- Renamed project from llm-usage to qbar
- Cache directory moved to `~/.cache/qbar/`
- Tooltip layout redesigned with box drawing characters
- Terminal output now matches hover/tooltip style
- Waybar interval set to 2 minutes

### Fixed

- Tooltip newline handling and JSON escaping
- Cache invalidation deletes file instead of writing empty object
- Null remainingFraction treated as 0% (exhausted)
- Login terminal stays open during OAuth flows
- Antigravity percentages normalization and tier grouping
- Bar rendering when filled/empty segments are zero
- Bun PATH resolution in Waybar environment

## [1.0.0] - 2026-02-04

### Added

- Initial release as Waybar LLM usage monitor
- Claude and Codex quota monitoring via shell scripts
- Antigravity cloud fallback helper scripts
- Right-click menu for login and refresh actions
- Waybar tooltip with usage bars and reset times
- Provider visibility toggling (hide when logged out)
- Logout submenu with per-provider cache cleanup
- Auto-refresh Waybar after login/logout actions
- Monospace tooltip formatting with Pango markup
- Documentation in English and PT-BR
