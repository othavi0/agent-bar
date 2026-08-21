# Requirements Matrix

This matrix maps every requirement family to implementation-plan tasks and
mandatory evidence. An inclusive range covers each ID inside the range;
letter-suffixed IDs are listed separately. `Tasks` always names the
`09-implementation-plan.md` task(s) that originally delivered a
requirement; it is not renumbered for later amendments.

Rows marked **amended 2026-08-05** were superseded by the git-native plugin
distribution conversion; their original task numbers stay as the historical
record of first delivery, and their evidence column points at the tests and
design doc that now govern the amended behavior. See
[docs/superpowers/specs/2026-08-05-git-plugin-distribution-design.md](../../superpowers/specs/2026-08-05-git-plugin-distribution-design.md).

## Product and architecture

| Requirements | Tasks | Primary evidence |
| --- | --- | --- |
| `PROD-001`–`PROD-005` | 8–10, 15–17 | manifest, singleton QML tests, migration placement tests, bundle inventory |
| `PROD-006` | 19 | active legacy scan and deleted-file inventory |
| `PROD-007` | 12–13, 17–18 | Settings/Maintenance QML tests; **amended 2026-08-05**: delegation argv tests in `tests/cli.rs` and `tests/qml/tst_Maintenance.qml` |
| `PROD-008` | 3, 6, 11 | schema fixtures, provider fixtures, provider-state QML tests |
| `PROD-009` | 14, 21–22 | keyboard/scroll/a11y tests and real screenshots |
| `PROD-010` | 17 | bundle receipt and version-match tests; **amended 2026-08-05**: distribution-tree mirror in `tests/dist_tree_validate.rs` (archive inventory retired, no archive) |
| `PROD-011`–`PROD-019` | 19–20 | source/dependency/doc legacy gates |
| `PROD-019A` | 3, 6, 10–11, 19 | rejected money fixture, provider/QML tests, legacy scan |
| `PROD-020`–`PROD-024` | 4, 6, 10, 12, 16 | settings order, discovery, chip, Settings, migration tests |
| `PROD-025`–`PROD-030` | 3, 7, 9, 11 | stale/partial/error/schema/service tests |
| `PROD-031` | 3, 6, 10–11 | empty-window fixture and `—` rendering tests |
| `ARCH-001` | 7, 9, 17 | process-lifecycle and transient-worker tests |
| `ARCH-002`–`ARCH-006` | 8–11 | manifest, singleton, two-widget, popup-transfer tests |
| `ARCH-007`–`ARCH-008` | 5–6 | adapter/catalog fixtures and QML raw-output guard |
| `ARCH-009`–`ARCH-010` | 4, 15–18 | settings purity and transaction fault matrix |
| `ARCH-011`–`ARCH-012` | 5–6, 13 | one catalog, argv runner, terminal-helper tests |
| `ARCH-013` | 7, 9 | notification-mode and shared-service tests |
| `ARCH-014` | 5–6 | separate collection/login discovery fixtures |
| `ARCH-015`–`ARCH-019` | 5–6, 13 | literal catalog, URL allowlist, login argv, collection tests |
| `ARCH-020`–`ARCH-021` | 8–9, 13, 17 | singleton IPC, health equality, refresh routing tests |
| `ARCH-022` | 5–7 | literal collection policy, limits, retry, and window-ID tests |
| `ARCH-023`–`ARCH-024` | 9, 12–13 | independent process-lane overlap and serialization tests |
| `ARCH-025` | 6 | exact provider HTTP endpoint, redirect, size, and redaction tests |
| `ARCH-026` | 7, 9, 15–18 | shared/exclusive maintenance barrier tests |

## CLI and JSON

| Requirements | Tasks | Primary evidence |
| --- | --- | --- |
| `CLI-001`–`CLI-005` | 2 | exhaustive grammar table |
| `CLI-005A` | 2, 7, 9 | default-skip and service-evaluate tests |
| `CLI-006`–`CLI-010` | 2, 20 | alias/rejection/help tests and active command docs; **CLI-009 amended 2026-08-05**: `setup` takes no arguments, covered by `tests/cli.rs` |
| `CLI-011`–`CLI-016` | 2–3 | stdout/stderr, exit, serializer-failure tests |
| `CLI-017` | 5–6, 13 | fake login process and terminal-helper status tests |
| `CLI-017A` | 2, 8–9, 17 | exact fast version output and cold-start health tests |
| `CLI-018`–`CLI-023` | 4 | settings purity/validation/atomicity tests |
| `CLI-023A` | 2, 4 | exact settings stdout/newline/stderr tests |
| `CLI-024`–`CLI-025` | 15–16 | doctor read-only/clean ownership tests |
| `CLI-026`–`CLI-030` | 16–18 | **CLI-027, CLI-029 amended 2026-08-05**: superseded by delegation-argv/purge-ordering tests in `tests/cli.rs`, not the retired setup/update/uninstall fault matrix |
| `CLI-031` | 7 | notification dispatch-failure test |
| `JSON-001`–`JSON-008` | 3, 6–7 | provider-state and stale fixtures |
| `JSON-009`–`JSON-015` | 1, 3 | structural schema, semantic validator, invalid fixtures |
| `JSON-009A` | 1, 3, 8, 17 | helper/manifest semantic-version equality tests |
| `JSON-016`–`JSON-022` | 3, 6–7 | order, explicit-provider, partial-failure tests |
| `JSON-022A`–`JSON-022C` | 1, 3, 6 | empty-window and rejected-money fixtures |
| `JSON-023`–`JSON-028` | 3, 5–6, 11 | URL/action enums, redaction, plain-text QML tests |

