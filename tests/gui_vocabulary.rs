//! Copy design §4 rule 2: GUI strings name what the user sees, not what the
//! code calls it. Twenty-one strings leaked internal vocabulary before the
//! rewrite; this test is what stops the twenty-second.
//!
//! Scope is the GUI only. The CLI is deliberately exempt (§7): its stderr is
//! read while debugging and Unix convention there is different.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Copy design §4 rule 2's list, complete. `provider` is deliberately absent
/// — decision 3 keeps it on screen. `bundle` is the one that was actually
/// leaking: six GUI strings carried it until the maintenance rewrite.
const BANNED: &[&str] = &[
    "adapter", "schema", "payload", "envelope", "bundle", "collect", "clause", "snapshot",
];

fn gui_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.join("assets/omarchy")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let is_source = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e == "qml" || e == "js");
            if is_source {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Double-quoted string literals, minus comment lines. Every quoted piece
/// is scanned, including single-token ids, enum values, and glyphs — there
/// is no length or shape filter. What keeps identifiers like
/// `schemaVersion` from tripping the guard is exact whole-token matching
/// against `BANNED`, not a filter on the literal here. Plural forms
/// (`bundles`) are a known gap: prefix matching would close it, but at the
/// cost of a false positive on `schemaVersion`, a legitimate JSON field
/// name in diagnostic strings in `CoreService.js` and `CoreSettings.js`.
fn user_facing_literals(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        for piece in line.split('"').skip(1).step_by(2) {
            out.push(piece.to_owned());
        }
    }
    out
}

#[test]
fn gui_copy_has_no_internal_vocabulary() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut violations: BTreeSet<String> = BTreeSet::new();
    let files = gui_files(&root);
    assert!(
        files.len() >= 15,
        "expected the GUI source tree, found {} files",
        files.len()
    );
    for path in files {
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .display()
            .to_string();
        for literal in user_facing_literals(&source) {
            let lowered = literal.to_lowercase();
            for word in BANNED {
                if lowered
                    .split(|c: char| !c.is_ascii_alphabetic())
                    .any(|t| t == *word)
                {
                    violations.insert(format!("{rel}: {word} in {literal:?}"));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "GUI copy leaks internal vocabulary ({}):\n  - {}",
        violations.len(),
        violations.into_iter().collect::<Vec<_>>().join("\n  - ")
    );
}
