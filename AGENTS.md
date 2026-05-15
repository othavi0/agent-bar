# agent-bar-omarchy — Agent Instructions

LLM quota monitor for Waybar. Tracks Claude, Codex, and Amp usage, exports Waybar modules/CSS, owns its runtime state, and provides an interactive TUI for provider login and layout/model configuration.

These instructions are the canonical guidance for coding agents in this repo. `CLAUDE.md` intentionally delegates here to avoid duplicated, stale instructions.

## Non-Negotiables

- **Bun only.** Do not use Node, npm, pnpm, yarn, ts-node, or Deno for runtime/test workflows.
- **Do not run `bun ./scripts/agent-bar-omarchy`.** That file is a Bash shim. Use `./scripts/agent-bar-omarchy` or `bun run start`.
- **Do not mutate the user's live desktop as verification** unless explicitly requested. Avoid running `agent-bar-omarchy setup`, `agent-bar-omarchy uninstall`, `agent-bar-omarchy remove`, or `agent-bar-omarchy update` without clear user approval. Run `assets install` only against temp/injected paths unless the user approves live paths.
- **Do not edit live `~/.config/waybar` / `~/.config/agent-bar-omarchy` manually for tests.** Use temp directories, injected paths, and XDG env overrides.
- **Do not convert Bash shims in `scripts/` to TypeScript.** `scripts/agent-bar-omarchy` is the `bin` entrypoint and must remain a Bash wrapper.
- **Keep stdout clean for Waybar JSON.** Diagnostics/logs belong on stderr unless a command is intentionally terminal/TUI output.
- **Preserve user changes.** If the worktree has unrelated modifications, do not revert or rewrite them.

## Commands

```bash
bun install
bun run start          # CLI entry, same app as ./scripts/agent-bar-omarchy
./scripts/agent-bar-omarchy --help
bun run dev            # watch mode
bun test               # bun:test with coverage via bunfig.toml
bun run typecheck      # tsc --noEmit
bun run lint           # biome check
bun run lint:fix       # biome check --write
```

Use the narrowest verification that covers the change:

| Change area | Preferred focused verification |
| --- | --- |
| Docs/agent instructions only | `git diff --check` |
| CLI parsing/help | `bun test tests/cli.test.ts` |
| Cache | `bun test tests/cache.test.ts` |
| Settings | `bun test tests/settings.test.ts` |
| Providers | `bun test tests/providers/<provider>.test.ts` plus related helper tests |
| Formatters/tooltips/classes | `bun test tests/formatters.test.ts tests/formatters-snapshot.test.ts` |
| Waybar contract/integration | `bun test tests/waybar-contract.test.ts tests/waybar-integration.test.ts` |
| Shared TypeScript contracts | `bun run typecheck` |
| Broad changes before handoff | `bun test && bun run typecheck && bun run lint` |

## Runtime and Owned Paths

The app owns these paths at runtime:

| Path | Purpose |
| --- | --- |
| `~/.config/agent-bar-omarchy/settings.json` | Versioned user settings; normalized on load; atomic tmp+rename writes |
| `~/.cache/agent-bar-omarchy/` | Provider quota cache |
| `~/.local/bin/agent-bar-omarchy` | Symlink created by setup |
| `~/.config/waybar/agent-bar-omarchy/icons/` | Installed provider icons |
| `~/.config/waybar/agent-bar-omarchy/modules.jsonc` | Generated Waybar module include |
| `~/.config/waybar/agent-bar-omarchy/style.css` | Generated Waybar stylesheet |
| `~/.config/waybar/scripts/agent-bar-omarchy-open-terminal` | Terminal helper for click actions |

Provider credentials are external and only read/used by providers:

- Claude: `~/.claude/.credentials.json`
- Codex: `~/.codex/auth.json`, `~/.codex/sessions/**`
- Amp: `amp` binary from PATH or common install locations

## Architecture Map

- `src/index.ts` — main CLI dispatcher. Parses args, configures logging, dynamically imports TUI/setup/update/uninstall paths, fetches quotas, and chooses terminal vs Waybar output.
- `src/cli.ts` — argument parser and help UI. Unknown commands warn/suggest; default command is Waybar JSON.
- `src/action-right.ts` — Waybar right-click flow. Disconnected/expired providers open single-provider login; connected providers refresh cache and show terminal output.
- `src/app-identity.ts` — single source for app name, legacy name, Waybar namespace, CSS/module prefixes, helper names, and backup suffixes.
- `src/config.ts` — XDG paths, provider credential locations, cache TTL, API timeout, colors, and thresholds.
- `src/cache.ts` — file cache with safe key validation, TTL, in-flight fetch deduplication, and legacy cache migration.
- `src/settings.ts` — settings schema v1, default values, validation, legacy settings migration, normalize-on-load, and atomic writes.
- `src/providers/` — provider implementations plus registry/orchestration.
- `src/formatters/` — terminal and Waybar rendering; shared quota formatting helpers.
- `src/tui/` — clack/prompts menu, login flows, model configuration, layout/provider ordering, and color helpers.
- `src/waybar-contract.ts` — stable generated Waybar module/CSS/assets contract.
- `src/waybar-integration.ts` — safe-ish live Waybar patching for include/import/modules-right while preserving unrelated config.
- `scripts/` — Bash wrappers and terminal launcher.
- `docs/` — operational documentation. `docs/plans/` and `docs/superpowers/specs/` are historical, not current source of truth.
- `snippets/` — reference/manual Waybar snippets only. Normal setup uses generated contract + integration code.

