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
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_owned();
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
