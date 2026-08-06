//! Active documentation gates (DOC-001–DOC-004 / TEST-032 / TEST-033).
//!
//! - Every executable helper command example parses under the v10 grammar.
//! - Every status/settings JSON example validates against checked-in schemas.
//! - Every relative Markdown link resolves inside the repository.
//! - Active product docs no longer carry pre-implementation target-only banners.

use std::fs;
use std::path::{Path, PathBuf};

use agent_bar::cli::parse;
use jsonschema::Validator;
use serde_json::Value;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Product and engineering documentation under the active language/legacy gate.
fn active_doc_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let top_level = [
        "README.md",
        "PRODUCT.md",
        "CONTEXT.md",
        "CONTRIBUTING.md",
        "CLAUDE.md",
        "AGENTS.md",
        "CHANGELOG.md",
    ];
    for rel in top_level {
        let path = root.join(rel);
        if path.is_file() {
            paths.push(path);
        }
    }

    // Active docs under docs/, excluding historical cuts and the specification package.
    let docs_root = root.join("docs");
    if docs_root.is_dir() {
        let mut stack = vec![docs_root];
        while let Some(dir) = stack.pop() {
            let entries = match fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().into_owned();
                if path.is_dir() {
                    if name == "superpowers" || name == "specs" {
                        continue;
                    }
                    stack.push(path);
                    continue;
                }
                if !path.is_file() || !name.ends_with(".md") {
                    continue;
                }
                // ADR bodies 0001–0003 remain historical (DOC-003).
                let rel = path
                    .strip_prefix(root)
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                if matches!(
                    rel.as_str(),
                    "docs/adr/0001-omarchy-right-click-settings.md"
                        | "docs/adr/0002-config-cli-dual-write.md"
                        | "docs/adr/0003-cli-help-hide-internals.md"
                ) {
                    continue;
                }
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
}

fn rel_str(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.display().to_string())
}

fn read_text(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| {
        panic!("failed to read {}: {err}", path.display());
    })
}

/// CHANGELOG: only the Unreleased section is active (DOC-003).
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
        if (line.starts_with("## [") || (line.starts_with("## ") && !line.starts_with("###")))
            && in_unreleased
        {
            break;
        }
        if in_unreleased {
            active.push_str(line);
            active.push('\n');
        }
    }
    if active.is_empty() {
        content.to_owned()
    } else {
        active
    }
}

fn active_body(root: &Path, path: &Path) -> String {
    let content = read_text(path);
    let rel = rel_str(root, path);
    if rel == "CHANGELOG.md" {
        changelog_active_slice(&content)
    } else {
        content
    }
}

#[derive(Debug, Clone)]
struct Fence {
    lang: String,
    body: String,
    start_line: usize,
}

fn extract_fences(content: &str) -> Vec<Fence> {
    let mut out = Vec::new();
    let mut lines = content.lines().enumerate().peekable();
    while let Some((idx, line)) = lines.next() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("```") {
            continue;
        }
        let lang = trimmed.trim_start_matches('`').trim().to_ascii_lowercase();
        let start_line = idx + 1;
        let mut body = String::new();
        for (body_idx, body_line) in lines.by_ref() {
            if body_line.trim_start().starts_with("```") {
                let _ = body_idx;
                break;
            }
            body.push_str(body_line);
            body.push('\n');
        }
        out.push(Fence {
            lang,
            body,
            start_line,
        });
    }
    out
}

/// Non-helper commands / shell tools that appear in bash fences but are not
/// private-helper grammar.
fn is_non_helper_line(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return true;
    }
    let first = t.split_whitespace().next().unwrap_or("");
    matches!(
        first,
        "curl"
            | "less"
            | "bash"
            | "find"
            | "cargo"
            | "shellcheck"
            | "omarchy"
            | "qmllint"
            | "git"
            | "readelf"
            | "file"
            | "test"
            | "mkdir"
            | "cp"
            | "rm"
            | "set"
            | "SOURCE_COMMIT"
            | "QT_QPA_PLATFORM"
            | "PLUGIN"
    ) || t.starts_with("PLUGIN=")
        || t.starts_with("SOURCE_COMMIT=")
        || t.starts_with("export ")
}

