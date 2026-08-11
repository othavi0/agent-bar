//! Binary-level grammar checks for `agent-bar-bundle` (monorepo migration
//! Task 2). `stamp` is the binary's only verb; the tarball `release`
//! packaging command was removed with the rest of the tarball machinery,
//! and `assemble`/`output` went with the separate-tree assembly step.

use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_agent-bar-bundle")
}

#[test]
fn help_exits_zero_and_names_only_stamp() {
    let out = Command::new(bin()).arg("help").output().expect("spawn");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("stamp"));
    assert!(stdout.contains("source-commit"));
    assert!(!stdout.contains("assemble"));
    assert!(!stdout.contains("output"));
    assert!(!stdout.contains("release bundle"));
    assert!(!stdout.contains("release-notes"));
}

#[test]
fn stamp_rejects_missing_keywords() {
    let out = Command::new(bin()).args(["stamp"]).output().expect("spawn");
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("missing") || err.contains("source-commit") || err.contains("failed"),
        "stderr={err}"
    );
}

#[test]
fn assemble_no_longer_a_known_command() {
    let out = Command::new(bin())
        .args([
            "assemble",
            "output",
            "/tmp/x",
            "source-commit",
            "0123456789abcdef0123456789abcdef01234567",
        ])
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn release_no_longer_a_known_command() {
    let out = Command::new(bin())
        .args([
            "release",
            "bundle",
            "/tmp/x",
            "output",
            "/tmp/y",
            "source-commit",
            "0123456789abcdef0123456789abcdef01234567",
            "release-notes",
            "/tmp/n",
        ])
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn unknown_command_exits_two() {
    let out = Command::new(bin())
        .arg("not-a-command")
        .output()
        .expect("spawn");
    assert_eq!(out.status.code(), Some(2));
}
