# v11 Foundation — Host Tokens + Language Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Delete Agent Bar's accidental parallel design system by binding every hardcoded colour value to the Quattro tokens the host already exports, and add the language gate that no test currently provides.

**Architecture:** Two independent mechanical sweeps, each ending in a test that makes the drift unrepeatable. The colour sweep replaces 16 `Qt.darker` calls and 18 of 19 hardcoded `Qt.rgba` alphas with `Util.alpha` and `Style` state tokens; the one survivor becomes a single named property. The language sweep adds a non-ASCII-letter gate with a shrinking translation backlog. No layout, copy, or behaviour changes here — those are plans 02 through 05.

**Tech Stack:** Rust (std only for the gate test), QML/Qt6, Qt6 `qmltestrunner`.

## Global Constraints

- Rust/Cargo and QML only. No Node, npm, Bun, pnpm, Yarn, ts-node, or Deno.
- No production `unwrap()` or `expect()` (clippy `-D warnings` is active).
- QML never parses raw provider output; external strings render as plain text.
- No Agent Bar-authored `Behavior`, `Transition`, `Animation` or `Animator` — `A11Y-013` and `TEST-029` must keep passing untouched.
- No colour-only meaning (`A11Y-012`).
- Active product copy is English. Commit subjects are English Conventional Commits, at most 50 characters.
- Do not edit `/usr/share/omarchy`. Do not mutate live Omarchy/Hyprland paths.
- Preserve unrelated worktree changes.
- **Baseline to preserve:** `cargo test` 283 passing across 12 suites; `qmltestrunner` 177 passing, 0 failed.
- `qmltestrunner` counts `initTestCase()` and `cleanupTestCase()` as passing tests, so a new `TestCase` file adds two beyond its own functions, and a data-driven function adds one per data row. Do not chase an exact total: the bar at every step is **0 failed**, plus the named tests appearing as `PASS`.
- Rust gate, run at every task end:
  ```bash
  cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings && git diff --check
  ```
- QML gate, run at every task that touches QML. The `qmltestrunner` on `PATH` is Qt5 and **fails silently**; the Qt6 path and both env vars are mandatory:
  ```bash
  find assets/omarchy -type f -name '*.qml' -exec qmllint -I /usr/share/omarchy/shell {} +
  omarchy plugin validate assets/omarchy
  QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
    /usr/lib/qt6/bin/qmltestrunner -input tests/qml \
    -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
  ```
- `cargo test` accepts ONE filter per invocation. Two test names means two invocations.
- Read every file before editing it. If an `Edit` reports "string not found", re-read the file first.

## Source Specs

- `docs/superpowers/specs/2026-07-30-visual-update-design.md` section 4 (token binding). **Partial:** this plan takes the colour rows. The three sizing rows — icon width to `Style.bar.iconCanvas`, chip `opacity` to `WidgetButton.dimmed`, ad-hoc spacings to `Style.spacing.*` — belong to plan 02, because they only make sense once the chip sits in a host slot.
- `docs/superpowers/specs/2026-07-30-copy-and-language-design.md` section 3 (language gate) and decision 2. **Complete:** nothing from section 3 is deferred.

## File Structure

| File | Responsibility | Action |
| --- | --- | --- |
| `tests/active_language.rs` | The language gate: scans tracked files, owns the exclusion list, allowlist, and translation backlog | create |
| `tests/qml/tst_Tokens.qml` | Proves the token binding held: no `Qt.darker`, bounded raw alphas, and secondary text recedes in both themes | create |
| `assets/omarchy/components/UsageWindow.qml` | Owns the usage track; after this plan it is the single declaration site for the track tint | modify |
| `assets/omarchy/ProviderRail.qml` | Rail chrome; loses its own frame alphas in favour of `Style` state tokens | modify |
| `assets/omarchy/{ProviderView,SettingsView,MaintenanceView}.qml` | Panes; separators become `PanelSeparator`, secondary text becomes `Util.alpha` | modify |
| `assets/omarchy/components/{ProviderHeader,StateMessage,ConfirmDialog}.qml` | Components; same two replacements | modify |
| `CHANGELOG.md`, `docs/adr/000{1,2,3}-*.md` | Translated to English | modify |
| `CLAUDE.md` | The historical-language carve-out shrinks to `docs/superpowers/**` only | modify |

---

## Task 1: Language gate mechanism

**Files:**
- Create: `tests/active_language.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `PENDING_TRANSLATION: &[&str]` — the shrinking backlog that Tasks 2 and 3 empty. Also establishes the exclusion and allowlist constants that later plans must not widen.

- [ ] **Step 1: Write the failing test**

Create `tests/active_language.rs`:

```rust
//! Active-surface language gate.
//!
//! Rule: no tracked text file may contain an alphabetic non-ASCII character.
//! "Alphabetic" is load-bearing — it flags accented letters while ignoring the
//! Nerd Font glyphs (Private Use Area) and the punctuation this project uses
//! on purpose.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Past session artefacts. Build record, not documentation.
const EXCLUDED_PREFIXES: &[&str] = &["docs/superpowers/"];

