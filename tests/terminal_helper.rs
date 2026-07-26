//! Argv-safe `scripts/agent-bar-open-terminal` contract (Task 13).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_helper_source() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/agent-bar-open-terminal")
}

fn write_executable(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("mkdir");
    }
    fs::write(path, body).expect("write");
    let mut perms = fs::metadata(path).expect("meta").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("chmod");
}

/// Minimal plugin root: scripts/helper + bin/agent-bar stub + PATH fake xdg-terminal-exec.
fn fixture_plugin_root(tmp: &Path) -> PathBuf {
    let plugin = tmp.join("agent-bar.usage");
    let scripts = plugin.join("scripts");
    let bin = plugin.join("bin");
    fs::create_dir_all(&scripts).unwrap();
    fs::create_dir_all(&bin).unwrap();

    let helper_src = fs::read_to_string(repo_helper_source()).expect("read helper source");
    write_executable(&scripts.join("agent-bar-open-terminal"), &helper_src);

    // Private helper stub — must be regular + executable.
    write_executable(
        &bin.join("agent-bar"),
        "#!/usr/bin/env bash\necho stub-agent-bar \"$@\"\n",
    );

    plugin
}

fn fake_xdg_on_path(tmp: &Path, argv_out: &Path) -> PathBuf {
    let path_dir = tmp.join("pathbin");
    fs::create_dir_all(&path_dir).unwrap();
    // Records NUL-separated argv then exits 0 (stands in for xdg-terminal-exec).
    let script = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
: > "{out}"
for a in "$@"; do
  printf '%s\0' "$a" >> "{out}"
done
exit 0
"#,
        out = argv_out.display()
    );
    write_executable(&path_dir.join("xdg-terminal-exec"), &script);
    path_dir
}

fn run_helper(plugin: &Path, path_dir: &Path, args: &[&str]) -> std::process::Output {
    let helper = plugin.join("scripts/agent-bar-open-terminal");
    let path = format!(
        "{}:{}",
        path_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    Command::new(&helper)
        .args(args)
        .env("PATH", path)
        .output()
        .expect("spawn helper")
}

fn read_nul_argv(path: &Path) -> Vec<String> {
    let bytes = fs::read(path).unwrap_or_default();
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

#[test]
fn helper_rejects_wrong_arity_and_verb() {
    let tmp = tempfile::tempdir().unwrap();
    let plugin = fixture_plugin_root(tmp.path());
    let argv_out = tmp.path().join("argv.bin");
    let path_dir = fake_xdg_on_path(tmp.path(), &argv_out);

    let out = run_helper(&plugin, &path_dir, &[]);
    assert_eq!(out.status.code(), Some(2));

    let out = run_helper(&plugin, &path_dir, &["login"]);
    assert_eq!(out.status.code(), Some(2));

    let out = run_helper(&plugin, &path_dir, &["menu", "claude"]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Usage") || stderr.contains("login"));
}

#[test]
fn helper_rejects_unknown_provider() {
    let tmp = tempfile::tempdir().unwrap();
    let plugin = fixture_plugin_root(tmp.path());
    let argv_out = tmp.path().join("argv.bin");
    let path_dir = fake_xdg_on_path(tmp.path(), &argv_out);

    let out = run_helper(&plugin, &path_dir, &["login", "copilot"]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("invalid provider"));
    assert!(!argv_out.exists());
}

#[test]
fn helper_requires_executable_private_helper() {
    let tmp = tempfile::tempdir().unwrap();
    let plugin = fixture_plugin_root(tmp.path());
    let argv_out = tmp.path().join("argv.bin");
    let path_dir = fake_xdg_on_path(tmp.path(), &argv_out);

    // Remove execute bit from private helper.
    let private = plugin.join("bin/agent-bar");
    let mut perms = fs::metadata(&private).unwrap().permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&private, perms).unwrap();

    let out = run_helper(&plugin, &path_dir, &["login", "claude"]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("not executable") || stderr.contains("missing"));
}

#[test]
fn helper_execs_exact_xdg_terminal_argv() {
    let tmp = tempfile::tempdir().unwrap();
    let plugin = fixture_plugin_root(tmp.path());
    let argv_out = tmp.path().join("argv.bin");
    let path_dir = fake_xdg_on_path(tmp.path(), &argv_out);

    let out = run_helper(&plugin, &path_dir, &["login", "codex"]);
    assert!(
        out.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );

    let argv = read_nul_argv(&argv_out);
    let plugin_root = fs::canonicalize(&plugin).unwrap();
    let expected_helper = plugin_root.join("bin/agent-bar");

    assert_eq!(argv.get(0).map(String::as_str), Some("--app-id=org.omarchy.terminal"));
    assert_eq!(argv.get(1).map(String::as_str), Some("--title=Agent Bar Login"));
    assert_eq!(argv.get(2).map(String::as_str), Some("--"));
    assert_eq!(
        argv.get(3).map(String::as_str),
        Some(expected_helper.to_str().unwrap())
    );
    assert_eq!(argv.get(4).map(String::as_str), Some("login"));
    assert_eq!(argv.get(5).map(String::as_str), Some("codex"));
    assert_eq!(argv.len(), 6);
}

#[test]
fn helper_source_has_no_forbidden_patterns() {
    let src = fs::read_to_string(repo_helper_source()).unwrap();
    for needle in [
        "cmd=\"$*\"",
        "cmd=\"$@\"",
        "sh -c",
        "bash -lc",
        "eval ",
        "command -v agent-bar",
        "alacritty",
        "kitty",
        "foot",
        "ghostty",
        "wezterm",
    ] {
        assert!(
            !src.contains(needle),
            "helper must not contain forbidden pattern: {needle}"
        );
    }
    assert!(src.contains("xdg-terminal-exec"));
    assert!(src.contains("BASH_SOURCE"));
    assert!(src.contains("login"));
}
