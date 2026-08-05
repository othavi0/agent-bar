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
use std::path::{Path, PathBuf};

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

/// Every `.rs` file under `src/cli/`, found by walking the tree with an
/// explicit stack rather than reading one directory. `src/cli/mod.rs` is
/// past a thousand lines; the day it splits into submodules
/// (`src/cli/help/mod.rs` or similar), a non-recursive `fs::read_dir` would
/// silently stop covering the new file while every guard below kept
/// passing. Shape matches `tests/gui_vocabulary.rs::gui_files` on purpose,
/// so the two guards read as siblings.
fn cli_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.join("src/cli")];
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
            let is_source = path.extension().and_then(|e| e.to_str()) == Some("rs");
            if is_source {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Concatenation of every `.rs` file under `src/cli/`, for guards that check
/// whether a message exists anywhere in the tree rather than walking file by
/// file themselves. A `\n` separator sits between files; none of the needles
/// these guards check span a file boundary, so the separator only prevents
/// two files' text from fusing into an accidental match.
fn all_cli_source(root: &Path) -> String {
    let mut combined = String::new();
    for path in cli_files(root) {
        if let Ok(source) = fs::read_to_string(&path) {
            combined.push_str(&source);
            combined.push('\n');
        }
    }
    combined
}

#[test]
fn cli_messages_do_not_say_clause() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut violations = Vec::new();
    let files = cli_files(&root);
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
/// code path noticed. This one was duplicated across the parse and validate
/// paths before this guard existed. Its former `setup plugins-dir` siblings
/// were deleted whole with the feature (git-plugin-distribution Task 4:
/// install is `omarchy plugin add` now) rather than left here unreachable.
#[test]
fn cli_messages_are_defined_once() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let watched = ["config apply requires stdin, file <path>, or json <value>"];
    let mut violations = Vec::new();
    let files = cli_files(&root);
    let mut totals = vec![0usize; watched.len()];
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
/// This is the whole remaining set of messages that name a remedy — every
/// other CLI error either states a grammar mistake the user can see from
/// their own command line, or a condition whose remedy depends on facts the
/// helper does not have. The `setup plugins-dir` writability/existence
/// messages this guard used to also check were deleted whole with the
/// feature (git-plugin-distribution Task 4: install is `omarchy plugin add`
/// now), including the writability probe that could not tell a permission
/// problem from a stale probe file and so deliberately named no fix.
#[test]
fn cli_messages_that_can_name_a_fix_do() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = all_cli_source(&root);
    let needle = "{} login executable was not found; install the provider CLI first";
    assert!(
        source.contains(needle),
        "missing fix-naming message: {needle}"
    );
}

/// `docs/guide/commands.md` documents the same command grammar the CLI's own help
/// text implements, and it drifted once: the v11-06 plan's own measurement
/// found "Status clauses" live in this file after `clause` was renamed to
/// `argument` in the help text it describes. None of the guards above can
/// see prose in a Markdown file, so this one reads `docs/guide/commands.md`
/// directly rather than relying on the `src/cli/**` walk to catch prose that
/// never lives under `src/cli/` in the first place.
#[test]
fn docs_commands_do_not_say_clause() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/guide/commands.md");
    let source = fs::read_to_string(&path).expect("read docs/guide/commands.md");
    let lowered = source.to_lowercase();
    let violations: Vec<&str> = lowered
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|token| *token == "clause" || *token == "clauses")
        .collect();
    assert!(
        violations.is_empty(),
        "docs/guide/commands.md still says clause ({} occurrences)",
        violations.len()
    );
}
