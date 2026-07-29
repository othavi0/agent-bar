# Settings save deadlock + update metadata redirect — design

Date: 2026-07-29. Approved by the owner. Fixes the two defects reported after
live use of 10.1.0: the Settings Save button silently does nothing, and the
update flow (issue #31) fails against real GitHub.

## Defect 1 — Settings save deadlocks on stdin EOF

Evidence: `~/.config/agent-bar/settings.json` mtime predates every recent save
attempt; toggling a provider and saving changes nothing.

Chain:

1. `agent-bar config apply stdin` reads the payload with `read_to_string`
   (`src/cli/mod.rs`), which blocks until **EOF**.
2. `settingsWriteProcess` in `Service.qml` writes the payload in `onStarted`
   but never closes the write channel, so EOF never arrives.
3. The helper hangs forever; nothing is written; `settingsWriteBusy` stays
   `true`, so every later save is silently rejected until the shell restarts.

`maintenanceHandoffProcess` in the same file already documents the gotcha
("write() alone does not deliver EOF; stdinEnabled=false closes the write
channel") and applies the correct pattern.

Fix (QML only; the helper is correct):

- `settingsWriteProcess.onStarted`: after `write(payload)`, set
  `stdinEnabled = false` to deliver EOF.
- `kickSettingsWrite()`: re-arm `stdinEnabled = true` before starting the
  process so consecutive saves work.

Test: QML source-inspection (tst pattern already used for process wiring)
asserting the settings write process closes stdin after writing and re-arms
before start.

## Defect 2 — Update check dies on GitHub's 302 (issue #31)

`UpdateCheck::run` (`src/plugin/maintenance.rs`) fetches the release
**metadata asset** with a raw `http.get` and requires status 200. GitHub
release assets always answer 302 to the CDN, so every `update check` — and
`update apply`, which re-runs the check — fails with
"metadata download HTTP 302". The archive and checksum downloads already use
`download_with_policy`, which follows ≤5 HTTPS redirects while validating
every hop against the closed host allowlist (`github.com`,
`*.githubusercontent.com`).

Fix (Rust):

- Replace the raw metadata fetch in `UpdateCheck::run` with
  `download_with_policy(http, &meta_asset.browser_download_url)`.
- Provider HTTP (`src/providers/http.rs`, OAuth path) keeps
  `Policy::none` untouched.

Test: fake `ReleaseHttp` where the metadata asset answers 302 with a
`Location` on `objects.githubusercontent.com` followed by 200 — update check
succeeds; a hop outside the allowlist still fails.

## Verification

Full gate (cargo fmt/test/clippy, qmllint, plugin validate, Qt6
qmltestrunner). Live QA on the owner's setup: disable a provider → Save →
chip disappears and `settings.json` changes on disk; `agent-bar update check`
against real GitHub returns the machine document instead of HTTP 302.

Out of scope: any other Settings behavior, the maintenance worker, and the
sidecar-checksum leniency (`if let Ok`) — unchanged.
