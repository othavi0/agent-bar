# Contributing

Quick start for developers.

## Prerequisites

| Tool | Minimum |
|------|---------|
| [Bun](https://bun.sh) | >= 1.0 |
| Git | recent |

Bun is the only supported runtime. Node and Deno will not work.

## Dev install (live edit → live Waybar)

Wire your local checkout straight into Waybar so every edit shows up on the
next poll, with no rebuild:

```bash
git clone git@github.com:othavioquiliao/agent-bar.git
cd agent-bar
bun install
bun run start setup
```

`setup` symlinks `~/.local/bin/agent-bar` to `scripts/agent-bar` inside this
checkout. The shim does `exec bun src/index.ts`, so saved changes apply on the
next Waybar tick.

If you already have a non-dev install (`~/.agent-bar`, npm global, or both),
wipe it first to avoid the symlink fighting your changes:

```bash
unlink ~/.local/bin/agent-bar 2>/dev/null
rm -rf ~/.agent-bar ~/.config/agent-bar ~/.cache/agent-bar
rm -rf ~/.config/waybar/agent-bar
```

`agent-bar update` refuses to run from a dev checkout. Use `git pull` instead.

## Useful commands

```bash
bun run start          # Run from source (equivalent to ./scripts/agent-bar)
bun run dev            # Watch mode (restart on save)
bun test               # Test suite (coverage via bunfig.toml)
bun run typecheck      # bun x tsc --noEmit
bun run lint           # biome check
bun run lint:fix       # biome check --write
```

Do not run `bun ./scripts/agent-bar`. It is a Bash shim and Bun will try to
parse it as JavaScript. Use `./scripts/agent-bar` or `bun run start`.

## Conventional Commits (in Portuguese)

Commit messages use [Conventional Commits](https://www.conventionalcommits.org/)
written in **Portuguese**, subject ≤ 50 chars:

| Prefix      | Use for                                |
|-------------|----------------------------------------|
| `feat:`     | New functionality                      |
| `fix:`      | Bug fix                                |
| `refactor:` | Refactor without behavior change       |
| `test:`     | Tests added or changed                 |
| `docs:`     | Documentation                          |
| `chore:`    | Maintenance (deps, CI, configs)        |
| `perf:`     | Performance                            |
| `style:`    | Formatting only                        |
| `build:`    | Build system or dependencies           |
| `ci:`       | CI configuration                       |

Examples:

```
feat: adiciona provider para Gemini
fix: corrige parsing do reset time no Amp
test: cobre cenários de cache expirado
```

## Code style

- TypeScript strict. Avoid `any`. Never `!` non-null assertion (use a guard
  that throws explicitly).
- Identifiers and file names in English, `camelCase`. Repo communication and
  commits in Portuguese.
- No path aliases — use relative imports.
- No build step at runtime. Bun runs TypeScript directly.
- Biome enforces formatting (2 spaces, single quotes, 120 cols).

## Tests

`bun:test`. Tests in `tests/`, mirroring `src/`. No real credentials, no live
CLIs, no network, no real Waybar — mock `fs`, `fetch`, `spawn`, app-server
data.

```bash
bun test                       # All
bun test tests/cache.test.ts   # One file
```

When using `XDG_CONFIG_HOME` / `XDG_CACHE_HOME` in a test, set them **before**
importing `src/config.ts` or any module that imports it. Config reads env at
import time.

Restore env and global state in `afterEach`.

## Releasing to npm

```bash
bun run release:check     # tests + typecheck + lint + build + pack dry-run
bun run publish:dry-run   # requires npm login in the environment
bun run publish:npm       # real publish — manual, requires explicit approval
```

Publishing is normally done by the GitHub `publish.yml` workflow on
`release: published` (uses `NPM_TOKEN` secret). See `CLAUDE.md` §8.

## Adding a provider

See [`docs/new-provider.md`](docs/new-provider.md) for the full checklist.
Short version: implement `Provider` from `src/providers/types.ts`, register in
`src/providers/index.ts`, add tests under `tests/providers/`, drop an icon in
`icons/`.

## Links

- [README](README.md)
- [Docs index](docs/README.md)
- [Waybar contract](docs/waybar-contract.md)
- [Troubleshooting](docs/troubleshooting.md)