/// Lines that are grammar synopsis, not executable examples.
fn is_synopsis_placeholder(line: &str) -> bool {
    let t = line.trim();
    t.contains('|')
        || t.contains('<')
        || t.contains('>')
        || t.contains('[')
        || t.contains(']')
        || t.contains("...")
}

/// Extract private-helper argv from one documentation line.
///
/// Accepts:
/// - `"$PLUGIN" status format json`
/// - `$PLUGIN status …`
/// - `agent-bar status …`
/// - bare synopsis forms without placeholders
fn extract_helper_argv(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    if trimmed.is_empty() || is_non_helper_line(trimmed) {
        return None;
    }
    if is_synopsis_placeholder(trimmed) {
        return None;
    }

    let mut rest = trimmed;
    // Strip optional surrounding shell prefixes such as env assignments on the
    // same line (none expected). Handle quoted/unquoted plugin path forms.
    if rest.starts_with("\"$PLUGIN\"") {
        rest = rest["\"$PLUGIN\"".len()..].trim_start();
    } else if rest.starts_with("$PLUGIN") {
        rest = rest["$PLUGIN".len()..].trim_start();
    } else if let Some(after) = rest.strip_prefix("agent-bar") {
        // Reject archive/product filenames like `othavi0.agent-bar-10.0.0-…`.
        if !after.is_empty() && !after.starts_with(char::is_whitespace) {
            return None;
        }
        rest = after.trim_start();
    } else if rest.starts_with("\"$HOME/.config/omarchy/plugins/othavi0.agent-bar/bin/agent-bar\"")
    {
        rest = rest["\"$HOME/.config/omarchy/plugins/othavi0.agent-bar/bin/agent-bar\"".len()..]
            .trim_start();
    } else {
        return None;
    }

    if rest.is_empty() {
        // Bare `agent-bar` / `"$PLUGIN"` equals default status.
        return Some(Vec::new());
    }

    Some(shell_split(rest))
}

/// Minimal shell-word split supporting single-quoted and double-quoted tokens.
fn shell_split(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut chars = input.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;
    while let Some(ch) = chars.next() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
            }
            '"' if !in_single => {
                in_double = !in_double;
            }
            c if c.is_whitespace() && !in_single && !in_double => {
                if !cur.is_empty() {
                    words.push(std::mem::take(&mut cur));
                }
            }
            '\\' if !in_single => {
                if let Some(next) = chars.next() {
                    cur.push(next);
                }
            }
            other => cur.push(other),
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

fn compile_schema(root: &Path, relative: &str) -> Validator {
    let path = root.join(relative);
    let raw = read_text(&path);
    let schema: Value = serde_json::from_str(&raw).unwrap_or_else(|err| {
        panic!("failed to parse schema {}: {err}", path.display());
    });
    jsonschema::validator_for(&schema).unwrap_or_else(|err| {
        panic!("failed to compile schema {}: {err}", path.display());
    })
}

fn looks_like_status_v2(value: &Value) -> bool {
    value
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .is_some_and(|v| v == 2)
        && value.get("helperVersion").is_some()
        && value.get("providers").is_some()
}

fn looks_like_settings_v1(value: &Value) -> bool {
    value
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .is_some_and(|v| v == 1)
        && value.get("providers").is_some()
        && value.get("display").is_some()
}

/// Phrases that mark pre-implementation target-only documentation (DOC-005 → DOC-004).
const TARGET_ONLY_BANNERS: &[&str] = &[
    "Target documentation for v10",
    "Target product contract for v10",
    "Target v10 vocabulary",
    "not yet implemented on the specification branch",
    "not yet implemented on the",
    "v9 remains the current release",
    "v9 remains implemented until the plan completes",
    "v9 source tree remains transitional",
    "These files describe the approved v10 target",
    "Target v10 command contract",
    "Target v10 architecture",
    "Target v10 schema",
    "Target v10 integration contract",
    "Target v10 provider adapter contract",
    "Target v10 integration for the locally verified",
    "Target v10 runtime and ownership model",
    "Target v10 troubleshooting paths",
    "the private helper commands below are not\n> available until the implementation plan completes",
    "the private helper commands below are not available until",
    "code may still contradict this file",
];

