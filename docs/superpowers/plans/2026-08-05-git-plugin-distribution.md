# Git-Native Plugin Distribution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert Agent Bar distribution from tarball/`install.sh`/self-update to the native `omarchy plugin add|update|remove` git flow, with a CI-populated distribution repo `othavi0/omarchy-agent-bar`.

**Architecture:** The dev repo stays the build source. CI assembles the plugin tree (existing `agent-bar-bundle assemble`) and pushes it append-only to the distribution repo whose root is the installable plugin. The helper keeps a read-only `update check` (fetching the dist repo's `bundle.json`) and delegates `update apply`/`uninstall` plugin-dir mutation to the omarchy CLI via detached `systemd-run`. The whole tarball/transaction/worker machinery is deleted.

**Tech Stack:** Rust (helper), QML/JS (Quickshell UI), GitHub Actions, Bash (CI only).

**Spec:** `docs/superpowers/specs/2026-08-05-git-plugin-distribution-design.md` — read it first. The audit reports behind it live in the session scratchpad; every line reference below was verified against source on 2026-08-05.

## Global Constraints

- Rust/Cargo and QML only. No Node toolchain.
- No production `unwrap()`/`expect()`.
- Every checkpoint runs: `cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings && git diff --check`. Clippy `-D warnings` means each task must delete the code it orphans (dead code fails the gate) — the task ordering below is built around that.
- QML tasks additionally run the Qt6 binaries (PATH tools are silent stubs):
  `find assets/omarchy -type f -name '*.qml' -exec /usr/lib/qt6/bin/qmllint -I /usr/share/omarchy/shell {} +`, `omarchy plugin validate assets/omarchy`, and
  `QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 /usr/lib/qt6/bin/qmltestrunner -input tests/qml -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt`.
- Commit subjects: English Conventional Commits, ≤50 chars.
- Distribution repo constants (use verbatim):
  - Repo: `othavi0/omarchy-agent-bar`, branch `master`.
  - Install URL: `https://github.com/othavi0/omarchy-agent-bar.git`
  - Discovery URL: `https://raw.githubusercontent.com/othavi0/omarchy-agent-bar/master/bundle.json`
  - Release notes URL pattern: `https://github.com/othavi0/agent-bar/releases/tag/v<version>`
- Never force-push anywhere; the dist repo history is append-only forever.
- Do not mutate live Omarchy paths (`~/.config/omarchy/...`) — Task 10's QA gate is the only exception and is owner-driven.
- Tests use isolated XDG dirs and fake seams; no live network.

## File Structure (locked decomposition)

| Area | Files |
|---|---|
| Update check rewrite | `src/plugin/maintenance.rs`, `tests/fixtures/update-check/*.json`, `tests/update_check_parity.rs` |
| Apply/uninstall delegation | `src/plugin/maintenance.rs`, `src/cli/mod.rs`, `src/cli/grammar.rs`, `src/cli/command.rs`, `tests/cli.rs` |
| Machinery sweep | `src/plugin/transaction.rs` (mostly deleted), `src/plugin/bundle.rs` (ReleaseBuilder deleted), `src/bin/agent-bar-bundle.rs`, `Cargo.toml`, `install.sh` (deleted), `tests/active_legacy_scan.rs`, `tests/agent_bar_bundle_cli.rs` |
| Assemble/dist tree | `src/plugin/bundle.rs`, `assets/omarchy/manifest.json`, new `assets/dist/README.md`, `tests/dist_tree_validate.rs` (new) |
| QML | `assets/omarchy/CoreMaintenance.js`, `Service.qml`, `MaintenanceView.qml`, `tests/qml/tst_Maintenance.qml`, `tst_Service.qml`, `tst_ServiceRaces.qml` |
| CI | `.github/workflows/auto-release.yml`, `docs/dev/releasing.md` |
| Docs/spec sweep | README.md, PRODUCT.md, CONTRIBUTING.md, CLAUDE.md, docs/guide/*, docs/dev/*, docs/specs/v10/{01,03,06,08,README}, REQUIREMENTS_MATRIX.md, CHANGELOG.md |

---

### Task 1: Update check discovers the dist repo receipt

**Files:**
- Modify: `src/plugin/maintenance.rs` (constants at :32-41, `UpdateCheck::run` :451-601, `UpdateCompatible` :106-115, document validate :141-191; delete `GitHubRelease`/`GitHubAsset` :620-632)
- Modify: `tests/fixtures/update-check/available.json`, `up-to-date.json`, `no-compatible.json`; Create: `tests/fixtures/update-check/reinstall-required.json`
- Modify: `tests/update_check_parity.rs`
- Test: unit tests inside `maintenance.rs` (`ScriptedReleaseHttp` seam stays, retargeted)

**Interfaces:**
- Produces: `UpdateCheckDocument` v-next consumed by QML in Task 7:
  top-level fields `schemaVersion:1, checkedAt, current{version,target,omarchyContract,quickshellVersion}, available:bool, reinstallRequired:bool, latestCompatible: null | {version, omarchyContract, minimumQuickshellVersion, releaseNotesUrl}`. No archive fields anywhere.
- Produces: `DIST_RECEIPT_URL` const = the Discovery URL above.
- Consumes: dist `bundle.json` receipt shape (already emitted by `BundleBuilder`): `{schemaVersion:1, pluginId:"agent-bar.usage", version, target, omarchyContract, minimumQuickshellVersion, sourceCommit, files:[...]}`.

- [ ] **Step 1: Read the current code** — `src/plugin/maintenance.rs:29-263` (constants, URL policy), `:451-632` (`UpdateCheck::run`, GitHub structs), `:95-192` (document), and `tests/update_check_parity.rs` in full.
- [ ] **Step 2: Rewrite the three fixtures + add the fourth (failing state).** New `available.json`:

```json
{"schemaVersion":1,"checkedAt":"2026-08-05T12:00:00Z","current":{"version":"10.3.1","target":"x86_64-unknown-linux-gnu","omarchyContract":1,"quickshellVersion":"0.3.0"},"available":true,"reinstallRequired":false,"latestCompatible":{"version":"10.4.0","omarchyContract":1,"minimumQuickshellVersion":"0.3.0","releaseNotesUrl":"https://github.com/othavi0/agent-bar/releases/tag/v10.4.0"}}
```

`up-to-date.json`: same `current` with `"version":"10.4.0"`, `"available":false,"reinstallRequired":false,"latestCompatible":{...same but version 10.4.0...}`. `no-compatible.json`: `"available":false,"reinstallRequired":false,"latestCompatible":null`. `reinstall-required.json`: `"available":false,"reinstallRequired":true,"latestCompatible":null`.
- [ ] **Step 3: Update `tests/update_check_parity.rs`** to require the four fixtures, assert byte-exact round-trip through the Rust serializer, assert `maintenanceUiFromCheck` (CoreMaintenance.js source scan) reads `latestCompatible` and `reinstallRequired`, and assert the strings `archiveUrl`/`checksumUrl`/`archiveSha256`/`sourceCommit` do NOT appear in any fixture.
- [ ] **Step 4: Run — expect FAIL** (`cargo test --test update_check_parity`): serializer still emits archive fields; `reinstallRequired` unknown.
- [ ] **Step 5: Implement.** In `maintenance.rs`: replace `RELEASES_API_URL`/`RELEASE_DOWNLOAD_PREFIX` with `pub const DIST_RECEIPT_URL: &str = "https://raw.githubusercontent.com/othavi0/omarchy-agent-bar/master/bundle.json";`. New parse struct:

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DistReceipt {
    schema_version: u32,
    plugin_id: String,
    version: String,
    target: String,
    omarchy_contract: u32,
    minimum_quickshell_version: String,
}
```

`UpdateCheck::run` becomes: one `http.get(DIST_RECEIPT_URL, [Accept: application/json, User-Agent])` (keep the existing `ReleaseHttp` seam and header-rejection rules; delete redirect-chain machinery — raw.githubusercontent serves directly), parse `DistReceipt`, hard-error on `schema_version != 1` or `plugin_id != PLUGIN_ID` or `target != OFFICIAL_TARGET` or `omarchy_contract != OMARCHY_CONTRACT`; compatibility requires `minimum_quickshell_version <= local quickshell`; `available = receipt.version > current.version` (semver). `releaseNotesUrl` is computed: `format!("https://github.com/othavi0/agent-bar/releases/tag/v{version}")`. `reinstallRequired` = `!paths.plugin_root().join(".git").is_dir()` evaluated in `dispatch_update_check` (pass as input to document build so unit tests can script both values). Shrink `UpdateCompatible` to the four kept fields; extend `validate()` accordingly (notes URL must start with the literal tag prefix). Delete `GitHubRelease`/`GitHubAsset`, `validate_release_asset_url`, `validate_redirect_target`, `validate_redirect_chain`, `MAX_DOWNLOAD_REDIRECTS`, and their unit tests; retarget `ScriptedReleaseHttp` scripts to serve receipt JSON.
- [ ] **Step 6: Run the focused tests, then the full gate.** `cargo test --test update_check_parity && cargo test` then full checkpoint commands. `download_archive` still exists (Task 2 deletes it) — if clippy flags it dead already, move its deletion here and note it in the commit body.
- [ ] **Step 7: Commit** — `feat: update check reads dist repo receipt`

### Task 2: `update apply` delegates to the omarchy CLI

**Files:**
- Modify: `src/cli/grammar.rs:260-278` (`parse_update`), `src/cli/command.rs` (`UpdateCommand`), `src/cli/mod.rs:639-797` (`dispatch_update_apply`, `dispatch_update_interactive`)
- Modify: `src/plugin/maintenance.rs` (delete `download_archive`, `stage_update_bundle` :2232-2256, `apply_version_allowed` :2259-2287, `handoff_update`/`worker_update`/`rollback_update` ~:1249-1670)
- Test: `tests/cli.rs` (`login_config_setup_update_uninstall_doctor_forms` :143-212, `update_apply_rejects_non_strict_semver` :227-238 (deleted), `binary_interactive_update_rejects_non_tty` :655-682)

**Interfaces:**
- Produces: grammar `update` / `update check` / `update apply` (no argument). `update apply` runs preflight then spawns detached: `systemd-run --user --collect --unit=agent-bar-update-<txid>.service -- <abs-omarchy> plugin update agent-bar.usage --yes`, exits 0 on successful spawn. Stdout: one JSON `{"schemaVersion":1,"operation":"updateApply","delegated":true,"unit":"<unit-name>"}` line.
- Consumes: `resolve_absolute_executable` (maintenance.rs:1073-1091, survives), `MaintenanceGate` lock (survives).

- [ ] **Step 1: Rewrite the grammar tests first.** In `tests/cli.rs`: `update apply` with a trailing token now exits 2 ("unexpected argument"); `update apply` alone parses. Delete `update_apply_rejects_non_strict_semver`. Update `binary_interactive_update_rejects_non_tty` expected stderr to name `update check` and `update apply` without a version. Add:

```rust
#[test]
fn update_apply_emits_delegation_document() {
    // omarchy resolved via a fake PATH shim in a tempdir that records argv;
    // systemd-run replaced by a recording shim the same way.
    // Assert stdout is one JSON object with operation=="updateApply",
    // delegated==true, and the recorded argv ends with
    // ["plugin","update","agent-bar.usage","--yes"].
}
```

(Build the shims as the existing cli.rs tests build fake plugin trees — executable shell scripts in a temp dir prepended to PATH.)
- [ ] **Step 2: Run — expect FAIL** (grammar still requires semver; no delegation document).
- [ ] **Step 3: Implement.** Grammar: `parse_update` accepts `[]` (interactive), `["check"]`, `["apply"]`; anything after `apply` is a grammar error. `UpdateCommand::Apply` loses its `ReleaseVersion` payload. `dispatch_update_apply`: take the gate lock, resolve `omarchy` and `systemd-run` absolute paths, build argv exactly `[systemd_run, "--user", "--collect", format!("--unit=agent-bar-update-{txid}.service"), "--", omarchy, "plugin", "update", "agent-bar.usage", "--yes"]`, spawn, print the delegation document. Delete the freshness re-check, download, staging, worker handoff for update, and `apply_version_allowed`. Keep `dispatch_update_interactive` as a two-line stderr help pointing at `update check`/`update apply`.
- [ ] **Step 4: Run focused tests, then the full gate.** Delete any code clippy now flags dead that belongs to the update-apply path only (worker_update chain). Uninstall's worker path must still compile — only touch shared helpers if the compiler proves them unused.
- [ ] **Step 5: Commit** — `feat: delegate update apply to omarchy cli`

### Task 3: `uninstall` = own-state purge + delegated remove

**Files:**
- Modify: `src/cli/mod.rs:461-604`, `src/plugin/maintenance.rs` (`handoff_uninstall`, `worker_uninstall` :1673-1963, `poll_uninstall_absence` :2105-2152 — deleted; confirmation :856-937 — kept), `src/plugin/transaction.rs` (`remove_exact_plugin_entries` :437-475 — deleted; shell.json is omarchy's now)
- Test: `tests/cli.rs`

**Interfaces:**
- Produces: `uninstall [purge]` keeps the exact stdin confirmation contract (`{schemaVersion:1, operation:"uninstall", confirmed:true, purgeSettingsAndBackups:bool}`) and the TTY phrase. Behavior: (with purge) delete `$XDG_CONFIG_HOME/agent-bar/settings.json`, `$XDG_CACHE_HOME/agent-bar/`, `$XDG_STATE_HOME/agent-bar/` (backups, notification state, locks); always: spawn detached `systemd-run --user --collect --unit=agent-bar-remove-<txid>.service -- <abs-omarchy> plugin remove agent-bar.usage --yes`; stdout one JSON `{"schemaVersion":1,"operation":"uninstall","purged":<bool>,"delegated":true,"unit":"..."}`.
- Consumes: shims pattern from Task 2.

- [ ] **Step 1: Write the failing tests.** Rework the uninstall cases in `tests/cli.rs`: confirmation gates unchanged (mismatched purge flag still rejected); with isolated XDG dirs pre-populated, `uninstall purge` (confirmation on stdin) removes the three XDG roots and records the delegated argv ending `["plugin","remove","agent-bar.usage","--yes"]`; without `purge` the XDG roots survive. Assert the helper never touches a fake `shell.json` placed in the fake `$HOME/.config/omarchy/`.
- [ ] **Step 2: Run — expect FAIL.**
- [ ] **Step 3: Implement.** Purge is plain `fs::remove_dir_all`/`remove_file` under the gate lock (settings/cache/state live outside the plugin dir; no quarantine, no journal — the operations are idempotent and the delegated remove is fire-and-forget). Delete the uninstall worker chain, `remove_exact_plugin_entries`, shell.json backup logic, and `poll_uninstall_absence`. With both worker paths gone, delete the shared worker core (`install_worker_copy`, `is_maintenance_worker_exe`, `MAINTENANCE_WORKER_NAME`, `run_worker_from_journal`, `WorkerDeadlines`, `systemd_run_argv` if unused after Tasks 2-3 built their own argv, `TransactionJournal`).
- [ ] **Step 4: Full gate.**
- [ ] **Step 5: Commit** — `feat: narrow uninstall to purge plus delegation`

### Task 4: `setup` becomes migration-only

**Files:**
- Modify: `src/cli/grammar.rs:226-259`, `src/cli/mod.rs:247-387` (`resolve_plugin_source_root` deleted, `dispatch_setup` keeps only `migrate_live_paths`)
- Test: `tests/cli.rs:364-611`

**Interfaces:**
- Produces: `setup` (no subcommand) runs settings migration only, exits 0, prints its existing migration JSON summary. `setup plugins-dir <path>` is a grammar error ("unknown setup argument").

- [ ] **Step 1: Rewrite the three setup tests.** Delete `binary_setup_plugins_dir_installs_from_local_plugin_tree` and `binary_setup_plugins_dir_validates_parent_versus_plugin_root`. Retarget `binary_setup_migrates_v9_settings_to_strict_v10` at plain `setup` (same fixtures, same assertions about v10 schema, backup existence, shell.json untouched). Add: `setup plugins-dir /tmp/x` exits 2.
- [ ] **Step 2: Run — expect FAIL.**
- [ ] **Step 3: Implement** (grammar + dispatch strip; delete the `Transaction`-based tree copy and `OmarchyClient::activate` call from setup).
- [ ] **Step 4: Full gate.** — **Step 5: Commit** — `feat: reduce setup to settings migration`

### Task 5: Sweep the tarball machinery, deps, and install.sh

**Files:**
- Delete: `install.sh`, `tests/agent_bar_bundle_cli.rs` (recreate minimal, below)
- Modify: `src/plugin/transaction.rs` (delete `Transaction`/`TransactionPlan`/`TxStep`/`TxFailPoint`/`exchange_paths`/`quarantine_rename`/`inspect_tar_zst_entries`; keep only what still compiles into maintenance — expected survivor: nothing; delete the file if empty and drop the module), `src/plugin/bundle.rs` (delete `ReleaseBuilder`, `ReleaseMetadata`, `write_tar_zst`, `extract_bundle_archive`, `validate_archive_bytes`), `src/bin/agent-bar-bundle.rs` (keep `assemble` only), `Cargo.toml` (drop `tar`, `zstd`; reword `reqwest` comment to "Claude HTTP collector and update check"), `tests/active_legacy_scan.rs`
- Test: `tests/active_legacy_scan.rs`, new slim `tests/agent_bar_bundle_cli.rs`

**Interfaces:**
- Produces: `agent-bar-bundle assemble output <dir> source-commit <hex>` is the binary's only verb.

- [ ] **Step 1: Update the enforcement tests first.** In `tests/active_legacy_scan.rs`: add `install.sh` to `LOCKED_DELETION_PATHS` (:16-61); delete the whole `install.sh`-reading block in `active_legacy_scan_cargo_and_install_contract` (:518-538); remove `tar`/`zstd` from `required_dependency_owners()` (:356-358); add forbidden tokens: `"tar.zst"`, `"releases/download/"`, `"agent-bar-maintenance-worker"`, `"RELEASES_API_URL"` (the negative-sentence allowance :147-169 keeps docs able to say "the tarball flow was removed"). New `tests/agent_bar_bundle_cli.rs`: `help` names only `assemble`; `release` exits 2; `assemble` keyword validation as today.
- [ ] **Step 2: Run — expect FAIL** (install.sh still exists, tokens still present).
- [ ] **Step 3: Delete.** `git rm install.sh`; strip the Rust surfaces listed above; `cargo remove tar zstd` (edit Cargo.toml directly, keep comment style). Chase every compile error to its dead root — no `#[allow(dead_code)]`, no stubs (CLAUDE.md: no dormant machinery).
- [ ] **Step 4: Full gate** — `dependencies_are_actually_used_in_src` now enforces the Cargo/source symmetry.
- [ ] **Step 5: Commit** — `feat: remove tarball distribution machinery`

### Task 6: Dist tree completeness + validate mirror

**Files:**
- Modify: `src/plugin/bundle.rs` (`BundleBuilder::assemble` :299-361; `validate_tree` learns to skip a root `.git`), `assets/omarchy/manifest.json`
- Create: `assets/dist/README.md` (dist repo user README, English)
- Test: Create `tests/dist_tree_validate.rs`

**Interfaces:**
- Produces: `assemble` output additionally contains `README.md` (from `assets/dist/README.md`), `LICENSE` (repo root copy), `preview.png` (from `docs/media/demo.png`), all 0644, all listed in `bundle.json`. Manifest gains `"defaultSection": "right"` inside the existing `barWidget` object.

- [ ] **Step 1: Write `tests/dist_tree_validate.rs` (failing).** Build a fake source layout in a tempdir (tiny fake ELF bytes as `bin/agent-bar` input, the real `assets/omarchy` copied in, fake LICENSE/README/preview), run `BundleBuilder::assemble`, then assert a faithful mirror of `omarchy-plugin-validate` over the output: `manifest.json` at root parses; `schemaVersion == 1`; id regex `^[A-Za-z0-9][A-Za-z0-9._-]*$`, not `omarchy.*`; `kinds` non-empty; every `entryPoints.*` relative, no `..`, exists; `barWidget.defaultSection` ∈ {left,center,right}; `find`-equivalent walk finds zero symlinks; `README.md`, `LICENSE`, `preview.png` exist at root and appear in `bundle.json.files`. Second test: drop a `.git/config` file into an assembled tree, `validate_tree` still passes; add a symlink, it fails.
- [ ] **Step 2: Run — expect FAIL.** — **Step 3: Implement** (assemble inputs + receipt entries + `.git` skip + manifest edit + write `assets/dist/README.md`: short — what it is, `omarchy plugin add https://github.com/othavi0/omarchy-agent-bar.git`, placement prompt note, update via `omarchy plugin update agent-bar.usage`, remove via `omarchy plugin remove agent-bar.usage`, link to `othavi0/agent-bar` for issues/source, MIT).
- [ ] **Step 4: Full gate + QML gate** (manifest changed → `omarchy plugin validate assets/omarchy`).
- [ ] **Step 5: Commit** — `feat: assemble full dist tree with metadata`

### Task 7: QML maintenance rework

**Files:**
- Modify: `assets/omarchy/CoreMaintenance.js` (:59-74 argv builders, :122-165 `maintenanceUiFromCheck`, :181-186 `updateConfirmMessage`), `assets/omarchy/Service.qml` (:676-706 detach, :823-840 stdin block — stdin stays, update path never used it), `assets/omarchy/MaintenanceView.qml` (message rendering for the new phase)
- Test: `tests/qml/tst_Maintenance.qml`, `tst_Service.qml`, `tst_ServiceRaces.qml`

**Interfaces:**
- Consumes: Task 1 document (`reinstallRequired`), Task 2 grammar (`update apply`, no version).
- Produces: `updateApplyArgv(helperPath)` → `[helper, "update", "apply"]`; `maintenanceUiFromCheck` new phase `"reinstall_required"`; confirm message: `"Updates <a> → <b>. Settings stay. Fast-forwards to the latest release; a failed validation rolls back."`

- [ ] **Step 1: Update the QML tests first.** In `tst_Maintenance.qml`: `updateApplyArgv("h")` equals `["h","update","apply"]` (version argument gone — passing one is not part of the signature anymore); a check document with `reinstallRequired:true` yields phase `"reinstall_required"` and a message containing both `omarchy plugin remove agent-bar.usage` and `omarchy plugin add https://github.com/othavi0/omarchy-agent-bar.git`; the confirm-message test pins the new sentence. Uninstall argv/confirmation tests stay as-is (contract unchanged). `tst_Service.qml`/`tst_ServiceRaces.qml`: lane names and drain rules unchanged — only touch them if the implementation renames something (it should not).
- [ ] **Step 2: Run qmltestrunner — expect FAIL.**
- [ ] **Step 3: Implement.** `updateApplyArgv(helperPath)` drops the version parameter and null-guard; `Service.qml` call site passes no version. `maintenanceUiFromCheck`: before the `available` branch, `if (doc.reinstallRequired === true) { next.phase = "reinstall_required"; next.targetVersion = ""; next.releaseNotesUrl = ""; next.message = "Installed without git. Run: omarchy plugin remove agent-bar.usage, then omarchy plugin add https://github.com/othavi0/omarchy-agent-bar.git"; return next }`. `MaintenanceView.qml`: the message lane already renders `ui.message`; hide the Update button for the new phase (it only shows for `update_available` — verify, no change expected).
- [ ] **Step 4: Full QML gate + `cargo test`** (`update_check_parity` scans CoreMaintenance.js; `gui_vocabulary` bans internal words — keep "git" out of user copy except in the literal commands above, which are commands, and re-run to confirm the scanner accepts them; if it rejects, move the commands into the view as monospace command text and keep the message generic).
- [ ] **Step 5: Commit** — `feat: maintenance ui delegates via omarchy cli`

### Task 8: CI dist-push

**Files:**
- Modify: `.github/workflows/auto-release.yml` (delete "Build release artifacts" :106-125 and the asset list from "Push and publish" :127-141)
- Modify: `docs/dev/releasing.md`

**Interfaces:**
- Consumes: assembled tree at `target/release/agent-bar.usage` (Task 6 makes it complete).
- Produces: dist repo commit per release; product release keeps tag + notes, no assets.

- [ ] **Step 1: Rewrite the workflow tail.** After the "Bundle inventory" step:

```yaml
      - name: Push plugin tree to distribution repo
        env:
          DIST_DEPLOY_KEY: ${{ secrets.OMARCHY_AGENT_BAR_DEPLOY_KEY }}
          VERSION: ${{ steps.cut.outputs.version }}
        run: |
          set -euo pipefail
          mkdir -p ~/.ssh
          printf '%s\n' "$DIST_DEPLOY_KEY" > ~/.ssh/dist_key
          chmod 600 ~/.ssh/dist_key
          export GIT_SSH_COMMAND="ssh -i ~/.ssh/dist_key -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new"
          SOURCE_COMMIT="$(git rev-parse HEAD)"
          git clone --depth 1 git@github.com:othavi0/omarchy-agent-bar.git dist
          find dist -mindepth 1 -maxdepth 1 -not -name .git -exec rm -rf {} +
          cp -a target/release/agent-bar.usage/. dist/
          git -C dist config user.name "github-actions[bot]"
          git -C dist config user.email "41898282+github-actions[bot]@users.noreply.github.com"
          git -C dist add -A
          git -C dist commit -m "release: v${VERSION} (agent-bar@${SOURCE_COMMIT})"
          git -C dist push origin HEAD:master

      - name: Push and publish release
        env:
          GH_TOKEN: ${{ github.token }}
          VERSION: ${{ steps.cut.outputs.version }}
        run: |
          set -euo pipefail
          git push origin HEAD:master "refs/tags/v${VERSION}"
          gh release create "v${VERSION}" \
            --title "Agent Bar ${VERSION}" \
            --notes-file "docs/releases/${VERSION}.md"
```

Note the ordering: dist push happens BEFORE the product push/release, so a failed dist push aborts the release entirely (spec's loud-failure rule). A `--depth 1` clone is fine for pushing one appended commit.
- [ ] **Step 2: Lint** — `actionlint` if available, else YAML parse via `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/auto-release.yml'))"`. ShellCheck the embedded script (repo rule for shell changes).
- [ ] **Step 3: Rewrite `docs/dev/releasing.md`**: pipeline description, the deploy-key setup runbook (generate `ssh-keygen -t ed25519 -f dist_key -N ""`, public half → dist repo Settings→Deploy keys with write access, private half → product repo secret `OMARCHY_AGENT_BAR_DEPLOY_KEY`), the append-only rule, and branch-protection note (deny force pushes on dist master).
- [ ] **Step 4: Full gate** (`active_docs` parses doc examples). — **Step 5: Commit** — `feat: publish releases to dist repo`

### Task 9: Docs and spec amendments

**Files:** (every location verified in the audit)
- Modify: `README.md` (Install section → `omarchy plugin add` one-liner + placement note; Settings paragraph), `PRODUCT.md` (:17, :25, :57), `CONTRIBUTING.md` (:93-97), `CLAUDE.md` (:40-41 boundaries, :65 rescan→`omarchy plugin update`, :119-121 verification pointer), `docs/README.md` (:7-8, :23-24), `docs/guide/commands.md` (Plugin integration/Update/Uninstall sections), `docs/guide/runtime.md` (:13-14, :23-30), `docs/guide/troubleshooting.md` (:92-101), `docs/guide/integration.md`, `docs/dev/architecture.md` (:124-145), `docs/specs/v10/01-product-contract.md` (PROD-007/010, Maintain journey), `03-cli-and-json-contract.md` (grammar + CLI-024..031), `06-migration-and-legacy-removal.md` (MIG-020..026), `08-plugin-bundle-and-release.md` (near-total: release-files/update-transaction/BUNDLE-013 out; dist-repo model in), `docs/specs/v10/README.md` (status + superseding note pointing at the 2026-08-05 design spec), `REQUIREMENTS_MATRIX.md` (re-map BUNDLE rows), `CHANGELOG.md` (Unreleased entry: distribution conversion + migration instructions)

- [ ] **Step 1: Sweep with the gates as the test.** Write all edits, then run `cargo test --test active_docs --test active_language --test active_legacy_scan --test cli_vocabulary`. Every embedded `agent-bar` command example must parse under the new grammar; every stale `install.sh`/tarball instruction now trips the new forbidden tokens unless written as a negative-removal sentence.
- [ ] **Step 2: Fix until green, then full gate.**
- [ ] **Step 3: Commit** — `docs: describe git-native distribution`

### Task 10: Dist repo bootstrap + live QA (owner-gated)

No repo files. Owner-driven, in order:
- [ ] Create `othavi0/omarchy-agent-bar` (public, empty, default branch `master`, description "Omarchy bar widget for AI quota — install artifact of othavi0/agent-bar"), enable branch protection: disallow force pushes.
- [ ] Generate the deploy key pair, install both halves (releasing.md runbook from Task 8).
- [ ] Merge the conversion to master → auto-release runs → verify the dist repo received `release: vX.Y.Z` with a complete tree (manifest at root, ELF present and 0755: `git ls-tree -r master | grep 100755`).
- [ ] Live QA on this machine (the authorized live-path exception): migrate the local tarball install exactly as the README instructs (`omarchy plugin remove agent-bar.usage` → `omarchy plugin add https://github.com/othavi0/omarchy-agent-bar.git` → placement prompt → enable); verify chips, popup, Settings; run `omarchy plugin update agent-bar.usage` (expect "up to date"); verify Settings→Check for updates renders correctly against the live dist repo.

### Task 11: Marketplace submission (owner-gated)

- [ ] Draft the issue body per `SUBMISSION.md`: title `[Plugin]: Agent Bar`, Repository URL `https://github.com/othavi0/omarchy-agent-bar`, Category `Widgets`, Tags `ai`, `bar`, `quickshell`, maintainer notes (one paragraph), full checklist.
- [ ] Show the complete title/body to the owner; create the issue with `gh issue create --repo HANCORE-linux/omarchy-plugin-marketplace ...` ONLY after explicit approval (marketplace rule + repo publish gate).

---

## Self-review notes

- Spec coverage: every spec section maps to a task (discovery→1, apply→2, uninstall→3, setup→4, deletions→5, dist tree/manifest→6, QML→7, CI/auth→8, docs/contract→9, migration/QA→10, marketplace→11). Version-pinning removal is implicit in Tasks 5/9 (env var dies with install.sh; docs stop mentioning it).
- Clippy-driven ordering: Tasks 2→3 delete their own orphans; Task 5 sweeps what remains. If an earlier task's build fails on dead code that a later task owns, delete it early and say so in the commit body — never suppress.
- The `update check` HTTP seam keeps `ReleaseHttp` naming until Task 5; if the name trips the new forbidden tokens, rename to `ReceiptHttp` in Task 5 with the tests.