## Quickshell UX and accessibility

| Requirements | Tasks | Primary evidence |
| --- | --- | --- |
| `UX-001`–`UX-012` | 10 | chip model, click routing, tooltip, wheel tests |
| `UX-013`–`UX-020` | 11 | popup layout and fitted-geometry tests/screenshots |
| `UX-021`–`UX-025` | 9, 11 | monitor-owner, selection, and scroll-reset tests |
| `UX-026`–`UX-032` | 9, 11 | complete provider-state fixture matrix |
| `UX-032A` | 10–11 | empty-window chip/popup tests |
| `UX-033`–`UX-039` | 12 | Settings controls, validation, restore/cancel/save tests |
| `UX-040`–`UX-048` | 13, 17–18 | Maintenance flows, confirmations, argv tests |
| `UX-049`–`UX-058` | 10–14 | approved assets, glyph guard, theme and no-money tests |
| `A11Y-001`–`A11Y-013` | 11, 14 | KeyboardPanel, focus, key, Accessible, motion tests |
| `A11Y-014`–`A11Y-022` | 14 | Flickable, wheel, bounds, scrollbar, focus-scroll tests |
| `A11Y-023` | 14 | PageUp/PageDown/Home/End clamp tests |

## Settings, cache, and notifications

| Requirements | Tasks | Primary evidence |
| --- | --- | --- |
| `SET-001`–`SET-013` | 1, 4, 16 | schema, read purity, atomic write, shell migration tests |
| `SET-014`–`SET-022` | 9, 12 | QML load/save generation and race tests |
| `SET-023` | 9, 12 | startup bootstrap read tests in `tst_Settings.qml` |
| `CACHE-001`–`CACHE-008` | 6–7 | cache content, permissions, TTL, policy parity tests |
| `CACHE-009`–`CACHE-019` | 7 | barrier/fake-clock singleflight and corruption tests |
| `CACHE-019A` | 7 | bypass-writes-cache test |
| `CACHE-019B` | 7, 15–18 | external-status versus maintenance barrier tests |
| `CACHE-020`–`CACHE-025` | 7, 9, 11 | loading/stale/auth/provider refresh tests |
| `NOTIFY-001`–`NOTIFY-010` | 7 | transition, persistence, recovery, dispatch tests |
| `NOTIFY-013`–`NOTIFY-014` | 7 | jitter-tolerance, reminder-cadence, settings-range tests |
| `NOTIFY-011` | 7, 9 | evaluate-mode and one-service tests |
| `NOTIFY-012` | 7 | crash-window and at-least-once contract tests |

## Migration, cleanup, bundle, and release

