# agent-bar — Agent Instructions

LLM quota monitor for Waybar. Tracks Claude, Codex, GitHub Copilot, and Amp
usage; renders a compact Waybar module plus a rich tooltip, exports Waybar
modules/CSS/icons, owns its runtime state, and ships an interactive TUI for
provider login and layout/model configuration.

This file is the canonical guidance for coding agents in this repo. `CLAUDE.md`
is a thin shim that delegates here so Claude-specific instructions never drift
from the shared contract. **The code in `src/` is the ultimate source of
truth** — when this document and the code disagree, the code wins; fix the
drift here.

## 1. Hard Rules

Violating any of these breaks the build, the user's desktop, or the product
contract.

- **Bun only.** No Node, npm, pnpm, yarn, ts-node, or Deno for runtime or test
  workflows.
- **Never run `bun ./scripts/agent-bar`.** That file is a Bash shim and the
  `bin` entrypoint — run it as `./scripts/agent-bar` or use `bun run start`.
- **Never convert the Bash shims in `scripts/` to TypeScript.**
  `scripts/agent-bar` must stay a Bash wrapper.
- **Do not mutate the user's live desktop as verification.** Do not run
  `agent-bar setup`, `update`, `uninstall`, or `remove` without explicit user
  approval. Run `agent-bar assets install` only against temp/injected paths
  unless the user approves live paths.
- **Do not hand-edit live `~/.config/waybar` or `~/.config/agent-bar` for
  tests.** Use temp directories, injected path flags, and `XDG_*` env
  overrides.
- **Keep stdout clean.** Waybar parses stdout as JSON; diagnostics and logs go
  to stderr (the `logger` already does this). Only intentionally terminal/TUI
  commands write rich text to stdout.
- **Preserve unrelated user changes.** If the worktree has modifications
  unrelated to your task, do not revert or rewrite them.
- **Legacy is gone — keep it gone.** The product name and namespace is
  `agent-bar`. The names `qbar` and `agent-bar-omarchy`, the Antigravity and
  `llm-usage` providers, external theme-repo dependencies, and Omarchy theme
  coupling were all removed in `4.0.0`. Do not reintroduce them as commands,
  module IDs, CSS selectors, settings keys, symlinks, or cache keys. Historical
  `CHANGELOG.md` mentions are fine.

## 2. How to Work

### Commands

```bash
bun install
bun run start          # CLI entry — same app as ./scripts/agent-bar
bun run dev            # watch mode
bun test               # bun:test
bun run typecheck      # bun x tsc --noEmit
bun run lint           # biome check
bun run lint:fix       # biome check --write
bun run build          # bundle to dist/
```

### Verification

Use the narrowest verification that covers the change; broaden only when shared
contracts move.

| Change area | Focused verification |
| --- | --- |
| Docs / agent instructions only | `git diff --check` |
| CLI parsing / help | `bun test tests/cli.test.ts` |
| Cache | `bun test tests/cache.test.ts` |
| Settings | `bun test tests/settings.test.ts` |
| A provider | `bun test tests/providers/<provider>.test.ts` (Codex also `tests/providers/codex-appserver.test.ts`) |
| `BaseProvider` orchestration | `bun test tests/providers/base.test.ts` |
| Formatters / tooltips / classes | `bun test tests/formatters.test.ts tests/formatters-snapshot.test.ts tests/formatters-segments.test.ts` |
| Waybar export contract | `bun test tests/waybar-contract.test.ts` |
| Managed update flow | `bun test tests/update.test.ts` |
| Theme / colors / identity constants | `bun test tests/theme.test.ts tests/colors.test.ts tests/config.test.ts tests/app-identity.test.ts` |
| CLI locator helpers | `bun test tests/amp-cli.test.ts tests/copilot-cli.test.ts` |
| Shared TypeScript contracts | `bun run typecheck` |
| Broad changes before handoff | `bun test && bun run typecheck && bun run lint` |