/// Deliberate non-ASCII, with the reason it stays.
const ALLOWLIST: &[(&str, &str)] = &[(
    "src/support/redact.rs",
    "accented fixture for the ANSI and control-character stripper",
)];

/// Files awaiting translation. Task 3 empties this and locks it shut.
const PENDING_TRANSLATION: &[&str] = &[
    "CHANGELOG.md",
    "docs/adr/0001-omarchy-right-click-settings.md",
    "docs/adr/0002-config-cli-dual-write.md",
    "docs/adr/0003-cli-help-hide-internals.md",
];

const BINARY_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "svg", "ico", "lock"];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn tracked_files(root: &Path) -> Vec<String> {
    let output = Command::new("git")
        .arg("ls-files")
        .current_dir(root)
        .output()
        .expect("git ls-files must be spawnable");
    // `.expect` above only covers a failure to spawn. Outside a checkout git
    // spawns fine and exits non-zero, which would leave an empty list and let
    // the gate pass having read nothing — the same defect as a gate that never
    // fails on purpose, arriving through a different door.
    assert!(
        output.status.success(),
        "git ls-files failed ({}): {}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect();
    assert!(
        !files.is_empty(),
        "git ls-files returned nothing; the gate would scan an empty set"
    );
    files
}

fn is_scannable(rel: &str) -> bool {
    if EXCLUDED_PREFIXES.iter().any(|p| rel.starts_with(p)) {
        return false;
    }
    if ALLOWLIST.iter().any(|(path, _)| *path == rel) {
        return false;
    }
    if PENDING_TRANSLATION.contains(&rel) {
        return false;
    }
    match Path::new(rel).extension().and_then(|e| e.to_str()) {
        Some(ext) => !BINARY_EXTENSIONS.contains(&ext),
        None => true,
    }
}

/// file:line:offending characters:trimmed line
fn offenders(root: &Path, rel: &str) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(root.join(rel)) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let bad: String = line
            .chars()
            .filter(|c| c.is_alphabetic() && !c.is_ascii())
            .collect();
        if !bad.is_empty() {
            found.push(format!(
                "{rel}:{}: [{bad}] {}",
                index + 1,
                line.trim()
            ));
        }
    }
    found
}

#[test]
fn active_files_contain_no_non_english_letters() {
    let root = workspace_root();
    let mut all = Vec::new();
    for rel in tracked_files(&root) {
        if !is_scannable(&rel) {
            continue;
        }
        all.extend(offenders(&root, &rel));
    }
    assert!(
        all.is_empty(),
        "alphabetic non-ASCII characters in active files:\n{}",
        all.join("\n")
    );
}