#[test]
fn active_docs_command_examples_parse() {
    let root = workspace_root();
    let mut examples: Vec<(String, usize, Vec<String>)> = Vec::new();
    let mut failures = Vec::new();

    for path in active_doc_paths(&root) {
        let rel = rel_str(&root, &path);
        let body = active_body(&root, &path);
        for fence in extract_fences(&body) {
            if fence.lang != "bash" && fence.lang != "text" && !fence.lang.is_empty() {
                continue;
            }
            // Only parse bash fences and plain `agent-bar` text synopsis that are
            // fully concrete. Skip pure path/text blocks without helper tokens.
            if fence.lang == "text" && !fence.body.contains("agent-bar") {
                continue;
            }
            for (offset, line) in fence.body.lines().enumerate() {
                let line_no = fence.start_line + offset + 1;
                let Some(argv) = extract_helper_argv(line) else {
                    continue;
                };
                examples.push((rel.clone(), line_no, argv.clone()));
                if let Err(err) = parse(argv.iter().map(String::as_str)) {
                    failures.push(format!(
                        "{rel}:{line_no}: failed to parse `{}`: {err:?}",
                        line.trim()
                    ));
                }
            }
        }
    }

    assert!(
        !examples.is_empty(),
        "expected at least one helper command example in active docs"
    );
    assert!(
        failures.is_empty(),
        "active command examples failed CLI parse ({} of {}):\n  - {}",
        failures.len(),
        examples.len(),
        failures.join("\n  - ")
    );
}

#[test]
fn active_docs_json_examples_validate() {
    let root = workspace_root();
    let status_schema = compile_schema(&root, "schemas/status-v2.schema.json");
    let settings_schema = compile_schema(&root, "schemas/settings-v1.schema.json");
    let mut validated = 0usize;
    let mut failures = Vec::new();

    for path in active_doc_paths(&root) {
        let rel = rel_str(&root, &path);
        let body = active_body(&root, &path);
        for fence in extract_fences(&body) {
            if fence.lang != "json" {
                continue;
            }
            let trimmed = fence.body.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Incomplete placeholder JSON must not appear in active docs.
            if trimmed.contains("...") {
                failures.push(format!(
                    "{rel}:{}: JSON example contains ellipsis placeholder",
                    fence.start_line
                ));
                continue;
            }
            let value: Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "{rel}:{}: JSON example is not valid JSON: {err}",
                        fence.start_line
                    ));
                    continue;
                }
            };

            if looks_like_status_v2(&value) {
                validated += 1;
                let errors: Vec<String> = status_schema
                    .iter_errors(&value)
                    .map(|e| format!("{e}"))
                    .collect();
                if !errors.is_empty() {
                    failures.push(format!(
                        "{rel}:{}: status-v2 example invalid:\n    - {}",
                        fence.start_line,
                        errors.join("\n    - ")
                    ));
                }
            } else if looks_like_settings_v1(&value) {
                validated += 1;
                let errors: Vec<String> = settings_schema
                    .iter_errors(&value)
                    .map(|e| format!("{e}"))
                    .collect();
                if !errors.is_empty() {
                    failures.push(format!(
                        "{rel}:{}: settings-v1 example invalid:\n    - {}",
                        fence.start_line,
                        errors.join("\n    - ")
                    ));
                }
            }
            // Other JSON (manifest shapes, partial objects) is structural prose only.
        }
    }

    assert!(
        validated >= 2,
        "expected at least one status-v2 and one settings-v1 JSON example; validated={validated}"
    );
    assert!(
        failures.is_empty(),
        "active JSON examples failed schema validation:\n  - {}",
        failures.join("\n  - ")
    );
}