## Provider Contract

Every provider implements `Provider` from `src/providers/types.ts`:

- `id`: stable lowercase identifier, e.g. `claude`, `codex`, `amp`.
- `name`: display name.
- `cacheKey`: unique cache key using only `[a-zA-Z0-9_-]`.
- `isAvailable()`: fast availability check. Do not do network requests here; avoid expensive work.
- `getQuota()`: returns `ProviderQuota`; provider-level errors should normally be represented as `error` in the returned object, not thrown.

Current provider patterns:

- Claude: reads Claude Code OAuth credentials, fetches Anthropic usage API with `AbortController`, caches successful API responses under `claude-usage`.
- Codex: prefers `codex app-server` JSON-RPC-ish protocol via stdio; falls back to recent session `.jsonl` rate limit events; normalizes buckets into `modelsDetailed`; cache key `codex-quota`.
- Amp: locates official `amp` CLI, runs `amp usage`, parses stdout, computes free-tier refill ETA, cache key `amp-quota`.

When adding/changing providers:

1. Implement the provider class in `src/providers/<name>.ts`.
2. Self-register at module scope with `registerProvider(new <Name>Provider())`.
3. Export the class and add a side-effect import in `src/providers/index.ts`.
4. Add the provider to `WAYBAR_PROVIDERS` in `src/waybar-contract.ts` if it should appear in Waybar settings/export.
5. Check all current provider-specific surfaces, not only the registry: `src/index.ts`, `src/theme.ts`, `src/formatters/waybar.ts`, `src/formatters/terminal.ts`, `src/tui/login.ts`, `src/tui/login-single.ts`, `src/tui/list-all.ts`, and `src/tui/configure-layout.ts`.
6. Add icon handling, provider color/theming, TUI login/config behavior, and tests.
7. Keep the standard not-logged-in message when applicable: `Not logged in. Open \`agent-bar-omarchy menu\` and choose Provider login.`

## Quota Data Conventions

- Percentages are `remaining` values from `0` to `100`.
- `resetsAt` is ISO 8601 or `null`.
- `primary` drives the compact Waybar module text/status.
- `secondary` is used for additional shared windows.
- `weeklyModels` is Claude-specific weekly model data.
- `models` is a flattened per-model map for legacy/simple rendering.
- `modelsDetailed` is the canonical multi-window per-model structure for Codex-style providers.
- `extraUsage` represents additional spend/credits/budget data.
- `meta` is provider-specific display metadata; keep it string-only.

## Settings Contract

Settings are schema version `1` and live under `~/.config/agent-bar-omarchy/settings.json`.

Defaults:

- `waybar.providers`: `['claude', 'codex', 'amp']`
- `waybar.providerOrder`: `['claude', 'codex', 'amp']`
- `waybar.showPercentage`: `true`
- `waybar.separators`: `gap`
- `windowPolicy.codex`: `both`

Validation/normalization rules:

- Valid separator styles: `pill`, `gap`, `bare`, `glass`, `shadow`, `none`.
- Valid window policies: `both`, `five_hour`, `seven_day`.
- Unknown providers are dropped.
- Duplicate providers are collapsed.
- Enabled providers missing from `providerOrder` are appended.
- Invalid values silently fall back to defaults.
- Normalized settings are saved back when loaded data differs.

## Cache Contract

- Default TTL is `CONFIG.cache.ttlMs` = 5 minutes.
- Cache files are JSON `CacheEntry<T>` objects with `data`, `fetchedAt`, and `expiresAt`.
- Cache keys must match `/^[a-zA-Z0-9_-]+$/`; traversal, spaces, dots, and slashes are rejected.
- `getOrFetch()` deduplicates concurrent misses for the same key.
- Failed fetches must not poison the cache.
- Legacy cache migration exists only to move old runtime state into `~/.cache/agent-bar-omarchy`.

## Waybar Contract and Integration

Module IDs:

