# Changelog

All notable changes to qbar will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

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
