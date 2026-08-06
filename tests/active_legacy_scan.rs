//! Active legacy gate (TEST-030 / TEST-031 / TEST-034).
//!
//! Closed behavioral set and path inventory from
//! `docs/specs/v10/07-testing-and-acceptance.md` and the locked deletion map in
//! `docs/specs/v10/09-implementation-plan.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Locked deletion inventory — every path must be absent after Task 19.
const LOCKED_DELETION_PATHS: &[&str] = &[
    "src/action_right.rs",
    "src/tui",
    "src/usage",
    "src/waybar",
    "src/formatters",
    "src/cache.rs",
    "src/cli.rs",
    "src/config.rs",
    "src/watch.rs",
    "src/theme.rs",
    "src/platform.rs",
    "src/runtime.rs",
    "src/term_prompt.rs",
    "src/config_cmd.rs",
    "src/http.rs",
    "src/install.rs",
    "src/logger.rs",
    "src/notify.rs",
    "src/omarchy_integration.rs",
    "src/settings.rs",
    "src/test_support.rs",
    "src/setup.rs",
    "src/update.rs",
    "src/uninstall.rs",
    "src/doctor.rs",
    "src/providers/amp_cli.rs",
    "src/providers/base.rs",
    "src/providers/error.rs",
    "src/providers/extras.rs",
    "src/providers/grok_cli.rs",
    "src/providers/types.rs",
    "assets/omarchy/Widget.qml",
    "src/snapshots/agent_bar__omarchy_integration__tests__omarchy_manifest.snap",
    "tests/golden.rs",
    "docs/assets/agent-bar-banner.png",
    "docs/waybar-contract.md",
    "icons/amp-icon.svg",
    "icons/claude-code-icon.png",
    "icons/codex-icon.png",
    "icons/grok-icon.svg",
    "packaging/aur/.SRCINFO",
    "packaging/aur/PKGBUILD",
    "packaging/aur/agent-bar-bin.install",
    "build.rs",
    "install.sh",
];

/// Forbidden tokens from the closed behavioral set (exact substring match).
const FORBIDDEN_TOKENS: &[&str] = &[
    "src/tui",
    "src/usage",
    "src/waybar",
    "custom/agent-bar",
    "format waybar",
    "format_for_waybar",
    "render_pango",
    "waybar_contract",
    "waybar_integration",
    "WAYBAR_",
    "SIGUSR2",
    "menu-font",
    "action-right",
    "--waybar-dir",
    "agent-bar menu",
    "usage.redb",
    "UsageRecord",
    "UsageSummary",
    "ExtraUsage",
    "fxRate",
    "creditsBalance",
    "freeRemaining",
    "AGENT_BAR_DATA",
    "AGENT_BAR_ASSET_DIR",
    "AGENT_BAR_FORCE_COMPILED",
    "InstallKind::System",
    "InstallKind::DevGit",
    "InstallKind::ManagedGit",
    "InstallKind::Standalone",
    "ManagedGit",
    "/usr/bin/agent-bar",
    ".local/bin/agent-bar",
    ".local/share/agent-bar",
    "agent-bar-bin",
    "package.metadata.binstall",
    "packaging/aur",
    "x86_64-unknown-linux-musl",
    "cargo-zigbuild",
    "cargo binstall agent-bar",
    "BIN_DIR=\"$HOME/.local/bin\"",
    "wire Waybar",
    "Waybar is Wayland-only",
    "ratatui",
    "crossterm",
    "tui-input",
    "throbber-widgets-tui",
    "tachyonfx",
    "redb",
    "postcard",
    "tar.zst",
    "releases/download/",
    "agent-bar-maintenance-worker",
    "RELEASES_API_URL",
];