#[test]
fn allowlisted_files_still_need_their_exemption() {
    let root = workspace_root();
    for (rel, reason) in ALLOWLIST {
        assert!(
            !offenders(&root, rel).is_empty(),
            "{rel} is allowlisted for '{reason}' but no longer contains \
             non-ASCII letters; remove the entry"
        );
    }
}
```

- [ ] **Step 2: Run the tests and confirm they pass**

Run: `cargo test --test active_language`
Expected: PASS, 2 tests. The gate proves the mechanism against everything that is already English; the backlog is quarantined, not yet solved.

- [ ] **Step 3: Confirm the gate actually bites**

Temporarily append a Portuguese line to `README.md`:

```bash
printf '\nEsta linha existe apenas para validar o gate de idioma.\n' >> README.md
cargo test --test active_language 2>&1 | tail -20
```

Expected: FAIL naming `README.md`, the line number, and the offending characters. Then revert:

```bash
git checkout -- README.md
cargo test --test active_language
```

Expected: PASS again. A gate that has never failed on purpose is not a gate.

- [ ] **Step 4: Run the full Rust gate**

Run: `cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings && git diff --check`
Expected: 285 passing (283 baseline + 2 new), clippy clean.

- [ ] **Step 5: Commit**

```bash
git add tests/active_language.rs
git commit -m "test: add active-surface language gate"
```

---

## Task 2: Translate the three ADRs

**Files:**
- Modify: `docs/adr/0001-omarchy-right-click-settings.md` (12 lines)
- Modify: `docs/adr/0002-config-cli-dual-write.md` (14 lines)
- Modify: `docs/adr/0003-cli-help-hide-internals.md` (13 lines)
- Modify: `tests/active_language.rs` — remove the three ADR entries from `PENDING_TRANSLATION`

**Interfaces:**
- Consumes: `PENDING_TRANSLATION` from Task 1.
- Produces: a backlog containing only `CHANGELOG.md`.

- [ ] **Step 1: Read all three ADRs in full**

```bash
cat docs/adr/0001-omarchy-right-click-settings.md
cat docs/adr/0002-config-cli-dual-write.md
cat docs/adr/0003-cli-help-hide-internals.md
```

These are decision records. Translate the meaning exactly. Do not re-decide anything, do not add rationale that is not there, and do not modernise a decision that has since changed — an ADR records what was decided at the time.

- [ ] **Step 2: Remove the ADRs from the backlog first, so the test fails**

Edit `tests/active_language.rs`, leaving only `CHANGELOG.md`:

```rust
const PENDING_TRANSLATION: &[&str] = &["CHANGELOG.md"];
```

- [ ] **Step 3: Run the test and verify it fails**

Run: `cargo test --test active_language`
Expected: FAIL, listing the three ADR files with their offending lines.

- [ ] **Step 4: Translate each ADR to English**

Preserve heading structure, links, code spans, and file paths verbatim. Only prose changes.

- [ ] **Step 5: Run the tests and verify they pass**

Run: `cargo test --test active_language`
Expected: PASS, 2 tests.

- [ ] **Step 6: Verify links still resolve**

Run: `cargo test --test active_docs`
Expected: PASS, 4 tests. `active_docs_internal_links_resolve` is the one that matters — a translated heading can break an anchor link.

- [ ] **Step 7: Commit**

```bash
git add docs/adr tests/active_language.rs
git commit -m "docs: translate ADRs 0001-0003 to English"
```

---

## Task 3: Translate the changelog and close the gate

**Files:**
- Modify: `CHANGELOG.md` (804 lines, 206 with accented characters)
- Modify: `tests/active_language.rs` — empty `PENDING_TRANSLATION` and assert it stays empty
- Modify: `CLAUDE.md` — shrink the carve-out

**Interfaces:**
- Consumes: `PENDING_TRANSLATION` from Task 2.
- Produces: an empty backlog and a test asserting it. After this task no active file may contain a non-English letter.

- [ ] **Step 1: Empty the backlog and add the lock, so the test fails**

Edit `tests/active_language.rs`:

```rust
/// Empty, and it stays empty. See `translation_backlog_is_empty`.
const PENDING_TRANSLATION: &[&str] = &[];
```

Add at the end of the file:

```rust
#[test]
fn translation_backlog_is_empty() {
    assert!(
        PENDING_TRANSLATION.is_empty(),
        "translation backlog must stay empty; still pending: {PENDING_TRANSLATION:?}"
    );
}
```

- [ ] **Step 2: Run the test and verify it fails**

Run: `cargo test --test active_language`
Expected: FAIL on `active_files_contain_no_non_english_letters`, listing the `CHANGELOG.md` lines. `translation_backlog_is_empty` passes already; it is the tripwire for the future, not for now.

- [ ] **Step 3: Translate `CHANGELOG.md`**

**Read every line of the file. Do not let the gate decide when you are finished.**
The gate detects alphabetic non-ASCII characters, so it sees accented
Portuguese and is blind to unaccented Portuguese. Measured on this file:
206 lines carry an accent and 16 do not, such as

```
- Clique esquerdo com settings aberto volta ao usage sem fechar o popup.
```

which the gate will never report. Translating until the test turns green
would leave those sixteen in place and call the job done. Green is necessary,
not sufficient.

Work release by release, oldest first, so a partial run is still coherent. Rules:

- Version headings, dates, links, code spans, file paths, command names and commit-type prefixes stay byte-identical.
- Translate only the prose describing what changed.
- Do not merge, reorder, reword-for-brevity, or delete entries. This is a published record; the meaning must survive, and nothing else may.
- After each release section, re-run the scan to watch the accented count fall,
  while still reading every line of the section you just finished:
  ```bash
  cargo test --test active_language 2>&1 | rg -c 'CHANGELOG.md:'
  ```

- [ ] **Step 4: Run the tests and verify they pass**

Run: `cargo test --test active_language`
Expected: PASS, 3 tests.

Then confirm what the gate cannot: search for unaccented Portuguese that
survived the pass.

```bash
rg -in '\b(nao|voce|para|que|uma|dos|das|pelo|pela|arquivo|erro|atualizar|instalar|usuario|falhou|verifique|remover|adicionar|corrigir|quando|sobre|agora|ainda)\b' CHANGELOG.md
```

Expected: only false positives — `com` inside a hostname, `para` inside an
English word broken across a line, and similar. Every genuine Portuguese hit
is a line you missed; translate it and run again.

- [ ] **Step 5: Update the contract in `CLAUDE.md`**

The Documentation section currently reads:

```
Active docs and public copy are English. Historical changelog entries, ADR
bodies 0001–0003, and `docs/superpowers/**` remain historical and are excluded
from active legacy/language scans.
```

Replace with:

```
Active docs and public copy are English, enforced by
`tests/active_language.rs`. Only `docs/superpowers/**` is excluded: it holds
past session plans and specs, which are a build record rather than
documentation. New files written there are English.
```

Note the en dash in `0001–0003` disappears with the old sentence. That is fine; en dash is punctuation, not a letter, and the gate never flagged it.

- [ ] **Step 6: Run the full Rust gate**

Run: `cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings && git diff --check`
Expected: 286 passing, clippy clean.

- [ ] **Step 7: Commit**

```bash
git add CHANGELOG.md CLAUDE.md tests/active_language.rs
git commit -m "docs: translate changelog and enforce English"
```

---

## Task 4: Replace `Qt.darker` with `Util.alpha`

**Files:**
- Create: `tests/qml/tst_Tokens.qml`
- Modify: `assets/omarchy/ProviderView.qml:222,232`
- Modify: `assets/omarchy/SettingsView.qml:83,93,108,162`
- Modify: `assets/omarchy/MaintenanceView.qml:37,60,70`
- Modify: `assets/omarchy/components/UsageWindow.qml:41,65,98,126`
- Modify: `assets/omarchy/components/ProviderHeader.qml:57`
- Modify: `assets/omarchy/components/StateMessage.qml:71`
- Modify: `assets/omarchy/components/ConfirmDialog.qml:78`

**Interfaces:**
- Consumes: `Util.alpha(color, opacity)` from `qs.Commons` — a singleton already imported by every file in the list.
- Produces: exactly two secondary-text levels used by plans 02 through 05: `Util.alpha(root.foreground, 0.72)` for supporting text and labels, `Util.alpha(root.foreground, 0.55)` for meta and caption text. No third value.

The five existing factors map by role, not by arithmetic:

| Site | Current factor | Role | New |
| --- | --- | --- | --- |
| `UsageWindow.qml:41` | 1.35 | window kicker label | `0.72` |
| `UsageWindow.qml:65` | 1.35 | unit next to the numeral | `0.72` |
| `UsageWindow.qml:98` | 1.35 | "resets" prefix | `0.72` |
| `UsageWindow.qml:126` | 1.2 | compact row label | `0.72` |
| `ProviderHeader.qml:57` | 1.2 | plan badge text | `0.72` |
| `StateMessage.qml:71` | 1.25 | state body | `0.72` |
| `ConfirmDialog.qml:78` | 1.15 | dialog body | `0.72` |
| `SettingsView.qml:83,93` | 1.3 | field labels | `0.72` |
| `SettingsView.qml:108,162` | 1.35 | section captions | `0.55` |
| `MaintenanceView.qml:37,60,70` | 1.35, 1.25, 1.2 | meta rows | `0.55` |
| `ProviderView.qml:222` | 1.4 | footer meta left | `0.55` |
| `ProviderView.qml:232` | ternary `1.0 : 1.15` | footer connection | `showStale ? 0.72 : 0.55` |

`ProviderView.qml:222` and `:232` sit in the meta footer that plan 03 deletes. Convert them anyway so the gate stays green in between; plan 03 removes the whole `Row`.

- [ ] **Step 1: Write the failing test**

Create `tests/qml/tst_Tokens.qml`:

```qml
import QtQuick
import QtTest
import qs.Commons

