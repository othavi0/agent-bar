# v11 CLI and Installer Copy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Take parser vocabulary out of the CLI's error messages, collapse the messages that exist twice, and rewrite the installer and the interactive update prompt in the voice a person reads once, at the moment they are installing the product for the first time.

**Architecture:** Two different rulebooks, deliberately. `src/cli/` keeps Unix convention — lower case, no trailing period, no product-name prefix — and only three things change there: the word `clause`, messages duplicated across the parse and validate paths, and a short enumerated set that names a problem without its fix. `install.sh` and the interactive `update` prompt are the exception the copy design carves out: they are read by a human installing the product, so they follow the full GUI voice. A narrow guard bans `clause` from CLI string literals; the GUI vocabulary guard is deliberately **not** extended here.

**Tech Stack:** Rust (`src/cli/`), Bash (`install.sh`), no QML behaviour change.

## Global Constraints

- Contract: `CLAUDE.md` at repo root; product contract: `docs/specs/v10/` plus the approved design `docs/superpowers/specs/2026-07-30-copy-and-language-design.md` (§4 voice, §7 CLI rules, §8 test impact). This plan is §9's phase 4, the last one.
- **`install.sh` stays Bash and stays argv-safe.** Never introduce `eval`, `sh -c`, `bash -lc`, or `cmd="$*"`. `scripts/agent-bar-open-terminal` is not touched at all.
- Rust: no production `unwrap()`/`expect()`. Test code may use them.
- All copy is English. The language gate flags alphabetic non-ASCII only. Keep `install.sh` output ASCII — it runs before any font or locale is known, so prefer `...` over `…` there. The Rust CLI may keep whatever it already uses.
- `cargo test` accepts ONE filter per invocation. Baseline at plan-06 start: **301 Rust tests / 18 suites, 231 QML / 0 failed**. Known flake: `binary_interactive_update_rejects_non_tty` (`ExecutableFileBusy`) — retry once, pre-existing.
- `qmltestrunner` from `PATH` is Qt 5 and fails SILENTLY; the Qt 6 binary and both env vars are mandatory. `qmllint` from `PATH` is a stub reporting version `1.0`; use `/usr/lib/qt6/bin/qmllint` and judge it by output, never exit code.
- Checkpoint gates: `cargo fmt --check` · `cargo test` · `cargo clippy --all-targets -- -D warnings` · `git diff --check` · ShellCheck on `install.sh` · the Qt 6 QML gates · `omarchy plugin validate` · `scripts/verify-v10-ui`.
- Commits: English Conventional Commit subject ≤ 50 chars. Never any AI-attribution text anywhere.
- The plan-01…05 defect pattern applies: *source that reads correctly and behaves differently at runtime.* Resolve every claim against the real file before trusting it.

## Measured facts (2026-08-01, this machine)

1. **`clause` is both a message word and an identifier, and only the message word is in scope.** `src/cli/grammar.rs` contains nine occurrences: six inside format strings (`:99` `unknown status clause`, `:106` `duplicate status clause`, `:243` `unknown setup clause`, `:268` `unknown update clause`, `:278` `unknown uninstall clause`, `:289` `unknown doctor clause`) and three as code (`:92` `let clause = match word`, `:103` `let idx = clause as usize`, `:120` `match clause`), plus the `StatusClause` enum. The enum and the binding are code vocabulary and stay. A guard that bans the token file-wide would fail on them, so the guard must scan string literals only.
2. **No test asserts a `clause` message.** `tests/cli.rs` has four hits and all four are a test-local `clauses` array (`:45-46,69-70`), not an assertion on the text. The rename breaks no test.
3. **Four messages exist twice, and two of those cross a module boundary.** `setup plugins-dir path must be absolute` is at `grammar.rs:229` **and** `mod.rs:104`; `setup plugins-dir path must be the parent directory, not the plugin root` is at `grammar.rs:234` **and** `mod.rs:113`. Two more repeat inside `grammar.rs` alone: `config apply requires stdin, file <path>, or json <value>` (`:189`, `:208`) and `setup plugins-dir requires a path` (`:221`, `:240`). A fifth repeats three times inside `mod.rs`: the long `setup requires a complete plugin tree at <plugin-root>/bin/agent-bar; use install.sh for first bootstrap from a release archive` (`:250`, `:259`, `:264`).
4. **`missing value for status {word}` is written twice** (`grammar.rs:114` and `:116-118`) for two different conditions — a missing token and a token that looks like a flag. Same text, same meaning to the reader; one definition serves both.
5. **`install.sh` has four output helpers and 49 call-site strings.** `log`/`ok`/`warn`/`die` at `:43-46` prefix with `==> `, `OK  `, `!   `, `ERR `. Those prefixes are structure, not copy, and stay. `--help` does not use them: it prints the file's own header comment through `sed -n '2,/^$/p' "$0" | sed 's/^# \?//'` (`:33`), so the header block at `:2-15` is a second output surface that must be edited as copy.
6. **`bundle` is not internal vocabulary in the installer.** The GUI vocabulary guard bans it because a popup user never sees an archive. The installer user downloads, verifies, and extracts one, and the v10 contract itself names the artifact a plugin bundle throughout `08-plugin-bundle-and-release.md`. `bundle` therefore stays in `install.sh` wherever it names the real artifact or the `bundle.json` receipt. Do not sweep it.
7. **The two confirmation phrases are load-bearing and frozen.** `update agent-bar` (`mod.rs:193,201`) and `uninstall agent-bar` are what the user must type to proceed, pinned by `mod.rs:1050-1052` and `:1101` and by `tests/cli.rs:678-680`. They are a safety mechanism, not copy. The sentence around them may change; the phrase may not.
8. **Test-pinned CLI strings are few.** `tests/cli.rs` pins `Quickshell plugin` and `diagnostics` (`:359-360`), `complete plugin tree` (`:404`), `doctor scan`/`read-only` (`:626-627`), `doctor clean`/`removed:` (`:649-650`), `update check`/`update apply` (`:678-680`), and the exact `version` stdout (`:293-311`). Inline `mod.rs` tests pin `Agent Bar is up to date.` exactly (`:1026-1029`) and the two typed phrases. Nothing pins an `install.sh` string: no test executes the installer, and `tests/active_legacy_scan.rs:518-538` only asserts negatives about it.
9. **`tests/active_docs.rs` structurally pins every `agent-bar …` argv appearing in fenced code blocks** of `README.md`, `docs/commands.md`, and the other active docs — they must still parse. Help *text* is not pinned, but the grammar the help text advertises is.
10. **One item is carried from plan 05.** `assets/omarchy/CoreMaintenance.js:219` sets `next.message = "Click Uninstall again to confirm."`, which nothing renders: the dialog binds its own message from `ui.uninstallArmed`/`ui.purgeSettings`, and the only reader of `ui.message` sits behind the dialog's `z: 100` scrim. Two reviewers confirmed it is dead. It is one line and the v10 contract forbids dormant code.