/// Relative prefixes excluded from content scans (TEST-031 historical cuts).
fn is_historical_cut(rel: &str) -> bool {
    if rel.starts_with("docs/superpowers/") {
        return true;
    }
    // Dated, frozen release-note snapshots describe what a past,
    // already-published version actually shipped (e.g. `docs/releases/10.0.0.md`
    // naming that release's real `.tar.zst` asset). Same rationale as the
    // ADR/CHANGELOG historical-slice exclusions below: never rewritten.
    // `docs/releases/README.md` is the live index for the current pipeline
    // (Task 8), not a dated cut, so it is deliberately NOT covered here.
    if (rel.starts_with("docs/releases/") && rel != "docs/releases/README.md")
        || rel.starts_with("docs/history/")
    {
        return true;
    }
    if rel == "docs/adr/0001-omarchy-right-click-settings.md"
        || rel == "docs/adr/0002-config-cli-dual-write.md"
        || rel == "docs/adr/0003-cli-help-hide-internals.md"
    {
        return true;
    }
    false
}

/// Fixture / contract paths allowed to mention otherwise-forbidden tokens.
fn is_allowlisted_path(rel: &str) -> bool {
    if rel.starts_with("tests/fixtures/migration/") {
        return true;
    }
    matches!(
        rel,
        "tests/fixtures/amp/usage-legacy-dollars.txt"
            | "tests/fixtures/amp/usage-free-pct.txt"
            | "tests/fixtures/status-v2/money-field.json"
            | "assets/omarchy/manifest.json"
            | "schemas/settings-v1.schema.json"
            | "schemas/status-v2.schema.json"
    )
}

/// Active documentation may state negative-removal policy; only those lines pass.
fn is_negative_removal_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("removed")
        || lower.contains("delete")
        || lower.contains("no longer")
        || lower.contains("must not")
        || lower.contains("never")
        || lower.contains("without")
        || lower.contains("do not")
        || lower.contains("forbids")
        || lower.contains("forbidden")
        || lower.contains("legacy")
        || lower.contains("historical")
        || lower.contains("superseded")
        || lower.contains("v9")
        || lower.contains("not retain")
        || lower.contains("no tui")
        || lower.contains("no waybar")
        || lower.contains("excludes")
        || lower.contains("exclusion")
        || lower.contains("gate rejects")
        || lower.contains("scan")
}

fn skip_dir_name(name: &str) -> bool {
    matches!(
        name,
        "target" | ".git" | ".grok" | "node_modules" | ".cargo" | "CACHEDIR.TAG"
    )
}

fn collect_active_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') && name != ".github" {
                // Still walk .github; skip other dot dirs.
                if path.is_dir() && name != ".github" {
                    continue;
                }
            }
            if path.is_dir() {
                if skip_dir_name(&name) {
                    continue;
                }
                stack.push(path);
            } else if path.is_file() {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn rel_str(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.display().to_string())
}

fn is_scannable_file(rel: &str) -> bool {
    if rel.ends_with(".png")
        || rel.ends_with(".jpg")
        || rel.ends_with(".svg")
        || rel.ends_with(".ico")
        || rel.ends_with(".zip")
        || rel.ends_with(".tar")
        || rel.ends_with(".zst")
        || rel.ends_with(".lock")
        || rel.ends_with(".rlib")
        || rel.ends_with(".rmeta")
        || rel.ends_with(".o")
        || rel.ends_with(".so")
    {
        return false;
    }
    // Always scan sources, tests, docs, manifests, workflows, packaging, scripts.
    rel.starts_with("src/")
        || rel.starts_with("tests/")
        || rel.starts_with("assets/")
        || rel.starts_with("docs/")
        || rel.starts_with("scripts/")
        || rel.starts_with("schemas/")
        || rel.starts_with("packaging/")
        || rel.starts_with(".github/")
        || matches!(
            rel,
            "Cargo.toml"
                | "README.md"
                | "PRODUCT.md"
                | "CONTEXT.md"
                | "CONTRIBUTING.md"
                | "CLAUDE.md"
                | "AGENTS.md"
                | "CHANGELOG.md"
                | "install.sh"
                | "LICENSE"
        )
}