TestCase {
  id: testCase
  name: "AgentBarTokens"
  when: windowShown

  property string repoRoot: {
    var path = String(Qt.resolvedUrl(".")).replace("file://", "")
    if (path.endsWith("/"))
      path = path.slice(0, -1)
    var parts = path.split("/")
    parts.pop(); parts.pop()
    return parts.join("/")
  }

  function read(rel) {
    var xhr = new XMLHttpRequest()
    xhr.open("GET", "file://" + repoRoot + "/" + rel, false)
    xhr.send()
    return String(xhr.responseText || "")
  }

  function tokenScannedFiles() {
    return [
      "assets/omarchy/BarWidget.qml",
      "assets/omarchy/Popup.qml",
      "assets/omarchy/ProviderRail.qml",
      "assets/omarchy/ProviderView.qml",
      "assets/omarchy/SettingsView.qml",
      "assets/omarchy/MaintenanceView.qml",
      "assets/omarchy/components/ProviderChip.qml",
      "assets/omarchy/components/ProviderHeader.qml",
      "assets/omarchy/components/UsageWindow.qml",
      "assets/omarchy/components/StateMessage.qml",
      "assets/omarchy/components/SettingsProviderRow.qml",
      "assets/omarchy/components/ConfirmDialog.qml"
    ]
  }

  // Qt.darker divides HSV value. On a dark theme that recedes; on a light
  // theme it advances, so secondary text outranks primary. Util.alpha works
  // in both directions with one value.
  function test_no_qt_darker() {
    var files = tokenScannedFiles()
    for (var i = 0; i < files.length; i++) {
      var code = read(files[i]).replace(/\/\/[^\n]*/g, "")
      verify(code.indexOf("Qt.darker") < 0,
             files[i] + " still calls Qt.darker; use Util.alpha")
    }
  }

  // Composite a translucent foreground over a background, the way the
  // compositor does, then compare WCAG contrast.
  function composite(fg, bg, a) {
    return Qt.rgba(fg.r * a + bg.r * (1 - a),
                   fg.g * a + bg.g * (1 - a),
                   fg.b * a + bg.b * (1 - a), 1)
  }

  function luminance(c) {
    function ch(v) { return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4) }
    return 0.2126 * ch(c.r) + 0.7152 * ch(c.g) + 0.0722 * ch(c.b)
  }

  function contrast(a, b) {
    var la = luminance(a), lb = luminance(b)
    var hi = Math.max(la, lb), lo = Math.min(la, lb)
    return (hi + 0.05) / (lo + 0.05)
  }

  function test_secondary_recedes_in_both_themes_data() {
    return [
      { tag: "dark",  fg: Qt.color("#fff6ff"), bg: Qt.color("#05080a") },
      { tag: "light", fg: Qt.color("#18181b"), bg: Qt.color("#f4f4f5") },
      { tag: "white", fg: Qt.color("#000000"), bg: Qt.color("#ffffff") }
    ]
  }

  function test_secondary_recedes_in_both_themes(data) {
    var primary = contrast(data.fg, data.bg)
    var supporting = contrast(composite(data.fg, data.bg, 0.72), data.bg)
    var meta = contrast(composite(data.fg, data.bg, 0.55), data.bg)
    verify(supporting < primary,
           data.tag + ": supporting " + supporting + " must be under primary " + primary)
    verify(meta < supporting,
           data.tag + ": meta " + meta + " must be under supporting " + supporting)
  }
}
```

- [ ] **Step 2: Run the QML suite and verify the source scan fails**

Run:
```bash
QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
  /usr/lib/qt6/bin/qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