## File Structure

- Modify: `src/cli/grammar.rs` — six `clause` messages become `argument`; four repeated messages become module constants.
- Modify: `src/cli/mod.rs` — consume `grammar`'s two shared constants; collapse the three-times-repeated setup message; the enumerated problem-without-fix rewrites; the interactive update prompt.
- Create: `tests/cli_vocabulary.rs` — bans `clause` from `src/cli/**` string literals.
- Modify: `install.sh` — the header banner and all 49 call-site strings.
- Modify: `assets/omarchy/CoreMaintenance.js` — delete the dead arming message (plan-05 carry-over).
- Modify: `tests/cli.rs` only if an assertion genuinely breaks; the measured expectation is that none does.
- Modify: `docs/superpowers/plans/2026-08-01-v11-06-cli-and-installer-copy.md` (this file — execution record).

## Seams (do not cross)

- **No grammar changes.** Every command, clause keyword, flag, and provider id stays exactly as it parses today. This plan edits messages, not the language. `tests/active_docs.rs` would catch a grammar change through the doc examples, but the rule is stronger than the test: do not touch parsing.
- **No behaviour changes.** Exit codes, TTY gating, confirmation phrases, and dispatch order are untouched.
- **The GUI vocabulary guard is not extended.** `tests/gui_vocabulary.rs` keeps scanning `assets/omarchy/**` only. `bundle`, `collect`, and `schema` are legitimate CLI and installer vocabulary; only `clause` gets a CLI guard, because §7.1 names it specifically.
- `src/bin/agent-bar-bundle.rs` is an internal build tool, not the shipped helper. Its strings are out of scope.

---

### Task 1: Take parser vocabulary out of the CLI

**Files:**
- Modify: `src/cli/grammar.rs` (messages at :99, :106, :243, :268, :278, :289)
- Create: `tests/cli_vocabulary.rs`

**Interfaces:** produces no API. The six messages change text only.

- [ ] **Step 1: Write the failing guard**

Create `tests/cli_vocabulary.rs`:

```rust
//! `clause` is parser vocabulary. It names a concept in the grammar's own
//! implementation, not anything the person typing the command can see, and it
//! leaked into six error messages. The word stays in the code — `StatusClause`
//! and its bindings are exactly the kind of internal name that belongs there —
//! so this guard scans string literals only.
//!
//! Scope is `src/cli/**`. The GUI has its own, wider guard in
//! `tests/gui_vocabulary.rs`; the two lists differ on purpose, because
//! `bundle` and `schema` name real things a CLI user deals with.

use std::fs;
use std::path::PathBuf;

/// Double-quoted spans, skipping whole-line `//` comments. Crude on purpose:
/// the CLI's messages are plain literals, never built by a macro.
fn string_literals(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in source.lines() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        for piece in line.split('"').skip(1).step_by(2) {
            out.push(piece.to_owned());
        }
    }
    out
}

#[test]
fn cli_messages_do_not_say_clause() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli");
    let mut violations = Vec::new();
    let entries = fs::read_dir(&root).expect("read src/cli");
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"))
        .collect();
    files.sort();
    assert!(
        files.len() >= 4,
        "expected the cli module tree, found {} files",
        files.len()
    );
    for path in files {
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_owned();
        for literal in string_literals(&source) {
            if literal
                .to_lowercase()
                .split(|c: char| !c.is_ascii_alphabetic())
                .any(|token| token == "clause")
            {
                violations.push(format!("{name}: {literal:?}"));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "CLI messages still say clause ({}):\n  - {}",
        violations.len(),
        violations.join("\n  - ")
    );
}
```

- [ ] **Step 2: Run it and confirm the intended failure**

Run: `cargo test --test cli_vocabulary`
Expected: FAIL, listing the six `grammar.rs` messages. Paste the list into the report — it is the evidence the guard has teeth.

- [ ] **Step 3: Implement**

In `src/cli/grammar.rs`, change only these six format strings. Leave `StatusClause`, `let clause`, `let idx = clause as usize`, and `match clause` exactly as they are.

| Line | Now | New |
| --- | --- | --- |
| :99 | `unknown status clause '{other}'` | `unknown argument '{other}' for status` |
| :106 | `duplicate status clause '{word}'` | `repeated argument '{word}' for status` |
| :243 | `unknown setup clause '{other}'` | `unknown argument '{other}' for setup` |
| :268 | `unknown update clause '{other}'` | `unknown argument '{other}' for update` |
| :278 | `unknown uninstall clause '{other}'` | `unknown argument '{other}' for uninstall` |
| :289 | `unknown doctor clause '{other}'` | `unknown argument '{other}' for doctor` |

The `unknown argument '{x}' for <command>` shape is the copy design's own example for the first of these; the other five follow it so the CLI speaks one pattern rather than six.

- [ ] **Step 4: Run the guard and the CLI suites**

`cargo test --test cli_vocabulary` (expect 1 passed), then `cargo test --test cli` and `cargo test --lib cli`. Measured expectation: no assertion breaks, because no test pins a `clause` message. If one does break, report it before changing it.

- [ ] **Step 5: Full Rust gates** — `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `git diff --check`. Expect **302 across 19 suites** (301 plus the new suite); report the real number.

- [ ] **Step 6: Commit**

```bash
git add src/cli/grammar.rs tests/cli_vocabulary.rs
git commit -m "feat: say argument, not clause, in cli errors"
```

---

### Task 2: One message, one definition

**Files:**
- Modify: `src/cli/grammar.rs` (:189, :208, :221, :229, :234, :240, and the two `missing value for status` sites at :114 and :116-118)
- Modify: `src/cli/mod.rs` (:104, :113, :250, :259, :264)

**Interfaces:**
- Produces `pub(crate)` constants in `src/cli/grammar.rs`, consumed by `src/cli/mod.rs`:
  - `SETUP_PLUGINS_DIR_ABSOLUTE: &str`
  - `SETUP_PLUGINS_DIR_NOT_PLUGIN_ROOT: &str`
- Produces two module-private constants used only inside `grammar.rs`, and one inside `mod.rs`.

- [ ] **Step 1: Write the failing test**

Add to `tests/cli_vocabulary.rs` (it already walks `src/cli/**`, so this is where a duplication guard belongs):

```rust
/// A message that exists twice drifts: one copy gets fixed, the other does
/// not, and the user sees two wordings for one condition depending on which
/// code path noticed. These four were duplicated across the parse and
/// validate paths before this guard existed.
#[test]
fn cli_messages_are_defined_once() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli");
    let watched = [
        "setup plugins-dir path must be absolute",
        "setup plugins-dir path must be the parent directory, not the plugin root",
        "config apply requires stdin, file <path>, or json <value>",
        "setup plugins-dir requires a path",
        "setup requires a complete plugin tree",
    ];
    let mut violations = Vec::new();
    let entries = fs::read_dir(&root).expect("read src/cli");
    let mut totals = vec![0usize; watched.len()];
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rs"))
        .collect();
    files.sort();
    for path in &files {
        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };
        // Count only literal occurrences, so the single `const` definition
        // counts once and every use site counts zero.
        for (idx, needle) in watched.iter().enumerate() {
            totals[idx] += source.matches(needle).count();
        }
    }
    for (idx, needle) in watched.iter().enumerate() {
        if totals[idx] > 1 {
            violations.push(format!("{needle:?} appears {} times", totals[idx]));
        }
    }
    assert!(
        violations.is_empty(),
        "CLI messages defined more than once ({}):\n  - {}",
        violations.len(),
        violations.join("\n  - ")
    );
}
```

- [ ] **Step 2: Run it and confirm the intended failure**

Run: `cargo test --test cli_vocabulary`
Expected: this test FAILS listing all five, with counts 2, 2, 2, 2, 3. Record the counts.

- [ ] **Step 3: Implement in `grammar.rs`**

Add near the top of `src/cli/grammar.rs`, after the imports:

```rust
/// Shared with `super::validate_plugins_dir`, which re-checks the same two
/// conditions after the filesystem is consulted. One definition means the
/// parse path and the validate path can never disagree about the wording.
pub(crate) const SETUP_PLUGINS_DIR_ABSOLUTE: &str = "setup plugins-dir path must be absolute";
pub(crate) const SETUP_PLUGINS_DIR_NOT_PLUGIN_ROOT: &str =
    "setup plugins-dir path must be the parent directory, not the plugin root";

const CONFIG_APPLY_USAGE: &str = "config apply requires stdin, file <path>, or json <value>";
const SETUP_PLUGINS_DIR_REQUIRES_PATH: &str = "setup plugins-dir requires a path";
```

Replace the literals at `:189`, `:208` with `CONFIG_APPLY_USAGE`; at `:221`, `:240` with `SETUP_PLUGINS_DIR_REQUIRES_PATH`; at `:229` with `SETUP_PLUGINS_DIR_ABSOLUTE`; at `:234` with `SETUP_PLUGINS_DIR_NOT_PLUGIN_ROOT`.

For the two `missing value for status {word}` sites (`:114` and `:116-118`), both take the same `word`, so give them one definition:

```rust
fn missing_status_value(word: &str) -> CliFailure {
    CliFailure::grammar(format!("missing value for status {word}"))
}
```

and call it from both.

- [ ] **Step 4: Implement in `mod.rs`**

Replace the literal at `:104` with `grammar::SETUP_PLUGINS_DIR_ABSOLUTE` and at `:113` with `grammar::SETUP_PLUGINS_DIR_NOT_PLUGIN_ROOT`, adjusting the `use` at the top of the file if `grammar` is not already in scope by that path.

Collapse the three copies of the long setup message (`:250`, `:259`, `:264`) into one module constant:

```rust
/// Three call sites reach this condition — a missing plugin root, a missing
/// helper, and a non-executable helper — and all three want the same sentence.
const SETUP_REQUIRES_PLUGIN_TREE: &str =
    "setup requires a complete plugin tree at <plugin-root>/bin/agent-bar; \
     use install.sh for first bootstrap from a release archive";
```

Take care with the line continuation: `tests/cli.rs:404` pins the substring `complete plugin tree`, so the concatenated result must still contain it with single spaces. Verify by running that test, not by reading.

- [ ] **Step 5: Run the guard and the CLI suites** — `cargo test --test cli_vocabulary` (2 passed), `cargo test --test cli`, `cargo test --lib cli`.

- [ ] **Step 6: Full Rust gates.** Expect **303 across 19 suites** — Task 1's 302 plus this task's second `cli_vocabulary` test. Report the real number.

- [ ] **Step 7: Commit**

```bash
git add src/cli/grammar.rs src/cli/mod.rs tests/cli_vocabulary.rs
git commit -m "refactor: define each cli message once"
```

---

### Task 3: Name the fix where the fix is certain

**Files:**
- Modify: `src/cli/mod.rs` (`:117-120`, `:123-126`, `:139-142`, `:902-905`)

**Interfaces:** none; message text only.

- [ ] **Step 1: Write the failing test**

Copy design §7.3 covers messages that state a problem without its fix "where the fix is short and certain". That qualifier is the whole rule — a guessed fix is worse than none. Exactly four messages qualify. Add to `tests/cli_vocabulary.rs`:

```rust
/// §7.3: a message names the fix only where the fix is short and certain.
/// These four are the whole set — every other CLI error either states a
/// grammar mistake the user can see from their own command line, or a
/// condition whose remedy depends on facts the helper does not have.
#[test]
fn cli_messages_that_can_name_a_fix_do() {
    let source = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/mod.rs"),
    )
    .expect("read mod.rs");
    for needle in [
        "setup plugins-dir path does not exist: {}; create it or pass an existing parent",
        "setup plugins-dir path is not a directory: {}; pass the parent directory",
        "setup plugins-dir path is not writable: {}; pass a directory you own",
        "{} login executable was not found; install the provider CLI first",
    ] {
        assert!(
            source.contains(needle),
            "missing fix-naming message: {needle}"
        );
    }
}
```

- [ ] **Step 2: Run it and confirm the intended failure** — `cargo test --test cli_vocabulary`, expect this test to fail on the first needle.

- [ ] **Step 3: Implement**

Extend the four messages, keeping Unix convention: lower case, no trailing period, semicolon before the remedy.

| Site | Now | New |
| --- | --- | --- |
| `:117-120` | `setup plugins-dir path does not exist: {}` | `setup plugins-dir path does not exist: {}; create it or pass an existing parent` |
| `:123-126` | `setup plugins-dir path is not a directory: {}` | `setup plugins-dir path is not a directory: {}; pass the parent directory` |
| `:139-142` | `setup plugins-dir path is not writable: {}` | `setup plugins-dir path is not writable: {}; pass a directory you own` |
| `:902-905` | `{} login executable was not found` | `{} login executable was not found; install the provider CLI first` |

Leave every other CLI message alone. In particular do **not** add a remedy to the grammar errors: the user's own command line already shows the mistake, and `agent-bar help <command>` exists for the rest.

- [ ] **Step 4: Run the suites** — `cargo test --test cli_vocabulary` (3 passed), `cargo test --test cli`, `cargo test --lib cli`, `cargo test --test login`.

- [ ] **Step 5: Full Rust gates.** Expect **304 across 19 suites**. Report the real number.

- [ ] **Step 6: Commit**

```bash
git add src/cli/mod.rs tests/cli_vocabulary.rs
git commit -m "feat: name the fix in four cli errors"
```

---

### Task 4: The installer speaks to a person

**Files:**
- Modify: `install.sh` (header `:2-15`, unknown-flag `:37`, and every `log`/`ok`/`warn`/`die` call site)

**Interfaces:** none. The four output helpers at `:43-46` and their prefixes are unchanged; only the messages passed to them change.

`install.sh` is one of the two surfaces the copy design exempts from Unix convention and holds to the full GUI voice: name before category, active voice, no ceremony, imperative fixes, sentences take a period, never blame the user. Keep every message ASCII.

- [ ] **Step 1: Rewrite the header banner**

`install.sh:2-15` is printed verbatim by `--help` (measured fact 5). Replace lines 3–15 with:

```bash
# Installs the Agent Bar plugin for Omarchy Quattro. Nothing else.
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/othavi0/agent-bar/master/install.sh | bash
#
# Flags:
#   --force      Reinstall even if this version is already installed.
#   --yes, -y    Answer yes to every prompt.
#
# Env:
#   AGENT_BAR_VERSION  Version to install. Defaults to the latest release.
#
# Installs to: $HOME/.config/omarchy/plugins/agent-bar.usage
# No global executable is installed.
```

Keep line 2 (`#`) and line 16 (blank) exactly — the `sed -n '2,/^$/p'` range depends on both.

- [ ] **Step 2: Rewrite every message**

Apply this table exactly. The left column is the current argument passed to the helper; the right column replaces it. Prefixes (`==> `, `OK  `, `!   `, `ERR `) are added by the helpers and are not part of these strings.

| Line | Now | New |
| --- | --- | --- |
| 37 | `agent-bar install: unknown flag: $arg` | `Unknown flag: $arg` |
| 51 | `agent-bar requires Linux. Detected: $uname_s` | `Agent Bar needs Linux. This is $uname_s.` |
| 53 | `Only x86_64 plugin bundles are published. Detected: $arch` | `Agent Bar ships for x86_64 only. This is $arch.` |
| 57 | `curl not found` | `curl is missing. Install it and run this again.` |
| 58 | `sha256sum not found` | `sha256sum is missing. Install coreutils and run this again.` |
| 59 | `tar not found` | `tar is missing. Install it and run this again.` |
| 60 | `zstd not found (required for .tar.zst)` | `zstd is missing. Install it and run this again.` |
| 70 | `Resolving latest release...` | `Finding the latest release...` |
| 76 | `Could not resolve latest release. Set AGENT_BAR_VERSION.` | `Could not reach the release list. Set AGENT_BAR_VERSION and run this again.` |
| 95 | `Staged bundle missing bundle.json receipt` | `The download is incomplete: bundle.json is missing.` |
| 96 | `Staged bundle missing manifest.json` | `The download is incomplete: manifest.json is missing.` |
| 97 | `Staged bundle missing Service.qml` | `The download is incomplete: Service.qml is missing.` |
| 98 | `Staged bundle missing BarWidget.qml` | `The download is incomplete: BarWidget.qml is missing.` |
| 99 | `Staged helper bin/agent-bar missing or not executable` | `The download is incomplete: bin/agent-bar is missing or not executable.` |
| 101 | `Staged terminal helper missing or not executable` | `The download is incomplete: the terminal helper is missing or not executable.` |
| 105 | `bundle.json pluginId is not ${PLUGIN_ID}` | `This download is for another plugin, not ${PLUGIN_ID}.` |
| 110 | `bundle.json missing version` | `The download is incomplete: bundle.json has no version.` |
| 112 | `bundle.json version ${receipt_version} != expected ${expected_version}` | `The download is version ${receipt_version}, not ${expected_version}.` |
| 119 | `manifest version ${man_version} != expected ${expected_version}` | `The manifest is version ${man_version}, not ${expected_version}.` |
| 127 | `Receipt path missing on disk: ${path}` | `The download is missing a file it lists: ${path}` |
| 132 | `Staged inventory does not match bundle.json` | `The download does not match its own receipt.` |
| 137 | `Staged helper did not print a version` | `The helper in this download does not run here.` |
| 139 | `Helper version ${helper_version} != expected ${expected_version}` | `The helper is version ${helper_version}, not ${expected_version}.` |
| 141 | `Staged bundle inventory/receipt validated (${expected_version})` | `Download verified (${expected_version})` |
| 153 | `Downloading ${asset}...` | `Downloading ${asset}...` (unchanged) |
| 157 | `Verifying checksum...` | `Verifying the download...` |
| 159 | `Checksum mismatch — download may be corrupted.` | `The download is corrupted. Run this again.` |
| 160 | `Checksum OK` | `Checksum matches` |
| 162 | `Extracting plugin bundle...` | `Extracting...` |
| 167 | `Archive missing top-level ${PLUGIN_ID}/` | `The archive has no ${PLUGIN_ID} directory.` |
| 171 | `Archive contains links or traversal components` | `The archive contains unsafe paths. Nothing was installed.` |
| 189 | `Failed to install plugin root` | `Could not write to ${PLUGINS_DIR}.` |
| 196 | `Plugin installed at ${PLUGIN_ROOT}` | `Installed at ${PLUGIN_ROOT}` |
| 204 | `omarchy CLI not found. Enable manually: omarchy plugin enable ${PLUGIN_ID}` | `omarchy was not found. Enable it yourself: omarchy plugin enable ${PLUGIN_ID}` |
| 210 | `Existing shell entry — running omarchy plugin rescan` | `Already in your shell. Rescanning...` |
| 211 | `rescan failed; run: omarchy plugin rescan` | `Rescan failed. Run it yourself: omarchy plugin rescan` |
| 218 | `Enable ${PLUGIN_ID} via omarchy plugin enable? [Y/n] ` | `Enable ${PLUGIN_ID} now? [Y/n] ` |
| 224 | `Non-interactive install. Run: omarchy plugin enable ${PLUGIN_ID}` | `Nothing to answer here. Run: omarchy plugin enable ${PLUGIN_ID}` |
| 228 | `Running omarchy plugin enable ${PLUGIN_ID}` | `Enabling ${PLUGIN_ID}...` |
| 230 | `enable failed; run: omarchy plugin enable ${PLUGIN_ID}` | `Enable failed. Run it yourself: omarchy plugin enable ${PLUGIN_ID}` |
| 237 | `agent-bar plugin installer` | `Agent Bar installer` |
| 248 | `agent-bar.usage is already at ${version}` | `Already at ${version}. Use --force to reinstall.` |
| 253 | `Updating agent-bar.usage (${existing} -> ${version})...` | `Updating ${existing} to ${version}...` |
| 255 | `Installing agent-bar.usage ${version}...` | `Installing ${version}...` |
| 260 | `agent-bar.usage ${version} ready` | `Agent Bar ${version} is ready` |
| 261 | `Private helper: ${PLUGIN_ROOT}/bin/agent-bar` | `Helper: ${PLUGIN_ROOT}/bin/agent-bar` |
| 262 | `No global executable was installed.` | `No global executable was installed.` (unchanged) |

Two things the table encodes deliberately, so do not "fix" them: `bundle`/`bundle.json` survive wherever they name the real artifact or receipt (measured fact 6), and `ok` lines are labels rather than sentences, so they take no trailing period (voice rule 9).

- [ ] **Step 3: ShellCheck and a real run**

```bash
shellcheck install.sh
bash -n install.sh
bash install.sh --help
```

`--help` must print the new banner with no leading `#`, and `shellcheck` must be clean. Paste the `--help` output into the report — it is the only way this surface gets looked at.

- [ ] **Step 4: Confirm nothing else broke**

```bash
cargo test --test active_legacy_scan
cargo test --test active_docs
cargo test --test active_language
```

- [ ] **Step 5: Commit**

```bash
git add install.sh
git commit -m "feat: rewrite the installer in plain words"
```

---

### Task 5: The interactive update prompt

**Files:**
- Modify: `src/cli/mod.rs` (`INTERACTIVE_UPDATE_REQUIRES_TTY` :156-157, `confirm_interactive_update` :182-210)

**Interfaces:** none. `confirm_interactive_update`'s signature and return values are unchanged.

This is the second surface the copy design holds to the GUI voice. **The typed phrase `update agent-bar` is frozen** (measured fact 7) — it is what the user must type, pinned by two tests, and it is a safety mechanism.

- [ ] **Step 1: Write the failing test**

The inline tests in `src/cli/mod.rs` already cover this flow. Extend them rather than adding a file — find `confirm_interactive_update`'s existing tests near `:1020-1060` and add:

```rust
    #[test]
    fn interactive_update_prompt_speaks_plainly() {
        let mut stdin = std::io::Cursor::new(b"update agent-bar\n".to_vec());
        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        confirm_interactive_update(
            true,
            InteractiveUpdateOffer::Available {
                current: "10.0.0".into(),
                target: "10.2.0".into(),
            },
            &mut stdin,
            &mut stdout,
            &mut stderr,
        )
        .expect("confirmed");
        let out = String::from_utf8(stdout).expect("utf8");
        let err = String::from_utf8(stderr).expect("utf8");
        assert_eq!(out, "Updates 10.0.0 to 10.2.0. Settings stay.\n");
        // The typed phrase is a safety mechanism, not copy: it must survive
        // every rewording of the sentence around it.
        assert!(err.contains("update agent-bar"));
        assert_eq!(err, "Type update agent-bar to continue: ");
    }
```

- [ ] **Step 2: Run it and confirm the intended failure** — `cargo test --lib cli`, expect this test to fail on the stdout comparison.

- [ ] **Step 3: Implement**

Replace the two `writeln!` calls to stdout (`:189-192`) with one line that matches the Maintenance screen's wording for the same decision, so the two surfaces agree:

```rust
            writeln!(stdout, "Updates {current} to {target}. Settings stay.")
                .map_err(|err| CliFailure::internal(err.to_string()))?;
```

and give the prompt a trailing space so the caret does not touch the colon (`:193`):

```rust
            write!(stderr, "Type update agent-bar to continue: ")
```

The GUI says `Updates 10.0.0 → 10.2.0. Settings stay. Rolls back if it fails.`; the CLI drops the arrow for ASCII safety and drops the rollback clause, which the CLI's own `update apply` path does not promise.

Leave `INTERACTIVE_UPDATE_REQUIRES_TTY` unchanged: it already names its fix, `tests/cli.rs:678-680` and `mod.rs:1009-1010` pin its substrings, and it is read by a script author, not an installer.

- [ ] **Step 4: Run the suites** — `cargo test --lib cli`, `cargo test --test cli`.

- [ ] **Step 5: Full Rust gates.**

- [ ] **Step 6: Commit**

```bash
git add src/cli/mod.rs
git commit -m "feat: plain words at the update prompt"
```

---

### Task 6: The carried dead line, the sweep, the checkpoint, the record

**Files:**
- Modify: `assets/omarchy/CoreMaintenance.js` (`:219`)
- Modify: `tests/qml/tst_Maintenance.qml` (add the guard)
- Modify: `docs/superpowers/plans/2026-08-01-v11-06-cli-and-installer-copy.md` (this file)

- [ ] **Step 1: Delete the dead arming message**

Plan 05 carried this forward and two reviewers confirmed it (measured fact 10). In `assets/omarchy/CoreMaintenance.js:219`, remove the `next.message = "Click Uninstall again to confirm."` assignment from `maintenanceUiArmOrConfirmUninstall`, leaving the rest of the function — the `uninstallArmed` flip and both return shapes — exactly as it is.

Pin it in `tests/qml/tst_Maintenance.qml`, beside the existing uninstall tests:

```qml
  // The dialog binds its own message from uninstallArmed/purgeSettings, and
  // the only reader of ui.message sits behind the dialog's scrim, so this
  // string was set and never seen. The second click is communicated by the
  // confirm button flipping to "Uninstall now".
  function test_arming_sets_no_unseen_message() {
    var src = read("assets/omarchy/CoreMaintenance.js")
    verify(src.indexOf("Click Uninstall again") < 0)
    var view = read("assets/omarchy/MaintenanceView.qml")
    verify(view.indexOf('"Uninstall now"') >= 0)
  }
```

- [ ] **Step 2: Hygiene greps (each must return nothing)**

```bash
rg -n "clause '" src/cli
rg -n 'Staged bundle|Staged helper|Staged inventory' install.sh
rg -n 'Click Uninstall again' assets
rg -n 'agent-bar plugin installer|Checksum OK' install.sh
```

- [ ] **Step 3: Full checkpoint**

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
git diff --check
shellcheck install.sh
find assets/omarchy -type f -name '*.qml' -exec \
  /usr/lib/qt6/bin/qmllint -I /usr/share/omarchy/shell {} +
omarchy plugin validate assets/omarchy
QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
  /usr/lib/qt6/bin/qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
scripts/verify-v10-ui
```

Expect Rust **305 across 19 suites**: 301 baseline, plus 3 in the new `cli_vocabulary` suite (one per Task 1–3), plus Task 5's inline prompt test. QML **232 / 0 failed** — 231 plus this task's guard. Verify rather than trust this arithmetic and report the real numbers; a mismatch means a test was lost, which is worth finding.

- [ ] **Step 4: Execution record** — append to this file in the shape plans 02–05 use: commits mapped to tasks, final counts, "What the plan got wrong", "Deferred minors carried forward". Then:

```bash
git add assets/omarchy/CoreMaintenance.js tests/qml/tst_Maintenance.qml \
  docs/superpowers/plans/2026-08-01-v11-06-cli-and-installer-copy.md
git commit -m "docs: record execution outcome in plan"
```

---

## Done when

- `cargo test --test cli_vocabulary` passes with all three tests, and each was seen failing first.
- No string literal under `src/cli/` contains `clause`; `StatusClause` and its bindings are untouched.
- Each of the five watched messages is defined exactly once.
- `shellcheck install.sh` is clean, `bash install.sh --help` prints the new banner, and no `Staged` wording survives.
- The interactive update prompt reads `Updates 10.0.0 to 10.2.0. Settings stay.` and still requires the exact phrase `update agent-bar`.
- `Click Uninstall again` appears nowhere under `assets/`.
- Every checkpoint gate green; the real test counts reported rather than assumed.
- Nothing installed into `~/.config/omarchy/plugins/` — live QA remains the owner's gate.

## Not in this plan

This is the last plan in the v11 copy and visual track. Deliberately untouched:

- `src/bin/agent-bar-bundle.rs`, an internal build tool that is never installed.
- The CLI grammar itself: every command, keyword, flag, and provider id parses exactly as before.
- `INTERACTIVE_UPDATE_REQUIRES_TTY`, which already names its fix and is read by script authors.
- The GUI vocabulary guard's word list, which stays GUI-only because `bundle`, `collect`, and `schema` are legitimate CLI and installer vocabulary.
- `ProviderHeader.showStale`, dead since plan 03 and still carried in `headerModel`.

## Execution record

Executed 2026-07-31–08-01 on branch `feat/v11-foundation`, task order
1 → 2 → 3 → 4 → 5 → 6. Nine implementation/fix commits across Tasks 1–5 plus
this record: `a01da0b` + `b25e203` + `8e8daa5` (Task 1: `clause` to
`argument`, then the live help-text leak measured fact 1 missed, then the
raw-string-aware scanner — two fix rounds), `ae0ae0e` (Task 2: one definition
per message, clean on the first commit), `9c04d6a` + `ec53882` (Task 3: name
the fix in four CLI errors, then retract the two that were not certain — one
fix round), `d8d0534` (Task 4: the installer rewritten in plain words, clean
on the first commit), `deefd70` (Task 5: the interactive update prompt).
Tasks 1, 3, and 5 needed fix rounds; Task 1 needed two. This commit closes
Task 6: it deletes the dead arming-message assignment at
`CoreMaintenance.js:219`, adds `tst_Maintenance.qml`'s
`test_arming_sets_no_unseen_message` guard (confirmed failing first — 231
passed, 1 failed — before the deletion), runs the four Step 2 hygiene greps,
the full checkpoint, and writes this record.

Final state: `cargo test` **306 passed across 19 suites** — one more than
Step 3's own "305 across 19 suites" arithmetic (301 baseline + 3 in
`cli_vocabulary`, one per Task 1–3, + Task 5's inline prompt test). The extra
test is not lost work, it is uncounted work: Task 3's fix round (`ec53882`)
split its own fix-naming test into two — narrowed
`cli_messages_that_can_name_a_fix_do` to the three needles that still name a
remedy, and added a companion, `cli_writability_message_does_not_claim_a_fix`,
pinning the fourth message's now-bare form — so `cli_vocabulary` carries four
tests, not the three the plan's arithmetic assumed when it was written against
Task 3's first commit. Verified suite-by-suite (`cargo test` piped to a file
sidesteps this environment's condensed one-line summary and prints the real
per-suite listing): lib 243, `src/main.rs` 0, `agent-bar-bundle` 0,
`active_docs` 5, `active_language` 3, `active_legacy_scan` 4,
`agent_bar_bundle_cli` 4, `cli` 19, `cli_vocabulary` 4, `countdown_parity` 2,
`gui_vocabulary` 1, `icon_assets` 1, `login` 6, `schema_contract` 5,
`screenshot_inventory` 1, `servicecore_contract` 1, `severity_parity` 2,
`terminal_helper` 5, doc-tests 0 — 19 suites, 306 passed, 0 failed.
`qmltestrunner` **232 passed / 0 failed** (231 baseline + this task's
`test_arming_sets_no_unseen_message`). `cargo fmt --check`, `cargo clippy
--all-targets -- -D warnings`, and `git diff --check` all clean. `shellcheck
install.sh` clean. `qmllint -I /usr/share/omarchy/shell` over every
`assets/omarchy/**/*.qml` file: 0 errors, 365 warnings — the same
pre-existing `qs.*`/unqualified-access/missing-import noise the earlier
plans recorded across the whole plugin tree, none of it in the files this
task touches (`CoreMaintenance.js` is JS, outside qmllint's `*.qml` scope;
`tst_Maintenance.qml` is under `tests/qml`, outside `find assets/omarchy`).
`omarchy plugin validate assets/omarchy`: exit 0, no output. `scripts/
verify-v10-ui`: `verify-v10-ui: 17 PNGs ok → …/SHA256SUMS`, exit 0 —
unchanged at 17, since this task adds and removes no capture.

Every Step 2 hygiene grep returned nothing: `clause '` under `src/cli`,
`Staged bundle|Staged helper|Staged inventory` in `install.sh`, `Click
Uninstall again` under `assets`, and `agent-bar plugin installer|Checksum OK`
in `install.sh`.