### Code Style

- TypeScript strict mode; ESM imports/exports only.
- Biome formatting: 2 spaces, single quotes, 120-column width. An unused import
  is an error.
- Variable, function, class, and file names in English. User-facing repo
  communication and commit messages in Portuguese unless the user asks
  otherwise. Conventional Commits, subject ≤ 50 chars.
- Prefer the existing identity constants (`APP_NAME`, `WAYBAR_*`,
  `TERMINAL_HELPER_NAME`, `BACKUP_SUFFIX` from `src/app-identity.ts`) over
  hardcoded app strings.
- Prefer existing local helpers and contracts over new one-off abstractions.
  Keep public behavior small and explicit; do not add configurability without a
  current need.
- Provider error strings are part of the contract — tests assert exact
  strings. Keep them useful and stable.
- Never use `!` non-null assertions; narrow with an explicit guard that
  `throw`s.

### Safe Development Workflow

1. Check the worktree; leave unrelated changes alone.
2. Read the smallest set of files needed to understand the contract you are
   changing.
3. Make focused edits that follow existing module boundaries.
4. Run focused verification; broaden when shared behavior changed.
5. Report what changed, what was verified, and any known unverified risk.

## 3. Architecture Map

**Entry & CLI**
- `src/index.ts` — shebang entry; parses args, configures logging,
  lazy-imports lifecycle/TUI paths, fetches quotas, picks terminal vs Waybar
  output.
- `src/cli.ts` — `parseArgs`, help text, Levenshtein command suggestion. The
  default command is Waybar JSON.
- `src/menu.ts` — secondary entry that opens the TUI.

**Lifecycle** (each lazy-imported by `index.ts`)
- `src/setup.ts` — installs Waybar assets, creates the `~/.local/bin/agent-bar`
  symlink, patches live Waybar config, reloads Waybar.
- `src/update.ts` — detects the install type and updates it: `bun add -g` for
  npm installs, git pull for the managed `~/.agent-bar` checkout, then re-runs
  setup.
- `src/uninstall.ts` — interactive removal of owned paths and managed Waybar
  entries.
- `src/remove.ts` — thin non-interactive wrapper over uninstall (`force: true`).
- `src/install.ts` — `bun` / global-package install helpers used by setup.
- `src/action-right.ts` — Waybar right-click handler: login if disconnected,
  else refresh the cache and show terminal output.

**Providers** — `src/providers/`
- `types.ts` — every provider/quota interface and type.
- `base.ts` — `BaseProvider` abstract class; owns the `getQuota()`
  orchestration template.
- `registry.ts` — in-memory provider registry (`registerProvider`, getters).
- `index.ts` — re-exports, side-effect imports the four providers
  (self-registration), `getAllQuotas`/`getQuotaFor` with timeout + retry.
- `claude.ts`, `codex.ts`, `copilot.ts`, `amp.ts` — the four provider
  implementations.
- `extras.ts` — typed accessors for each provider's `extra` payload.

**Formatters** — `src/formatters/` (see §4 Formatters Pipeline)
- `builders/` — pure `Line[]`-emitting builders (`claude`, `codex`, `copilot`,
  `amp`, `generic`, plus `shared` and `types`).
- `render-ansi.ts`, `render-pango.ts` — `Line[]` → ANSI / Pango string.
- `segments.ts` — `Line`/`Segment`/`ColorToken` types and segment helpers.
- `view-model.ts`, `codex-helpers.ts`, `shared.ts` — derived view models and
  pure formatting helpers.
- `terminal.ts`, `waybar.ts` — thin dispatchers: pick the builder per provider,
  call the renderer.

**Waybar**
- `src/waybar-contract.ts` — stable generated module/CSS/icon export contract;
  `WAYBAR_PROVIDERS`.
- `src/waybar-integration.ts` — careful in-place patching of live Waybar
  config/style, preserving unrelated content.