| Requirements | Tasks | Primary evidence |
| --- | --- | --- |
| `MIG-001`, `MIG-006` | 15 | `tests/cli.rs::binary_setup_migrates_v9_settings_to_strict_v10` (setup migration backup) and `src/plugin/doctor.rs::clean_removes_only_owned_legacy_and_backups` (doctor clean backup) |
| `MIG-002`, `MIG-003`, `MIG-004`, `MIG-005` | 15 | **amended 2026-08-05**: the stage/exchange/journal pipeline they described was never used by any live command path; the machinery that implemented it (`PluginPaths::stage_dir`/`quarantine_dir`/`journal_path`/`settings_quarantine`/`cache_quarantine`/`backups_quarantine`, `validate_txid`, `ensure_not_symlink`, `same_filesystem`) was removed, no replacement test surface exists |
| `MIG-002A` | 15, 17–18 | **amended 2026-08-05**: `tests/cli.rs::binary_setup_migrates_v9_settings_to_strict_v10` and `src/plugin/doctor.rs::clean_removes_only_owned_legacy_and_backups` (destination-local stage/quarantine path tests retired with the machinery above) |
| `MIG-007`–`MIG-019` | 16 | complete v9 fixture matrix and idempotency |
| `MIG-019A`–`MIG-019C` | 16 | byte-preservation tests; **MIG-019A amended 2026-08-05**: `omarchy plugin add` is the install now, no exact-argv test needed |
| `MIG-020`–`MIG-026` | 17–18 | **amended 2026-08-05**: superseded by the delegation model; evidence is `tests/cli.rs` delegation-argv/purge-ordering tests and `tests/dist_tree_validate.rs`, not the retired stage/exchange fault matrix |
| `CLEAN-001`–`CLEAN-007` | 15–16, 19 | ownership fixtures, doctor tests, active legacy scan |
| `BUNDLE-001`–`BUNDLE-007` | 8, 17 | exact bundle tree, private-helper, version tests |
| `BUNDLE-007A` | 15–17 | literal Quattro path and injected-test-root tests |
| `BUNDLE-008`–`BUNDLE-012` | 17, 21 | mode, traversal, reproducibility; **amended 2026-08-05**: `tests/dist_tree_validate.rs` (tree, not archive, inventory) |
| `BUNDLE-012A` | 10, 17 | asset inventory and rendered-icon screenshots |
| `BUNDLE-012B` | 17, 21 | **retired 2026-08-05**: no release-metadata document; `update check` reads `bundle.json` directly, covered by `src/plugin/maintenance.rs` update-check tests |
| `BUNDLE-013`–`BUNDLE-019` | 16–17 | **amended 2026-08-05**: install.sh/bootstrap retired; superseded by native `omarchy plugin add`, no Agent Bar-side test surface remains |
| `BUNDLE-019A` | 16–17 | **retired 2026-08-05**, superseded by `MIG-019A` |
| `BUNDLE-020`–`BUNDLE-025` | 13, 17 | **amended 2026-08-05**: `tests/cli.rs` update-apply/check delegation tests and `src/plugin/maintenance.rs` `reinstallRequired` tests, not the official-release-source tests they replace |
| `BUNDLE-026`–`BUNDLE-032` | 15, 17 | **amended 2026-08-05**: superseded by delegation; `omarchy plugin update`'s own fetch/fast-forward/validate/rollback is out of Agent Bar's test surface, covered here only by `tests/cli.rs` handoff-argv tests |
| `BUNDLE-032A`–`BUNDLE-032E` | 17 | **retired 2026-08-05**: copied-worker/transient-unit/argv0/health-IPC subsystem deleted whole |
| `BUNDLE-032F` | 17–18 | **retired 2026-08-05**: no `listPlugins` absence polling; `omarchy plugin remove` owns removal verification |
| `BUNDLE-032G`–`BUNDLE-032J` | 15, 17–18 | **retired 2026-08-05**: async health poll and worker environment forwarding deleted whole |
| `BUNDLE-032K` | 17 | **retired 2026-08-05**: no post-commit exchange-sibling cleanup; there is no exchange sibling |
| `BUNDLE-033`–`BUNDLE-038` | 13, 18 | standard/purge confirmation tests; **amended 2026-08-05**: `tests/cli.rs` purge-then-delegate tests, transaction machinery retired |
| `BUNDLE-038A` | 18 | **retired 2026-08-05**: no quarantine; `omarchy plugin remove` deletes or, for a non-git tree, backs up |
| `BUNDLE-038B`–`BUNDLE-038C` | 18 | **retired 2026-08-05**: no commit-point/residual-report machinery; `omarchy plugin remove`'s own outcome is the result |
| `BUNDLE-039`–`BUNDLE-042` | 20–22 | version/release docs, checkpoint and remote audit |

## Verification

| Requirements | Tasks | Primary evidence |
| --- | --- | --- |
| `TEST-001`–`TEST-006` | every task; 21 | task gates and final isolated logs |
| `TEST-007`–`TEST-016` | 1–7, 15–18 | backend and transaction suites |
| `TEST-017`–`TEST-029` | 8–14, 21 | QML lint/tests and deterministic screenshots |
| `TEST-030`–`TEST-034` | 19–20 | active legacy/docs/dependency gates |
| `TEST-035`–`TEST-042` | 22 | live backup, QA, screenshots, logs, rollback hashes |

## Documentation and execution controls

| Requirements | Tasks | Primary evidence |
| --- | --- | --- |
| `DOC-001`–`DOC-005` | 20 | English active docs, history allowlist, executable examples |
| `EXEC-001`–`EXEC-010` | all checkpoints | worktree, commits, checkpoint files, Codex reviews, remote audit |
