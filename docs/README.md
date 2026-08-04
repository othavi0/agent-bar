# Documentation

Active product and engineering documentation for Agent Bar.

## User and operator guide

- [Integration](guide/integration.md) — install, migration, update,
  uninstall, ownership, and rollback.
- [Runtime](guide/runtime.md) — owned paths, settings, cache, privacy, and
  state.
- [Troubleshooting](guide/troubleshooting.md) — typed provider and plugin
  failures.
- [Commands](guide/commands.md) — private helper diagnostics and recovery
  grammar.

## Engineering

- [Architecture](dev/architecture.md) — shared service, Rust helper, and
  data flow.
- [JSON output](dev/json-output.md) — status schema v2.
- [New provider](dev/new-provider.md) — adapter and fixture checklist.
- [Omarchy integration](dev/omarchy-shell.md) — Quattro plugin contract.
- [Releasing](dev/releasing.md) — automatic release pipeline and manual
  boundary.
- [Agent process](dev/agents/domain.md) — reading order, plus the
  [issue tracker](dev/agents/issue-tracker.md) and
  [triage labels](dev/agents/triage-labels.md).
- [ADRs](adr/README.md) — durable architectural decisions.
- [Domain vocabulary](../CONTEXT.md) — canonical terms.

## Releases

- [Release notes](releases/README.md) — one tracked file per published
  version, consumed by the automatic release pipeline.

## Canonical v10 package

- [Specification index](specs/v10/README.md)
- [Implementation plan](specs/v10/09-implementation-plan.md)
- [Grok runbook](specs/v10/10-grok-execution-runbook.md)
- [Requirements matrix](specs/v10/REQUIREMENTS_MATRIX.md)

## Historical records

- [v10 post-merge handoff](history/handoff-v10-post-merge.md) — 2026-07-27
  snapshot.
- [v10.0.0 live QA](history/qa/v10.0.0-live-qa-2026-07-27.md) — post-release
  TEST-035…042 matrix and screenshots.

`CHANGELOG.md` release sections 9.0.0 and older, ADR bodies 0001–0003, the
dated snapshots under `docs/history/`, and `docs/superpowers/**` preserve
earlier design and delivery history. The Unreleased changelog section and
the ADR index remain active.