- `custom/agent-bar-omarchy-claude`
- `custom/agent-bar-omarchy-codex`
- `custom/agent-bar-omarchy-amp`

CSS selectors use `#custom-agent-bar-omarchy-<provider>`.

Waybar class contract:

- Aggregate output starts with `agent-bar-omarchy` and adds provider-scoped status classes such as `claude-ok`, `codex-warn`, or `amp-critical`.
- Per-provider output starts with `agent-bar-omarchy-<provider>` and adds one plain status class: `ok`, `low`, `warn`, `critical`, or `disconnected`.
- Disabled single-provider output uses `agent-bar-omarchy-hidden`.

Integration rules:

- `setup` writes generated `modules.jsonc` and `style.css` under `~/.config/waybar/agent-bar-omarchy/`.
- Live `config.jsonc` is patched by adding/updating `include` and `modules-right`, not rewritten wholesale.
- Live `style.css` receives exactly one managed import: `@import url("./agent-bar-omarchy/style.css");`.
- Backups use `.<app-name>-backup` suffix and are created before first managed mutation.
- JSONC comments and unrelated Waybar entries should be preserved. Do not replace this with naive `JSON.parse`/`JSON.stringify` over live Waybar config.

## Formatters and UI Rules

- Terminal output uses ANSI true-color from `src/theme.ts` and respects `NO_COLOR`.
- Waybar tooltip output uses Pango markup with hex colors; never emit ANSI escapes in Waybar JSON.
- Escape dynamic tooltip/error content with XML escaping before embedding in Pango markup.
- Keep box-drawing characters centralized in `BOX` from `src/theme.ts`.
- Keep stdout valid for the current command: JSON for Waybar/export commands, rich terminal text for terminal/TUI commands.
- TUI flows use `@clack/prompts`; handle `p.isCancel()` and leave the terminal in a sane state.

## Legacy Policy

The current product name and public namespace is **`agent-bar-omarchy`**.

`qbar` is legacy compatibility only:

- Allowed in `LEGACY_*` constants, migration/removal code, and tests that prove legacy state is migrated or cleaned.
- Allowed in historical changelog entries and historical planning/spec files.
- Not allowed for new user-facing commands, new docs examples, new Waybar module IDs, new CSS selectors, new settings paths, new symlinks, or new cache keys.

Also do not reintroduce removed/old surfaces such as Antigravity, `llm-usage`, external theme-repo dependencies, or Omarchy theme coupling. The app is theme-agnostic and owns its generated Waybar integration.

## Code Style

- TypeScript strict mode is enabled.
- Use ESM imports/exports.
- Prefer existing local helpers and contracts over new one-off abstractions.
- Names of variables, functions, classes, and files should be in English.
- User-facing repo communication and commit messages are Portuguese unless the user asks otherwise.
- Formatting is Biome: 2 spaces, single quotes, 120-column line width.
- Keep public behavior small and explicit; avoid adding configurability without a current need.
- Keep provider errors useful but stable; tests assert exact strings in several places.
- Prefer `APP_NAME`, `WAYBAR_*`, `TERMINAL_HELPER_NAME`, and related identity constants over hardcoded app strings.

## Testing Patterns

- Tests use `bun:test`.
- Tests should not require real provider credentials, live CLIs, network access, or a running Waybar.
- Use temp directories and restore env/global state in `afterEach`.
- Set `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, and related env overrides before importing modules that read path config, especially `src/config.ts` or modules that import it.
- Provider tests mock filesystem, fetch, process spawning, and app-server/session data.
- Snapshot tests cover terminal/Waybar display contracts; update snapshots only when the display contract intentionally changes.
- Waybar integration tests should pass injected paths and assert unrelated config/style content is preserved.

## Operational Documentation

Use current operational docs as references:

- `README.md` — quick start and command surface.
- `CONTRIBUTING.md` — contributor workflow and Portuguese conventional commits.
- `docs/commands.md` — public command matrix.
- `docs/runtime.md` — owned paths/settings/cache behavior.
- `docs/new-provider.md` — current provider extension checklist.
- `docs/waybar-contract.md` — export/assets/CSS/module contract.
- `docs/integration.md` — setup/apply/remove ownership model.
- `docs/troubleshooting.md` — runtime troubleshooting.

Treat these as non-operational or historical unless explicitly editing them:

- `docs/plans/**`
- `docs/superpowers/specs/**`
- old `CHANGELOG.md` release notes
- `snippets/**` manual integration examples

## Safe Development Workflow

1. Check the current worktree and do not disturb unrelated changes.
2. Read the smallest set of files needed to understand the contract you are changing.
3. Make focused edits that follow the existing module boundaries.
4. Run focused verification; broaden only when contracts or shared behavior changed.
5. Report exactly what changed, what was verified, and any known unverified risk.