### What the plan got wrong

Five defects surfaced during execution, all confirmed live rather than
assumed:

1. Measured fact 1 counted `clause` only in `src/cli/grammar.rs`. It missed
   a live user-facing leak: `agent-bar help status` printed `Clauses (any
   order, each at most once):` (`src/cli/mod.rs:53`), and
   `docs/commands.md:24` said "Status clauses". Found by review, reproduced
   by running the real binary before the fix was dispatched.
2. The guard Task 1 specified could not have caught that leak even with the
   word in scope: `string_literals` split each physical line on quote
   characters, and the help text is one literal continued across nine lines
   with trailing backslashes — line 53 itself contains no quote character at
   all, so it was never scanned. The plural, `Clauses`, would not have
   matched a singular-only pattern either. The fix round taught the scanner
   to track an open string across line continuations and to match the
   plural, alongside fixing both live texts.
3. A reviewer stated that `src/cli/**` contains no raw strings; the
   controller propagated that claim verbatim into the fix instruction for
   Task 1's second round; the implementer wrote it into the new scanner's
   doc comment as fact. A second reviewer found three `br#"..."#` fixtures in
   `src/cli/mod.rs`'s own test module (`:1116`, `:1127`, `:1139`) that the
   claim was false against. The lesson recorded plainly for next time: a
   reviewer's negative claim ("none exist") deserves the same verification as
   an implementer's positive one — neither is free just because it is easy
   to state.
