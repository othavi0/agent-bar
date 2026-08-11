# Design: Executable Discovery Must Not Canonicalize Shims (mise)

Date: 2026-08-11
Status: approved (option A)

## Problem

With the Amp CLI installed through mise, the only `amp` on the Quickshell PATH
is the shim `~/.local/share/mise/shims/amp`, a symlink to `/usr/bin/mise`.
Discovery (`resolve_executable` in `src/providers/catalog.rs`) canonicalized the
candidate it found, so the helper executed `/usr/bin/mise usage` instead of the
shim. mise sees `argv[0]` = `mise`, never enters shim mode, and runs its own
`mise usage` subcommand (usage-spec output, ~213 KB, exit 0, ~15 ms). The parser
finds no Amp line, the result becomes `Ready` with `windows: []`, and the UI
shows "Amp reports no quota / This account is billed another way."

Reproduced on the user's machine on 2026-08-11 with the exact Quickshell
environment. Decisive tests:

- `amp` as a symlink to `/usr/bin/mise` on PATH: collection fails (`ready`, 0
  windows, 33 ms).
- `amp` as a symlink to the real Node binary: collection works (`ready`, 3
  windows, ~1.2 s).

## Blast radius

- Amp collection (`amp usage`): broken when installed through mise.
- Codex app-server: same failure; the adapter falls back to the session log and
  serves stale data.
- `login` for any provider installed through mise: would execute
  `/usr/bin/mise login`.
- Claude collection is unaffected (HTTP/OAuth). Grok reads billing JSON from
  disk during collection, but its `login` would be affected.

## Fix (option A, approved)

`resolve_executable` returns the path it found **without** following symlinks,
in both branches (PATH scan and fallback templates). Executing the path that
PATH resolved preserves the shim's `argv[0]` and activates mise's shim mode —
standard POSIX behaviour. `canonicalize_best_effort` becomes unused during
resolution and is removed.

Rejected alternatives:

- Canonicalize only when the target basename matches: an extra branch with no
  benefit, since nothing requires canonical paths at execution time.
- Force `argv[0]` through `CommandExt::arg0` while keeping the canonical path:
  fragile, depends on a mise implementation detail, and would require extending
  `ProcessSpec`.

## Tests

- Regression test in `catalog.rs` for the PATH branch: in a temporary directory
  on PATH, `amp` is a symlink to another binary with a different basename (the
  shim shape); discovery must return the symlink path, not the target.
- A second regression test for the fallback-template branch, which returns from
  a different point in `resolve_executable` and would otherwise be free to
  reintroduce canonicalization unnoticed.
- No existing test had to change: `fallback_used_when_path_empty` canonicalizes
  both sides of its comparison and never had a symlink to resolve, so it is
  insensitive to this behaviour either way.
- Contract gates: `cargo fmt --check`, `cargo test`,
  `cargo clippy --all-targets -- -D warnings`.

## Out of scope

- Investigating why Grok reports `windows: []` (a distinct flow that reads
  billing JSON from disk).
- Updating the installed bundle in `~/.config/omarchy/plugins` — that follows
  the final QA gate defined in `CLAUDE.md`.