**TUI** — `src/tui/`
- `index.ts` — clack menu (list / layout / models / login).
- `list-all.ts`, `configure-layout.ts`, `configure-models.ts`, `login.ts`,
  `login-single.ts` — the menu actions.
- `render-colorize.ts` — `Line[]` → colorized TUI string.
- `terminal-ui.ts`, `colors.ts` — shared clack UI helpers and the TUI color
  map.

**Support**
- `src/app-identity.ts` — single source for app name, Waybar namespace,
  CSS/module prefixes, helper/backup names.
- `src/config.ts` — XDG paths, credential locations, cache TTL, API timeout,
  color thresholds.
- `src/cache.ts` — file cache with key validation, TTL, in-flight dedup.
- `src/settings.ts` — settings schema, defaults, validation, normalize-on-load,
  atomic writes.
- `src/theme.ts` — `ONE_DARK` palette, provider colors, ANSI codes, `BOX`
  characters; respects `NO_COLOR`.
- `src/logger.ts` — leveled logger; all output to stderr; default level `warn`.
- `src/amp-cli.ts`, `src/copilot-cli.ts` — locate the external Amp / Copilot
  CLI binaries.

## 4. Contracts

### Provider Contract

Every provider satisfies `Provider` (`src/providers/types.ts`): `id`, `name`,
`cacheKey` (all `readonly`), `isAvailable()`, `getQuota()`.

- `id` — stable lowercase identifier (`claude`, `codex`, `copilot`, `amp`).
- `cacheKey` — unique, matching `/^[a-zA-Z0-9_-]+$/`.
- `isAvailable()` — fast local check (credential file exists, CLI on disk). No
  network, no expensive work.
- `getQuota()` — returns a `ProviderQuota`; provider-level failures are
  represented as the `error` field of the returned object, not thrown.

Most providers extend **`BaseProvider`** (`src/providers/base.ts`), which owns
the `getQuota()` template: build the base object → availability gate →
`cache.getOrFetch` → `buildQuota`, with error handling. A subclass implements
only `fetchRaw()`, `buildQuota()`, and `unavailableError()`, and may override
`toUserFacingError()`. **`CodexProvider`, `CopilotProvider`, and `AmpProvider`
extend `BaseProvider`.** **`ClaudeProvider` implements `Provider` directly** —
it does not fit the template and manages its own caching inline.

Current fetch strategies:
- Claude — reads Claude Code OAuth credentials, fetches the Anthropic usage API
  with an `AbortController`; cache key `claude-usage`.
- Codex — `codex app-server` JSON-RPC-ish protocol over stdio, falling back to
  recent session `.jsonl` rate-limit events; cache key `codex-quota`.
- Copilot — the official Copilot CLI over LSP-framed stdio JSON-RPC
  (`account.getQuota`); cache key `copilot-quota`.
- Amp — locates the `amp` CLI, runs `amp usage`, parses stdout; cache key
  `amp-quota`.

### Quota Data Conventions

`ProviderQuota` is a union of per-provider shapes; all share `QuotaCore`
(`src/providers/types.ts`).

- A `QuotaWindow` carries `remaining` (0–100 percentage), `resetsAt` (ISO 8601
  or `null`), and optional `windowMinutes`.
- `primary` drives the compact Waybar module text and status class.
  `secondary` is an additional window.
- `models` is a flat `Record<string, QuotaWindow>` for simple per-model
  rendering.
- The per-provider `extra` payload (accessed via `src/providers/extras.ts`)
  carries the rich data: Claude's `weeklyModels` and `extraUsage`, Codex's
  `modelsDetailed` (the canonical multi-window per-model structure) and
  `extraUsage`, the Copilot `quotaSnapshots`, and the string-only `meta` map
  carried by Amp and Copilot.

### Settings Contract