#[test]
fn active_docs_internal_links_resolve() {
    let root = workspace_root();
    let mut checked = 0usize;
    let mut failures = Vec::new();
    // [text](target) — ignore images that use the same form when target is URL.
    let link_re = regex_lite_links();

    for path in active_doc_paths(&root) {
        let rel = rel_str(&root, &path);
        let body = active_body(&root, &path);
        let parent = path.parent().unwrap_or(root.as_path());
        for (line_idx, line) in body.lines().enumerate() {
            for target in link_re(line) {
                if target.is_empty()
                    || target.starts_with("http://")
                    || target.starts_with("https://")
                    || target.starts_with("mailto:")
                    || target.starts_with('#')
                {
                    continue;
                }
                // Strip anchor fragments.
                let path_part = target.split('#').next().unwrap_or(target.as_str());
                if path_part.is_empty() {
                    continue;
                }
                checked += 1;
                let resolved = if path_part.starts_with('/') {
                    // Repo-root absolute style is not used; treat as relative miss.
                    root.join(path_part.trim_start_matches('/'))
                } else {
                    parent.join(path_part)
                };
                if !resolved.exists() {
                    failures.push(format!(
                        "{rel}:{}: broken link `{}` -> {}",
                        line_idx + 1,
                        target,
                        resolved.display()
                    ));
                }
            }
        }
    }

    assert!(
        checked > 0,
        "expected at least one internal Markdown link in active docs"
    );
    assert!(
        failures.is_empty(),
        "broken internal links ({}):\n  - {}",
        failures.len(),
        failures.join("\n  - ")
    );
}

/// Lightweight Markdown link extractor without an extra dependency.
fn regex_lite_links() -> impl Fn(&str) -> Vec<String> {
    |line: &str| {
        let mut out = Vec::new();
        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            // Find "]("
            if bytes[i] == b']' && i + 1 < bytes.len() && bytes[i + 1] == b'(' {
                let start = i + 2;
                let mut j = start;
                while j < bytes.len() && bytes[j] != b')' {
                    j += 1;
                }
                if j < bytes.len() {
                    let target = &line[start..j];
                    // Skip reference-style empty and image titles with spaces only if pure URL.
                    if !target.is_empty() && !target.starts_with('<') {
                        // Drop optional title: url "title"
                        let url = target
                            .split_whitespace()
                            .next()
                            .unwrap_or(target)
                            .trim_matches('"')
                            .trim_matches('\'');
                        out.push(url.to_owned());
                    }
                    i = j + 1;
                    continue;
                }
            }
            i += 1;
        }
        out
    }
}

#[test]
fn active_docs_no_target_only_banners() {
    let root = workspace_root();
    let mut failures = Vec::new();
    for path in active_doc_paths(&root) {
        let rel = rel_str(&root, &path);
        // Spec package and historical cuts are excluded by active_doc_paths.
        let body = active_body(&root, &path);
        for banner in TARGET_ONLY_BANNERS {
            if body.contains(banner) {
                // Locate first line for a useful message.
                let line_no = body
                    .lines()
                    .position(|l| l.contains(banner) || banner.lines().any(|b| l.contains(b)))
                    .map(|i| i + 1)
                    .unwrap_or(1);
                failures.push(format!(
                    "{rel}:{line_no}: still contains target-only banner `{banner}`"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "active docs still claim pre-implementation target-only status (DOC-004):\n  - {}",
        failures.join("\n  - ")
    );
}

#[test]
fn active_docs_release_notes_10_0_0_exist() {
    let path = workspace_root().join("docs/releases/10.0.0.md");
    assert!(
        path.is_file(),
        "missing tracked release notes at docs/releases/10.0.0.md"
    );
    let body = read_text(&path);
    assert!(
        body.contains("10.0.0"),
        "release notes must mention version 10.0.0"
    );
    // Historical document: 10.0.0 shipped under the original plugin ID and
    // keeps it (the ID became othavi0.agent-bar on 2026-08-06).
    assert!(
        body.contains("agent-bar.usage"),
        "release notes must name the plugin product agent-bar.usage"
    );
    assert!(
        !body.contains("not yet implemented"),
        "release notes must not claim unimplemented status"
    );
}
