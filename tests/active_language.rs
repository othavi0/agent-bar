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

/// Empty, and it stays empty. See `translation_backlog_is_empty`.
const PENDING_TRANSLATION: &[&str] = &[];

const BINARY_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "svg", "ico", "lock"];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn tracked_files(root: &Path) -> Vec<String> {
    let output = Command::new("git")
        .arg("ls-files")
        .current_dir(root)
        .output()
        .expect("git ls-files must run inside the repository");
    assert!(
        output.status.success(),
        "git ls-files exited with {}, stderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let files: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect();
    assert!(
        !files.is_empty(),
        "git ls-files returned zero tracked files; the gate would silently \
         scan nothing"
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
            found.push(format!("{rel}:{}: [{bad}] {}", index + 1, line.trim()));
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

#[test]
fn translation_backlog_is_empty() {
    assert!(
        PENDING_TRANSLATION.is_empty(),
        "translation backlog must stay empty; still pending: {PENDING_TRANSLATION:?}"
    );
}
