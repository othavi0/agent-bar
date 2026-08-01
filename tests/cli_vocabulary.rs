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

/// True when `prefix` opens a raw string as a real token on `line` at byte
/// offset `pos`, not merely as the tail of an ordinary word ending in `r`
/// right before a closing quote (`"doctor"`, `"provider"` both contain the
/// literal bytes `r"` but do not open anything).
fn is_token_boundary(line: &str, pos: usize) -> bool {
    pos == 0 || {
        let prev = line.as_bytes()[pos - 1];
        !(prev.is_ascii_alphanumeric() || prev == b'_')
    }
}

/// True when `line` opens one of this codebase's raw string prefixes (`r"`,
/// `r#"`, `br"`, `br#"`) — used by the CLI test module for raw JSON
/// payloads. See [`is_token_boundary`] for why a plain substring search is
/// not enough.
fn opens_raw_string(line: &str) -> bool {
    ["br#\"", "br\"", "r#\"", "r\""]
        .iter()
        .any(|prefix| matches!(line.find(prefix), Some(pos) if is_token_boundary(line, pos)))
}

/// Double-quoted spans, tracked across lines so a literal continued with a
/// trailing `\` (the multi-line `HelpTopic` help text) is not lost. `//`
/// comments are skipped, but only outside a literal — a continuation line
/// never starts with `//` in this codebase.
///
/// `src/cli/mod.rs`'s test module has three raw byte-string literals
/// (`br#"..."#`, JSON confirmation payloads). Counting quote characters
/// would misread their embedded `"` as ordinary literal boundaries, so those
/// lines are detected by [`opens_raw_string`] and skipped rather than
/// parsed — never scanned for content, and never allowed to flip the
/// carried in/out-of-string state on a hunch. That skip is safe only when
/// the line is self-contained (an even number of quote characters); it is
/// verified per line rather than assumed, because an odd count would mean
/// the raw string continues past this line and silently skipping it would
/// desynchronize the parity count for every line after it. Outside of those
/// three lines, counting quote characters is sound because no other line in
/// this scope has an escaped quote.
fn string_literals(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_string = false;
    for line in source.lines() {
        if !in_string && line.trim_start().starts_with("//") {
            continue;
        }
        let quote_count = line.matches('"').count();
        if !in_string && opens_raw_string(line) && quote_count % 2 == 0 {
            continue;
        }
        for (idx, piece) in line.split('"').enumerate() {
            let is_inside = if in_string {
                idx % 2 == 0
            } else {
                idx % 2 == 1
            };
            if is_inside {
                out.push(piece.to_owned());
            }
        }
        if quote_count % 2 == 1 {
            in_string = !in_string;
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
                .any(|token| token == "clause" || token == "clauses")
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

/// §7.3: a message names the fix only where the fix is short and certain.
/// These three are the whole set of messages that name a remedy — every
/// other CLI error either states a grammar mistake the user can see from
/// their own command line, or a condition whose remedy depends on facts the
/// helper does not have.
///
/// The writability probe is that last case: `create_new(true)` fails both on
/// a real permission problem and on a stale `.agent-bar-write-probe` left by
/// an earlier run (its own cleanup swallows failure), so a directory the
/// user fully owns can still report this error. There is no short, certain
/// fix to name, so [`cli_writability_message_does_not_claim_a_fix`] pins
/// that message bare.
#[test]
fn cli_messages_that_can_name_a_fix_do() {
    let source =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/mod.rs"))
            .expect("read mod.rs");
    for needle in [
        "setup plugins-dir path cannot be read: {}; create it, or check the permissions on its parents",
        "setup plugins-dir path is not a directory: {}; pass the parent directory",
        "{} login executable was not found; install the provider CLI first",
    ] {
        assert!(
            source.contains(needle),
            "missing fix-naming message: {needle}"
        );
    }
}

/// Companion to [`cli_messages_that_can_name_a_fix_do`]: the writability
/// probe cannot tell a permission problem from a stale probe file, so its
/// message must not claim a fix. Checks for the literal closing quote and
/// trailing comma right after `{}` so a future well-meaning edit cannot
/// quietly bolt a remedy back onto this message.
#[test]
fn cli_writability_message_does_not_claim_a_fix() {
    let source =
        fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cli/mod.rs"))
            .expect("read mod.rs");
    assert!(
        source.contains("\"setup plugins-dir path is not writable: {}\","),
        "the writability probe cannot tell a permission problem from a stale probe file, \
         so its message must not claim a fix"
    );
}
