# Contributing

Quick start for developers.

## Prerequisites

| Tool | Minimum |
|------|---------|
| [Rust](https://rustup.rs) | 1.88 (MSRV — `tachyonfx` via `anpa` requires it) |
| Git | recent |

Rust + Cargo is the only supported toolchain.

## Dev install (build from source → live Waybar)

Wire your local checkout straight into Waybar so every build shows up on the
next poll:

```bash
git clone git@github.com:othavi0/agent-bar.git
cd agent-bar
cargo build
./target/debug/agent-bar setup
```

`setup` symlinks `~/.local/bin/agent-bar` to the debug binary inside this
checkout. Rebuild with `cargo build`; the next Waybar tick picks it up.

If you already have a non-dev install, wipe it first to avoid the symlink
fighting your changes:

```bash
unlink ~/.local/bin/agent-bar 2>/dev/null
rm -rf ~/.agent-bar ~/.config/agent-bar ~/.cache/agent-bar
rm -rf ~/.config/waybar/agent-bar
```

`agent-bar update` refuses to run from a dev checkout. Use `git pull` instead.

## Useful commands

```bash
cargo build                                      # Debug build
cargo build --release                            # Release build
cargo run -- status                              # Run from source
cargo test                                       # Full test suite
cargo clippy --all-targets -- -D warnings        # Lint (must pass clean)
cargo fmt                                        # Format
```

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

- Rust stable. No `unsafe` without explicit justification. No `unwrap()` in
  production paths — use explicit error handling that produces user-facing
  messages.
- Identifiers and file names in English, `snake_case`. Repo communication and
  commits in Portuguese.
- `cargo fmt` for formatting. `cargo clippy --all-targets -- -D warnings` must
  pass clean.

## Tests

Tests live in `tests/`. No real credentials, no live CLIs, no network, no real
Waybar — mock filesystem, fetch, spawn, and app-server data.

```bash
cargo test                        # All
cargo test cache                  # Filter by name
```

When using `XDG_CONFIG_HOME` / `XDG_CACHE_HOME` in a test, set them **before**
any code that reads config paths, since config resolves paths at initialization.

Restore env and global state after each test.

## Releasing

Releases are cut by bumping `version` in `Cargo.toml`, updating `CHANGELOG.md`,
committing, and creating a GitHub Release with tag `v<version>`. The
`publish.yml` workflow triggers on `release: published`.

## Adding a provider

See [`docs/new-provider.md`](docs/new-provider.md) for the full checklist.
Short version: implement the `Provider` trait from `src/providers/types.rs`,
register it in `src/providers/mod.rs`, add tests under `tests/`, drop an icon
in `icons/`.

## Links

- [README](README.md)
- [Docs index](docs/README.md)
- [Waybar contract](docs/waybar-contract.md)
- [Troubleshooting](docs/troubleshooting.md)
