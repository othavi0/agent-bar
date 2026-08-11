//! Internal release-stamp builder (not installed in the plugin bundle).
//!
//! Grammar (exact):
//!   agent-bar-bundle stamp source-commit <40-hex>
//!
//! CI stamps the repo root in place, and the release workflow commits and
//! pushes this repository's own `master` -- there is no separate
//! assemble/output step or local release/archive verb.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use agent_bar::plugin::bundle::BundleBuilder;

fn main() -> ExitCode {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("{}", usage());
        return ExitCode::from(2);
    }
    match args[0].as_str() {
        "stamp" => match run_stamp(&mut args[1..]) {
            Ok(path) => {
                eprintln!("stamped {}", path.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("stamp failed: {e}");
                ExitCode::from(1)
            }
        },
        "--help" | "help" => {
            print!("{}", usage());
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unknown command '{other}'\n{}", usage());
            ExitCode::from(2)
        }
    }
}

fn usage() -> String {
    "agent-bar-bundle stamp source-commit <40-lowercase-hex>\n".to_string()
}

fn run_stamp(args: &mut [String]) -> Result<PathBuf, String> {
    let map = parse_kv(args, &["source-commit"])?;
    let source_commit = map
        .get("source-commit")
        .ok_or("missing source-commit")?
        .clone();

    let repo_root = repo_root()?;
    let helper = find_helper_bin(&repo_root)?;
    let version = env!("CARGO_PKG_VERSION").to_string();
    let builder = BundleBuilder::new(version, source_commit).map_err(|e| e.to_string())?;
    builder
        .stamp(&repo_root, &helper)
        .map_err(|e| e.to_string())?;
    Ok(repo_root)
}

/// Parse alternating keyword value pairs. Keywords must match exactly once.
fn parse_kv(
    args: &[String],
    required: &[&str],
) -> Result<std::collections::HashMap<String, String>, String> {
    if !args.len().is_multiple_of(2) {
        return Err("arguments must be keyword/value pairs".into());
    }
    let mut map = std::collections::HashMap::new();
    let mut i = 0;
    while i < args.len() {
        let key = args[i].as_str();
        if !required.contains(&key) {
            return Err(format!("unknown or misplaced keyword '{key}'"));
        }
        if map.contains_key(key) {
            return Err(format!("duplicate keyword '{key}'"));
        }
        map.insert(key.to_string(), args[i + 1].clone());
        i += 2;
    }
    for r in required {
        if !map.contains_key(*r) {
            return Err(format!("missing keyword '{r}'"));
        }
    }
    Ok(map)
}

fn repo_root() -> Result<PathBuf, String> {
    // Prefer CARGO_MANIFEST_DIR when invoked via cargo run; else walk from cwd.
    if let Ok(m) = env::var("CARGO_MANIFEST_DIR") {
        return Ok(PathBuf::from(m));
    }
    let mut dir = env::current_dir().map_err(|e| e.to_string())?;
    loop {
        if dir.join("Cargo.toml").is_file() && dir.join("manifest.json").is_file() {
            return Ok(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    Err("could not locate repository root (Cargo.toml + manifest.json)".into())
}

fn find_helper_bin(repo_root: &Path) -> Result<PathBuf, String> {
    let candidates = [
        repo_root.join("target/release/agent-bar"),
        repo_root.join("target/debug/agent-bar"),
    ];
    for c in candidates {
        if c.is_file() {
            return Ok(c);
        }
    }
    Err(
        "helper binary not found; build with `cargo build --release` first \
         (looked in target/release/agent-bar and target/debug/agent-bar)"
            .into(),
    )
}