4. Two of the four remedies Task 3's brief specified for §7.3 failed the
   rule's own qualifier, "short **and certain**", once reproduced against the
   real filesystem. `pass a directory you own` fired on a directory the user
   fully owns whenever a stale `.agent-bar-write-probe` file was left behind:
   the probe's own `create_new` also fails with `AlreadyExists`, and its
   cleanup swallows its own error, so the message blamed ownership for a
   condition ownership does not explain. `create it or pass an existing
   parent` fired on `EACCES` on an ancestor directory, where creating
   anything is not the fix. Both were reproduced live before the fix round.
   The controller ruled per §7.3 as written: the writability message reverted
   to bare (`setup plugins-dir path is not writable: {}`), and the
   existence message was reworded to name both realistic causes
   (`setup plugins-dir path cannot be read: {}; create it, or check the
   permissions on its parents`).
5. The controller ran two agents editing `tests/cli_vocabulary.rs` at the
   same time — Task 1's second fix round (the raw-string-aware scanner) and
   Task 3's fix round — a direct violation of this project's own rule against
   parallel implementers sharing a tree. Nothing broke: the Task 3 implementer
   noticed the foreign uncommitted diff already in the worktree, isolated its
   own hunks with `git apply --cached`, committed only those, and left the
   other agent's insertions untouched for its own commit. The cost was risk
   that happened not to land, not actual breakage — the rule holds regardless.