/// CHANGELOG: only Unreleased is active; release sections 9.0.0 and older are cuts.
fn changelog_active_slice(content: &str) -> String {
    let mut active = String::new();
    let mut in_unreleased = false;
    for line in content.lines() {
        if line.starts_with("## [Unreleased]") || line.starts_with("## Unreleased") {
            in_unreleased = true;
            active.push_str(line);
            active.push('\n');
            continue;
        }
        if line.starts_with("## [") || (line.starts_with("## ") && !line.starts_with("###")) {
            // Leaving Unreleased into a version section ends the active region.
            if in_unreleased {
                break;
            }
        }
        if in_unreleased {
            active.push_str(line);
            active.push('\n');
        }
    }
    // If no Unreleased header, treat whole file as active (fail closed for Task 20).
    if active.is_empty() {
        content.to_owned()
    } else {
        active
    }
}

fn scan_content_for_tokens(rel: &str, content: &str, violations: &mut Vec<String>) {
    if is_historical_cut(rel) || is_allowlisted_path(rel) {
        return;
    }

    let body = if rel == "CHANGELOG.md" {
        changelog_active_slice(content)
    } else {
        content.to_owned()
    };

    // Self-scan of this gate file documents the forbidden set; skip token hits here.
    if rel == "tests/active_legacy_scan.rs" {
        return;
    }

    let is_active_doc = rel.ends_with(".md")
        && (rel.starts_with("docs/")
            || matches!(
                rel,
                "README.md"
                    | "PRODUCT.md"
                    | "CONTEXT.md"
                    | "CONTRIBUTING.md"
                    | "CLAUDE.md"
                    | "AGENTS.md"
                    | "CHANGELOG.md"
            ));

    // Specs under docs/specs/v10 describe the removal contract; allow tokens there.
    if rel.starts_with("docs/specs/v10/") {
        return;
    }

    for token in FORBIDDEN_TOKENS {
        if !body.contains(token) {
            continue;
        }
        for (idx, line) in body.lines().enumerate() {
            if !line.contains(token) {
                continue;
            }
            if is_active_doc && is_negative_removal_line(line) {
                continue;
            }
            violations.push(format!(
                "{rel}:{} contains forbidden token `{token}`: {}",
                idx + 1,
                line.trim()
            ));
        }
    }
}

/// Direct Cargo dependencies and their required v10 owners (TEST-034).
fn required_dependency_owners() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("serde", "status/settings/cache/plugin JSON contracts"),
        ("serde_json", "status/settings/cache/plugin JSON contracts"),
        ("thiserror", "typed domain errors across modules"),
        ("time", "clock, status timestamps, provider windows"),
        ("tempfile", "atomic writes and isolated tests"),
        ("log", "stderr diagnostics for cache/notifications"),
        ("env_logger", "helper binary stderr logger init"),
        ("tokio", "async process/HTTP collection runtime"),
        ("reqwest", "Claude HTTP collector and update check"),
        ("futures", "HTTP body streaming in providers/http"),
        ("regex", "provider stdout/session parsers"),
        ("semver", "update version comparison"),
        ("fs2", "exclusive maintenance gate lock"),
        ("sha2", "bundle and ownership hashes"),
        ("assert_cmd", "CLI integration tests"),
        ("predicates", "CLI integration tests"),
        ("tokio", "test-util runtime in tests"),
        ("jsonschema", "schema_contract fixture gate"),
    ])
}

/// Parse direct dependency names declared under a single `[section]` header.
fn parse_direct_deps_of_section(cargo_toml: &str, wanted_section: &str) -> BTreeSet<String> {
    let mut deps = BTreeSet::new();
    let mut section = "";
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            section = trimmed;
            continue;
        }
        if section != wanted_section {
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((name, _)) = trimmed.split_once('=') {
            let name = name.trim().trim_matches('"');
            if !name.is_empty() {
                deps.insert(name.to_owned());
            }
        }
    }
    deps
}