```
Expected: `test_no_qt_darker` FAILS naming the first offending file. The three `test_secondary_recedes_in_both_themes` rows PASS already — they describe the target, and they are what proves `Util.alpha` is the right replacement rather than a different magic number.

- [ ] **Step 3: Replace every call site**

Work through the table above. Each edit is the same shape. In `UsageWindow.qml:41`:

```qml
      color: Util.alpha(root.foreground, 0.72)
```

Every listed file already imports `qs.Commons`; confirm with `rg -n 'import qs.Commons' <file>` before editing, and add the import if a file lacks it.

`ProviderView.qml:232` keeps its conditional, expressed on the new scale:

```qml
        color: Util.alpha(root.foreground, root.header.showStale ? 0.72 : 0.55)
```

- [ ] **Step 4: Run the QML gate and verify it passes**

Run:
```bash
find assets/omarchy -type f -name '*.qml' -exec qmllint -I /usr/share/omarchy/shell {} +
omarchy plugin validate assets/omarchy
QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
  /usr/lib/qt6/bin/qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
```
Expected: 0 failed. `AgentBarTokens::test_no_qt_darker` and all three
`test_secondary_recedes_in_both_themes` rows report `PASS`. `qmllint` clean.

- [ ] **Step 5: Commit**

```bash
git add tests/qml/tst_Tokens.qml assets/omarchy
git commit -m "fix: bind secondary text to theme alpha"
```

---

## Task 5: Bind control chrome to `Style` state tokens

**Files:**
- Modify: `assets/omarchy/ProviderRail.qml:88,90,146,150,208`
- Modify: `assets/omarchy/components/StateMessage.qml:31,37,43`
- Modify: `assets/omarchy/components/ConfirmDialog.qml:31,47,48`
- Modify: `assets/omarchy/components/ProviderHeader.qml:52`
- Modify: `tests/qml/tst_Tokens.qml`

**Interfaces:**
- Consumes: `Style.selectedFill`, `Style.selectedBorderColor`, `Style.hoverFill`, `Style.normalFill`, `Style.normalBorderColor` from `qs.Commons` — all `readonly property color`, already resolved against the active palette.
- Produces: no new symbols. After this task the only raw alphas left are separators (Task 6) and the usage track (Task 7).

| Site | Current | Role | New |
| --- | --- | --- | --- |
| `ProviderRail.qml:88` | `Qt.rgba(fg, 0.05)` | rail frame fill | delete — the frame goes in plan 03; until then, `Style.normalFill` |
| `ProviderRail.qml:90` | `Qt.rgba(fg, 0.22)` | rail frame border | `Style.normalBorderColor` |
| `ProviderRail.qml:146` | `Qt.rgba(fg, 0.12)` | selected provider plate | `Style.selectedFill` |
| `ProviderRail.qml:150` | `Qt.rgba(fg, 0.3)` | selected plate border | `Style.selectedBorderColor` |
| `ProviderRail.qml:208` | `Qt.rgba(fg, 0.08)` | settings hover plate | `Style.hoverFill` |
| `StateMessage.qml:31` | `Qt.rgba(fg, 0.12)` | skeleton bar, strong | `Style.selectedFill` |
| `StateMessage.qml:37,43` | `Qt.rgba(fg, 0.08)` | skeleton bar, weak | `Style.hoverFill` |
| `ConfirmDialog.qml:31` | `Qt.rgba(fg, 0.45)` | scrim | keep raw; a scrim is not control chrome. Declare once as `readonly property color scrimColor` on the dialog root and reference it. |
| `ConfirmDialog.qml:47` | `Qt.rgba(fg, 0.55)` | card fill | `Style.selectedFill` |
| `ConfirmDialog.qml:48` | `Qt.rgba(fg, 0.28)` | card border | `Style.normalBorderColor` |
| `ProviderHeader.qml:52` | `Qt.rgba(fg, 0.25)` | plan badge border | `Style.normalBorderColor` |

The values differ slightly from the tokens — 0.12 against the theme's 0.18, 0.22 against 0.4. That is the point: the tokens track the installed theme, the literals never did.

- [ ] **Step 1: Add the failing test**

Append to `tests/qml/tst_Tokens.qml`, inside the `TestCase`:

```qml
  // Raw foreground alphas are allowed only where no host token exists.
  // Every other surface must read the theme's state vocabulary.
  function allowedRawAlphaFiles() {
    return [
      "assets/omarchy/components/UsageWindow.qml",   // usage track, Task 7
      "assets/omarchy/components/ConfirmDialog.qml", // modal scrim
      "assets/omarchy/ProviderView.qml",             // separators, Task 6
      "assets/omarchy/SettingsView.qml",             // separators, Task 6
      "assets/omarchy/MaintenanceView.qml"           // separators, Task 6
    ]
  }

  function test_control_chrome_uses_style_tokens() {
    var files = tokenScannedFiles()
    var allowed = allowedRawAlphaFiles()
    for (var i = 0; i < files.length; i++) {
      if (allowed.indexOf(files[i]) >= 0)
        continue
      var code = read(files[i]).replace(/\/\/[^\n]*/g, "")
      verify(code.indexOf("Qt.rgba(") < 0,
             files[i] + " still hardcodes an alpha; use a Style state token")
    }
  }
