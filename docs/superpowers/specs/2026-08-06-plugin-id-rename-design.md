# Rename plugin ID to othavi0.agent-bar

Date: 2026-08-06
Status: approved

## Context

The plugin ID `agent-bar.usage` predates the marketplace submission. Quattro's
ID convention is `<vendor>.<plugin>` (`omarchy.clock`, `omarchy.model-usage`),
so the current ID reads as vendor "agent-bar", plugin "usage". The owner finds
it unprofessional and the marketplace listing (HANCORE-linux/
omarchy-plugin-marketplace#36, approved-for-listing) is not live yet:
the marketplace shows zero published community plugins, so no external
install exists. This is the last cheap moment to rename — after listing,
the ID freezes forever because `omarchy plugin update` cannot rename an
install directory.

## Decision

The plugin ID becomes `othavi0.agent-bar` everywhere current: manifest,
QML `moduleName`, Rust constants (`PLUGIN_ID`, `OMARCHY_PLUGIN_ID`),
delegated update/uninstall commands, workflow paths, tests, contract
(`CLAUDE.md`), and active docs. The dist repository name
(`omarchy-agent-bar`) and all XDG paths (`agent-bar`) are unchanged.

## The one non-mechanical spot

`src/settings/migration.rs` matches plugin entries in **v9-era
`shell.json` data on disk**, which contains the historical ID. That
matcher must keep the literal `agent-bar.usage` via a dedicated
`LEGACY_PLUGIN_ID` constant; renaming it would silently turn the v9
migration into a no-op. The v9 fixtures
(`tests/fixtures/migration/v9/*.json`) represent that on-disk data and
also keep the old ID.

## Consequences for existing installs

`omarchy plugin update` pulls into the existing directory name, so an
install created as `agent-bar.usage` that receives the renamed tree
becomes incoherent (`update check` fails on the dist receipt pluginId
mismatch by design — maintenance.rs guards it). The only existing
install is the owner's machine; the migration is `omarchy plugin remove
agent-bar.usage` + `omarchy plugin add <dist-url>`. Settings survive:
they live under XDG paths that do not carry the plugin ID.

## Out of scope

Historical documents keep the old ID as a build record: `docs/history/`,
`docs/releases/`, `docs/superpowers/`, `CHANGELOG.md`. The v10 specs get
one amendment note at the normative declaration of the ID rather than a
rewrite. The marketplace submission issue is updated after the release
lands on the dist repo.