fn parse_direct_deps(cargo_toml: &str) -> BTreeSet<String> {
    let mut deps = parse_direct_deps_of_section(cargo_toml, "[dependencies]");
    deps.extend(parse_direct_deps_of_section(
        cargo_toml,
        "[dev-dependencies]",
    ));
    deps
}

/// Recursively collect `.rs` files under `root/rel_dir`.
fn walkdir_rs_files(root: &Path, rel_dir: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.join(rel_dir)];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }
    out
}

#[test]
fn active_legacy_scan_locked_paths_absent() {
    let root = workspace_root();
    let mut present = Vec::new();
    for rel in LOCKED_DELETION_PATHS {
        let path = root.join(rel);
        if path.exists() {
            present.push(*rel);
        }
    }
    assert!(
        present.is_empty(),
        "locked deletion inventory still present:\n  - {}",
        present.join("\n  - ")
    );
}

#[test]
fn active_legacy_scan_forbidden_tokens_absent() {
    let root = workspace_root();
    let mut violations = Vec::new();
    for path in collect_active_files(&root) {
        let rel = rel_str(&root, &path);
        if !is_scannable_file(&rel) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        scan_content_for_tokens(&rel, &content, &mut violations);
    }
    assert!(
        violations.is_empty(),
        "active legacy tokens remain ({}):\n  - {}",
        violations.len(),
        violations.join("\n  - ")
    );
}

#[test]
fn active_legacy_scan_cargo_and_install_contract() {
    let root = workspace_root();
    let cargo = fs::read_to_string(root.join("Cargo.toml")).expect("Cargo.toml");
    assert!(
        cargo.contains(r#"description = "LLM quota monitor for Claude, Codex, Amp, and Grok.""#),
        "Cargo.toml must use the exact package description"
    );
    assert!(
        !cargo.contains("package.metadata.binstall"),
        "Cargo.toml must not declare package.metadata.binstall"
    );
    assert!(
        !cargo.contains("[package.metadata.binstall]"),
        "Cargo.toml must not declare binstall metadata"
    );

    let forbidden_deps = [
        "ratatui",
        "crossterm",
        "tui-input",
        "throbber-widgets-tui",
        "tachyonfx",
        "redb",
        "postcard",
        "async-trait",
        "serial_test",
        "temp-env",
        "insta",
        "indexmap",
        "wiremock",
        "filetime",
    ];
    let deps = parse_direct_deps(&cargo);
    let mut leftover = Vec::new();
    for d in forbidden_deps {
        if deps.contains(d) {
            leftover.push(d);
        }
    }
    assert!(
        leftover.is_empty(),
        "removed dependencies still declared: {leftover:?}"
    );

    let owners = required_dependency_owners();
    let mut missing_owner = Vec::new();
    for dep in &deps {
        if !owners.contains_key(dep.as_str()) {
            missing_owner.push(dep.clone());
        }
    }
    assert!(
        missing_owner.is_empty(),
        "direct Cargo deps without documented v10 owner: {missing_owner:?}"
    );
}

/// TEST-034 hardening: a documented owner is not proof of real use — every
/// `[dependencies]` entry must actually appear (as a Rust identifier) in src/.
/// `[dev-dependencies]` are intentionally excluded: they are expected to be
/// referenced only from tests/.
#[test]
fn dependencies_are_actually_used_in_src() {
    let root = workspace_root();
    let cargo = fs::read_to_string(root.join("Cargo.toml")).expect("Cargo.toml");
    let deps = parse_direct_deps_of_section(&cargo, "[dependencies]");

    let mut sources = String::new();
    for entry in walkdir_rs_files(&root, "src") {
        sources.push_str(&fs::read_to_string(&entry).expect("read src file"));
        sources.push('\n');
    }

    let mut unused = Vec::new();
    for dep in &deps {
        let ident = dep.replace('-', "_");
        if !sources.contains(&ident) {
            unused.push(dep.clone());
        }
    }
    assert!(
        unused.is_empty(),
        "dependencies declared in Cargo.toml [dependencies] but never referenced in src/ \
         (remove them or use them): {unused:?}"
    );
}