```

- [ ] **Step 2: Run the suite and verify it fails**

Run the QML gate command from Task 4 Step 4.
Expected: `test_control_chrome_uses_style_tokens` FAILS naming `ProviderRail.qml`.

- [ ] **Step 3: Replace the call sites**

Work the table. Two shapes appear. A direct token, `ProviderRail.qml:146`:

```qml
          color: railItem.selected ? Style.selectedFill : "transparent"
```

And the one value with no host token, `ConfirmDialog.qml`, declared once on the root and referenced at line 31:

```qml
  readonly property color scrimColor: Util.alpha(Color.foreground, 0.45)
```

Verify `import qs.Ui` is present in `ProviderRail.qml` and `StateMessage.qml` — `Style` lives in `qs.Commons`, but these files also use `qs.Ui` components; do not remove either import.

- [ ] **Step 4: Run the QML gate and verify it passes**

Run the QML gate command from Task 4 Step 4.
Expected: 0 failed, with `AgentBarTokens::test_control_chrome_uses_style_tokens`
reporting `PASS`.

- [ ] **Step 5: Verify the rail still shows selection**

The selected-provider plate is the one visible behaviour this task touches, and `UX-020B` requires a neutral plate with no accent tick. Confirm the existing coverage still passes:

Run:
```bash
QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
  /usr/lib/qt6/bin/qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt 2>&1 | rg -i 'rail|selected'
```
Expected: every matching line reports PASS.

- [ ] **Step 6: Commit**

```bash
git add tests/qml/tst_Tokens.qml assets/omarchy
git commit -m "fix: bind control chrome to Quattro tokens"
```

---

## Task 6: Replace separators with `PanelSeparator`

**Files:**
- Modify: `assets/omarchy/ProviderView.qml:72,164`
- Modify: `assets/omarchy/SettingsView.qml:150,248,302`
- Modify: `assets/omarchy/MaintenanceView.qml:129`
- Modify: `tests/qml/tst_Tokens.qml`

**Interfaces:**
- Consumes: `PanelSeparator` from `qs.Ui`. It is a `Rectangle` with `property color foreground` and `property real strength` defaulting to `0.12`, `height` fixed at 1, `width` bound to its parent.
- Produces: no new symbols.

Six hand-rolled 1px rules become the host component. Five already use `0.12`, which is `PanelSeparator`'s default. `ProviderView.qml:164` uses `0.08` — the quiet rule above the secondary window list. Pass `strength: 0.08` there rather than changing the default.

- [ ] **Step 1: Tighten the test so the separator files lose their exemption**

In `tests/qml/tst_Tokens.qml`, reduce `allowedRawAlphaFiles()` to:

```qml
  function allowedRawAlphaFiles() {
    return [
      "assets/omarchy/components/UsageWindow.qml",   // usage track, Task 7
      "assets/omarchy/components/ConfirmDialog.qml"  // modal scrim
    ]
  }