Settings live at `~/.config/agent-bar/settings.json`, schema `version: 2`
(`CURRENT_VERSION` in `src/settings.ts`).

Defaults: `waybar.providers` and `waybar.providerOrder` =
`['claude', 'codex', 'copilot', 'amp']`; `waybar.showPercentage` = `true`;
`waybar.separators` = `'gap'`; `waybar.displayMode` = `'remaining'`;
`windowPolicy.codex` = `'both'`.

Normalization on load:
- Valid separators: `pill`, `gap`, `bare`, `glass`, `shadow`, `none`. Valid
  display modes: `remaining`, `used`. Valid window policies: `both`,
  `five_hour`, `seven_day`.
- Invalid values fall back to the default silently.
- Unknown providers are dropped; duplicates collapsed; enabled providers
  missing from `providerOrder` are appended.
- A v1 settings file is migrated to v2 (inserting `copilot` after `codex` when
  it held the legacy default).
- When the normalized form differs from disk, it is written back via a
  `.tmp`+rename atomic write.

### Cache Contract

- Provider quota responses are cached as JSON files under `~/.cache/agent-bar/`.
- Default TTL is `CONFIG.cache.ttlMs` = 5 minutes.
- A cache entry is `{ data, fetchedAt, expiresAt }`.
- Cache keys must match `/^[a-zA-Z0-9_-]+$/`; traversal, dots, spaces, and
  slashes are rejected.
- `getOrFetch()` deduplicates concurrent misses for the same key via an
  in-flight promise map.
- A failed fetch throws and does not write the cache — failures never poison
  it.

### Waybar Contract & Integration

Module IDs are `custom/agent-bar-<provider>`; CSS selectors are
`#custom-agent-bar-<provider>`. `WAYBAR_PROVIDERS` (`src/waybar-contract.ts`)
is `['claude', 'codex', 'copilot', 'amp']`.

Class contract:
- Aggregate output starts with `agent-bar` and adds `<provider>-<status>` per
  available provider (e.g. `claude-ok codex-warn`).
- Per-provider output starts with `agent-bar-<provider>` and adds one plain
  status: `ok`, `low`, `warn`, `critical`, or `disconnected`.
- A disabled single-provider module uses the `agent-bar-hidden` class.

Integration (`src/waybar-integration.ts`):
- `setup` writes generated `modules.jsonc` and `style.css` under
  `~/.config/waybar/agent-bar/`.
- Live `config.jsonc` is *patched* — `include` and `modules-right` are
  added/updated, never rewritten wholesale.
- Live `style.css` gets exactly one managed import:
  `@import url("./agent-bar/style.css");`.
- A `.<app-name>-backup` copy is made before the first managed mutation.
- JSONC comments and unrelated entries must survive. Never round-trip live
  Waybar config through naive `JSON.parse`/`JSON.stringify`.

### Formatters Pipeline

Rendering is a three-stage pipeline: **builder → renderer → dispatcher**.

1. **Builders** (`src/formatters/builders/`) take a `ProviderQuota` plus
   `BuildOptions` and return `Line[]` — a list of `Segment[]`. They hold all
   layout logic and are pure: no I/O, no markup, no settings reads. One builder
   per provider, plus `generic` for an unrecognized provider.
2. **Renderers** turn `Line[]` into a string: `render-ansi.ts` for terminal
   ANSI true-color, `render-pango.ts` for Waybar Pango markup,
   `tui/render-colorize.ts` for the TUI. **XML-escaping happens only in
   `render-pango.ts`** — builders never escape; a `Segment` marked `raw`
   bypasses both color-wrapping and escaping.
3. **Dispatchers** `terminal.ts` and `waybar.ts` map each provider to its
   builder, call the right renderer, and print/return. They are thin;
   `waybar.ts` additionally caches settings for 5 s because Waybar polls on a
   tight interval.

