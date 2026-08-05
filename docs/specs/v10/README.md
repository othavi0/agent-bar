# Agent Bar v10 Specification

Status: **implemented, published as `v10.0.0`, live-accepted (2026-07-27),
and amended by the git-native plugin distribution conversion (2026-08-05)**

Approved on: 2026-07-26 · Merged: [PR #25](https://github.com/othavi0/agent-bar/pull/25) ·
Release: [v10.0.0](https://github.com/othavi0/agent-bar/releases/tag/v10.0.0) ·
Live QA: [docs/history/qa/v10.0.0-live-qa-2026-07-27.md](../../history/qa/v10.0.0-live-qa-2026-07-27.md)

This directory is the canonical product and engineering contract for Agent Bar
v10. Post-merge notes and residuals:
[docs/history/handoff-v10-post-merge.md](../../history/handoff-v10-post-merge.md).

The plugin update/uninstall, distribution, and installation model is
superseded by the git-native plugin distribution design, approved
2026-08-05:
[docs/superpowers/specs/2026-08-05-git-plugin-distribution-design.md](../../superpowers/specs/2026-08-05-git-plugin-distribution-design.md).
That design is this specification's change-control record for
`01-product-contract.md` (PROD-007, PROD-010, the Maintain journey),
`03-cli-and-json-contract.md` (the maintenance grammar and CLI-024..031),
`06-migration-and-legacy-removal.md` (MIG-019A, MIG-020..026), and
`08-plugin-bundle-and-release.md` (the distribution-repository model). Every
other file in this directory, and every other requirement, still describes
the current, unamended contract.

## Product statement

Agent Bar v10 is an Omarchy Quattro Quickshell plugin. Its only graphical
surface is the Quickshell bar widget and consolidated popup. A Rust executable
is bundled inside the plugin as a private helper for provider collection,
normalization, settings, cache, migration, update, and uninstall.

Agent Bar v10 is not a terminal UI, a Waybar module, a standalone desktop
application, an AUR product, or a cargo-binstall product.

## Canonical reading order

1. [01-product-contract.md](01-product-contract.md)
2. [02-target-architecture.md](02-target-architecture.md)
3. [03-cli-and-json-contract.md](03-cli-and-json-contract.md)
4. [04-quickshell-ux-and-accessibility.md](04-quickshell-ux-and-accessibility.md)
5. [05-settings-cache-and-notifications.md](05-settings-cache-and-notifications.md)
6. [06-migration-and-legacy-removal.md](06-migration-and-legacy-removal.md)
7. [07-testing-and-acceptance.md](07-testing-and-acceptance.md)
8. [08-plugin-bundle-and-release.md](08-plugin-bundle-and-release.md)
9. [09-implementation-plan.md](09-implementation-plan.md)
10. [10-grok-execution-runbook.md](10-grok-execution-runbook.md)
11. [REQUIREMENTS_MATRIX.md](REQUIREMENTS_MATRIX.md)
12. [CHECKPOINT_TEMPLATE.md](CHECKPOINT_TEMPLATE.md)

When two statements conflict, the earlier contract in this reading order wins
unless a later file explicitly identifies the requirement ID it refines.

## Requirement IDs

| Prefix | Area |
| --- | --- |
| `PROD` | Product scope and user-facing behavior |
| `ARCH` | Architecture and ownership |
| `CLI` | Private helper command grammar |
| `JSON` | Status schema and provider states |
| `UX` | Quickshell interaction and visual behavior |
| `A11Y` | Keyboard, focus, motion, and accessibility |
| `SET` | Settings |
| `CACHE` | Cache and refresh coordination |
| `NOTIFY` | Usage notifications |
| `MIG` | v9-to-v10 migration |
| `CLEAN` | Legacy removal and ownership |
| `BUNDLE` | Plugin assembly, installation, update, and uninstall |
| `TEST` | Verification and acceptance |
| `DOC` | Documentation |
| `EXEC` | Grok execution and review workflow |

Requirement IDs are stable. An implementation may not silently weaken,
rename, or delete a requirement. A necessary deviation must be documented in
the active checkpoint and approved before work continues.

## Language policy

- All v10 UI copy, tooltips, notifications, accessibility labels, CLI help,
  terminal output, active documentation, specifications, tests, and release
  material are English.
- Commands, code identifiers, JSON keys, provider IDs, and technical names are
  English.
- Provider trademarks and official command names retain their original form.
- v10 does not add an internationalization layer.
- Changelog release sections beginning at `## [9.0.0]`, ADR bodies
  `0001`–`0003`, and `docs/superpowers/**` remain untouched historical evidence
  and are excluded from the active language gate. `CHANGELOG.md` Unreleased,
  the ADR index, and ADR 0004 remain active and must pass.

Documentation requirements:

- `DOC-001`: All active v10 product and engineering documentation is English.
- `DOC-002`: Active commands and JSON examples are executable contract tests.
- `DOC-003`: Changelog releases 9.0.0 and older, ADR bodies 0001–0003, and
  `docs/superpowers/**` are preserved and explicitly excluded from active
  legacy/language gates; Unreleased, the ADR index, and ADR 0004 are active.
- `DOC-004`: Active docs describe only the plugin-first v10 target after
  implementation completes.
- `DOC-005`: Before implementation completes, active docs clearly label target
  behavior and do not claim that v10 is already installed.

## Change control

- This specification branch may change only after explicit user approval.
- Grok implements the specification; Grok does not redefine it.
- Codex performs independent review at every mandatory checkpoint.
- Implementation happens in an isolated worktree on
  `feat/quickshell-native-v10`.
- No implementation commit may be made directly on the specification branch.
- No merge, tag, GitHub Release, or live installation is authorized by this
  specification alone.