### Deferred minors carried forward

None blocking; triaged here for whoever next touches these files.

1. `cli_messages_are_defined_once`'s duplication guard counts raw source
   substrings read non-recursively from `src/cli/`. A duplicate message
   reintroduced with a different line-continuation shape would have
   identical runtime text but different source text, so the guard's count
   would not move and it would miss the duplicate.
2. `opens_raw_string` finds each raw-string prefix (`r"`, `r#"`, `br"`,
   `br#"`) with a leftmost match. A line holding a false prefix substring
   before a genuine opener of the same prefix would report the false one and
   miss the real opener. No such line exists in `src/cli` today.
3. `install.sh`'s `Could not reach the release list` names a network cause,
   but the same `die` fires on zero published releases or GitHub API
   rate-limiting too — the same defect class as item 4 above, materially
   weaker because the remedy it suggests (set `AGENT_BAR_VERSION`) is correct
   whichever cause fired. Cause-agnostic fix on file: `reach` → `find`.
4. Task 4's own report claims two `install.sh` lines gained a backslash
   continuation; only one did. The report is inaccurate; the shipped code is
   not.
5. `shellcheck` was not installed on the machine that ran Task 4, and the
   implementer installed it via `mise`, writing to the global
   `~/.config/mise/config.toml` — outside this repository. Benign (the
   project contract requires ShellCheck for shell changes), but a host side
   effect the owner should know about.