```

- [ ] **Step 2: Run the suite and verify it fails**

Run the QML gate command from Task 4 Step 4.
Expected: `test_control_chrome_uses_style_tokens` FAILS naming `ProviderView.qml`.

- [ ] **Step 3: Replace each separator**

`ProviderView.qml:69-73` currently reads:

```qml
    Rectangle {
      width: parent.width
      height: 1
      color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)
    }
```

becomes:

```qml
    PanelSeparator {
      width: parent.width
      foreground: root.foreground
    }
```

and `ProviderView.qml:161-165`, the quiet rule, becomes:

```qml
      PanelSeparator {
        width: parent.width
        foreground: root.foreground
        strength: 0.08
      }
```

`SettingsView.qml` and `MaintenanceView.qml` follow the first shape. Confirm each file imports `qs.Ui`; `ProviderView.qml` currently does not — add `import qs.Ui` there.

- [ ] **Step 4: Run the QML gate and verify it passes**

Run the QML gate command from Task 4 Step 4.
Expected: 0 failed. `qmllint` must be clean — a missing `qs.Ui` import surfaces here as an unresolved type, not as a test failure.

- [ ] **Step 5: Commit**

```bash
git add tests/qml/tst_Tokens.qml assets/omarchy
git commit -m "refactor: use PanelSeparator for panel rules"
```

---

## Task 7: Give the usage track a single declaration site

**Files:**
- Modify: `assets/omarchy/components/UsageWindow.qml:73-91`
- Modify: `tests/qml/tst_Tokens.qml`

**Interfaces:**
- Consumes: `Util.alpha` from Task 4.
- Produces: `UsageWindow.trackColor` — a `readonly property color`. Plan 03 reuses it for the compact rows so both track shapes tint from one place.

The track background has no host token: it is not control chrome, it is a data surface. That does not license repeating the literal. It is declared once, named, and referenced.

- [ ] **Step 1: Tighten the test to its final shape**

In `tests/qml/tst_Tokens.qml`, reduce the exemption to the scrim alone and add a declaration-count assertion:

```qml
  function allowedRawAlphaFiles() {
    return ["assets/omarchy/components/ConfirmDialog.qml"] // modal scrim
  }

  // The track tint has no host token, so it gets a name and exactly one
  // declaration. Two would be a parallel system starting over.
  function test_usage_track_declared_once() {
    var code = read("assets/omarchy/components/UsageWindow.qml")
        .replace(/\/\/[^\n]*/g, "")
    var declarations = code.split("readonly property color trackColor").length - 1
    compare(declarations, 1, "trackColor must be declared exactly once")
    verify(code.indexOf("Qt.rgba(") < 0,
           "UsageWindow must reference trackColor, not a literal alpha")
  }
```

- [ ] **Step 2: Run the suite and verify it fails**

Run the QML gate command from Task 4 Step 4.
Expected: `test_usage_track_declared_once` FAILS with `trackColor must be declared exactly once`, actual 0.

- [ ] **Step 3: Declare the property and reference it**

Add to the `UsageWindow` root, beside the existing `readonly property bool hasPercent`:

```qml
  // Data surface, not control chrome — no host token covers it. Declared
  // once here so plan 03's compact rows tint from the same place.
  readonly property color trackColor: Util.alpha(root.foreground, 0.12)
```

Then at line 77 replace the literal:

```qml
      color: root.trackColor
```

- [ ] **Step 4: Run the QML gate and verify it passes**

Run the QML gate command from Task 4 Step 4.
Expected: 0 failed, with `AgentBarTokens::test_usage_track_declared_once`
reporting `PASS`.

- [ ] **Step 5: Commit**

```bash
git add tests/qml/tst_Tokens.qml assets/omarchy/components/UsageWindow.qml
git commit -m "refactor: name the usage track tint"
```

---

## Task 8: Make the light-theme evidence real

**Files:**
- Modify: `tests/qml/TestPalette.js:27-43` (`themePalette`) and `requiredScreenshotNames()`
- Modify: `tests/qml/tst_Screenshots.qml:85-101` (`applyTheme`, `paintState`)

**Interfaces:**
- Consumes: `Util.alpha` from Task 4.
- Produces: a screenshot harness whose muted colour is derived the same way the shipped components derive it, plus a `white` theme case.

**Why this task exists.** The repository already renders a `ready-light.png`
and has since v10. It never caught the `Qt.darker` inversion, because the
harness does not use the components: `TestPalette.themePalette` hands back a
hand-picked `muted` (`#52525b` light, `#a1a1aa` dark) and `applyTheme` assigns
it straight to `stage.muted`. The light-theme gate was testing a mock of
itself. Binding the fixture to the real expression is what turns it into
evidence; the `white` palette is then the worst case, because
`Qt.darker(#000000, n)` returns `#000000` and the hierarchy collapses to
identical pixels.