UI rules: terminal output uses ANSI true-color and respects `NO_COLOR`; Waybar
JSON never contains ANSI escapes; box-drawing characters come from `BOX` in
`src/theme.ts`; TUI flows use `@clack/prompts` and must handle `p.isCancel()`
and leave the terminal sane.

### Runtime & Owned Paths

| Path | Purpose |
| --- | --- |
| `~/.config/agent-bar/settings.json` | Versioned user settings; atomic writes |
| `~/.cache/agent-bar/` | Provider quota cache |
| `~/.agent-bar` | Managed install checkout (update flow) |
| `~/.local/bin/agent-bar` | Symlink created by setup |
| `~/.config/waybar/agent-bar/` | Generated `modules.jsonc`, `style.css`, `icons/` |
| `~/.config/waybar/scripts/agent-bar-open-terminal` | Terminal helper for click actions |

Paths honor `XDG_CONFIG_HOME` / `XDG_CACHE_HOME`. Provider credentials are
external and only read by providers: Claude `~/.claude/.credentials.json`;
Codex `~/.codex/auth.json` and `~/.codex/sessions/`; Copilot the official
Copilot CLI config; Amp the `amp` binary located via PATH or common install
dirs.

## 5. Adding or Changing a Provider

1. Implement the provider in `src/providers/<name>.ts`. **Extend
   `BaseProvider`** and implement `fetchRaw()`/`buildQuota()`/
   `unavailableError()` — only implement `Provider` directly if the provider
   genuinely cannot fit the template (as Claude does not).
2. Self-register at module scope: `registerProvider(new <Name>Provider())`.
3. Export the class from `src/providers/<name>.ts` and add a side-effect import
   in `src/providers/index.ts`.
4. Add the provider to `WAYBAR_PROVIDERS` in `src/waybar-contract.ts` if it
   should appear in Waybar export/settings.
5. Add a dedicated builder `src/formatters/builders/<name>.ts` and register it
   in the `terminal.ts` and `waybar.ts` dispatchers — otherwise it falls back
   to `buildGeneric`.
6. Sweep the remaining provider-specific surfaces: `src/theme.ts`
   (color/icon), `src/tui/login.ts`, `src/tui/login-single.ts`,
   `src/tui/list-all.ts`, `src/tui/configure-layout.ts`, and `src/index.ts`.
7. Add tests under `tests/providers/<name>.test.ts` and snapshot coverage.
8. Keep the standard not-logged-in message where applicable:
   `` Not logged in. Open `agent-bar menu` and choose Provider login. ``

## 6. Testing Patterns

- Tests run on `bun:test`.
- A test must not need real provider credentials, live CLIs, network access, or
  a running Waybar. Mock the filesystem, `fetch`, process spawning, and
  app-server/session data.
- Use temp directories; set `XDG_CONFIG_HOME` / `XDG_CACHE_HOME` (and related
  overrides) *before* importing any module that reads path config, especially
  `src/config.ts` or anything importing it. Restore env and global state in
  `afterEach`.
- Snapshot tests cover the display contract. The terminal snapshot is sanitized
  (ANSI stripped) to validate text and layout; the Waybar snapshot keeps full
  Pango byte-for-byte. Update snapshots only when the display contract changes
  on purpose.
- Provider error strings are asserted verbatim in several suites — changing one
  is a contract change.

## 7. Pointers

- `README.md` — quick start and command surface.
- `CONTRIBUTING.md` — contributor workflow, Portuguese Conventional Commits.
- `docs/commands.md` — full CLI command/flag reference.
- `docs/runtime.md` — owned paths, settings, cache behavior.
- `docs/new-provider.md` — provider extension checklist.
- `docs/waybar-contract.md` — export/CSS/module/asset contract.
- `docs/integration.md` — setup/update/remove ownership model.
- `docs/troubleshooting.md` — runtime troubleshooting.

`CHANGELOG.md` is historical — treat it as non-operational unless explicitly
editing release notes.