- [ ] **Step 1: Add the white palette and require its screenshot**

In `tests/qml/TestPalette.js`, add `"ready-white.png"` to the array returned by
`requiredScreenshotNames()`, and add the palette to `themePalette` above the
existing `light` branch. Note it deliberately carries no `muted` key — Step 3
removes that key from every palette:

```js
  if (mode === "white") {
    return {
      mode: "white",
      background: "#ffffff",
      foreground: "#000000",
      accent: "#0057d8",
      urgent: "#c31432"
    }
  }
```

- [ ] **Step 2: Run the suite and verify it fails**

Run the QML gate command from Task 4 Step 4.
Expected: FAIL on the screenshot inventory test, reporting `ready-white.png`
missing.

- [ ] **Step 3: Derive `muted` the way the components do**

In `tests/qml/TestPalette.js`, delete the `muted` key from both the `light` and
`dark` return objects. A fixture that hardcodes the value it is meant to verify
cannot verify it.

Then in `tests/qml/tst_Screenshots.qml`, add `import qs.Commons` at the top if
it is absent, and replace `applyTheme` at line 85:

```qml
  function applyTheme(mode) {
    var p = Core.themePalette(mode)
    stage.color = p.background
    stage.fg = p.foreground
    // Same expression the shipped components use, so this fixture actually
    // exercises the token binding instead of a hand-picked stand-in.
    stage.muted = Util.alpha(p.foreground, 0.72)
    stage.badgeColor = p.urgent
  }
```

- [ ] **Step 4: Render the white case**

In `tests/qml/tst_Screenshots.qml`, `paintState` currently special-cases
`ready-light` at line 95 and then falls through to `applyTheme("dark")`. Add
the white branch immediately after the light one, before that fallthrough,
using the same three strings so the only variable between the three renders is
the palette:

```qml
    if (name.indexOf("ready-white") === 0) {
      applyTheme("white")
      stage.titleText = "Claude"
      stage.badgeText = "Connected"
      stage.bodyText = "Session (5h) 58% left · Max plan"
      return
    }
```

- [ ] **Step 5: Run the QML gate and verify it passes**

Run the QML gate command from Task 4 Step 4.
Expected: 0 failed, and `ready-white.png` present in the screenshot output
directory alongside `ready-light.png` and `ready-dark.png`.

- [ ] **Step 6: Look at the three renders**

The contrast test proves the ordering arithmetically; a human confirms it reads
right.

```bash
ls tests/qml/screenshots/ready-{dark,light,white}.png
```

Open all three. In every one, `Session (5h) 58% left · Max plan` must read
quieter than `Claude`. If the body competes with the title in any theme, the
role mapping in Task 4 is wrong — fix the mapping, not the token.

- [ ] **Step 7: Run both full gates**

```bash
cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings && git diff --check
find assets/omarchy -type f -name '*.qml' -exec qmllint -I /usr/share/omarchy/shell {} +
omarchy plugin validate assets/omarchy
QT_QPA_PLATFORM=offscreen QML_XHR_ALLOW_FILE_READ=1 QT_LOGGING_TO_CONSOLE=1 \
  /usr/lib/qt6/bin/qmltestrunner -input tests/qml \
  -import /usr/share/omarchy/shell -import assets/omarchy -o -,txt
```
Expected: cargo 286 passing, clippy clean; qmltestrunner 0 failed.

- [ ] **Step 8: Commit**

```bash
git add tests/qml
git commit -m "test: bind theme fixture to real token"
```

---

## Done when

- `cargo test` reports 286 passing; `qmltestrunner` reports 0 failed.
- `rg -c 'Qt.darker' assets/omarchy` returns nothing.
- `rg -c 'Qt.rgba' assets/omarchy` matches only `ConfirmDialog.qml`.
- `tests/qml/TestPalette.js` no longer defines a `muted` colour anywhere.
- No tracked file outside `docs/superpowers/**` contains an alphabetic non-ASCII character, except the one allowlisted fixture.
- `A11Y-013` and `TEST-029` still pass, untouched.

## Not in this plan

Layout, copy, assets, severity, notifications, and the CLI. Those are:

| Plan | Covers |
| --- | --- |
| 02 — bar chip | `WidgetButton` base, icon canvas and optical scale, fixed-width numeral, Codex mark, Grok tint, state cues, tooltip humaniser |
| 03 — popup | rail alignment, header tags, footer removal, stale banner, typed-state copy |
| 04 — severity | thresholds shared with Rust, lead-window election, `UX-017`/`UX-020A`/`UX-028`/`UX-054` amendments |
| 05 — notifications and CLI | Rust countdown humaniser, notification metric and copy, CLI and `install.sh` copy |
