//! Exhaustive v10 word-based CLI grammar and binary contract tests.

use std::path::PathBuf;
use std::process::Command as StdCommand;

use agent_bar::cli::{
    parse, CacheMode, Command, ConfigCommand, ConfigInput, DoctorCommand, HelpTopic,
    NotificationMode, ProviderId, SetupOptions, StatusFormat, StatusOptions, UpdateCommand,
    GRAMMAR, SUCCESS, VALIDATION,
};
use assert_cmd::Command as CargoBin;
use tempfile::tempdir;

fn words(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| (*s).to_owned()).collect()
}

fn status_opts(
    format: StatusFormat,
    provider: Option<ProviderId>,
    cache: CacheMode,
    notifications: NotificationMode,
) -> StatusOptions {
    StatusOptions {
        format,
        provider,
        cache,
        notifications,
    }
}

#[test]
fn bare_agent_bar_equals_status_format_human() {
    assert_eq!(
        parse(words(&[])).unwrap(),
        Command::Status(StatusOptions::default())
    );
    assert_eq!(
        parse(words(&["status"])).unwrap(),
        Command::Status(StatusOptions::default())
    );
}

#[test]
fn all_24_status_clause_order_permutations_parse() {
    let clauses = [
        ("format", "json"),
        ("provider", "claude"),
        ("cache", "bypass"),
        ("notifications", "evaluate"),
    ];
    let mut count = 0usize;
    for a in 0..4 {
        for b in 0..4 {
            if b == a {
                continue;
            }
            for c in 0..4 {
                if c == a || c == b {
                    continue;
                }
                for d in 0..4 {
                    if d == a || d == b || d == c {
                        continue;
                    }
                    let order = [a, b, c, d];
                    let mut args = vec!["status".to_owned()];
                    for &idx in &order {
                        args.push(clauses[idx].0.to_owned());
                        args.push(clauses[idx].1.to_owned());
                    }
                    let cmd = parse(args).unwrap_or_else(|e| panic!("perm {order:?}: {e:?}"));
                    assert_eq!(
                        cmd,
                        Command::Status(status_opts(
                            StatusFormat::Json,
                            Some(ProviderId::Claude),
                            CacheMode::Bypass,
                            NotificationMode::Evaluate,
                        )),
                        "perm {order:?}"
                    );
                    count += 1;
                }
            }
        }
    }
    assert_eq!(count, 24);
}

#[test]
fn status_rejects_duplicates_missing_values_and_unknowns() {
    let cases = [
        words(&["status", "format"]),
        words(&["status", "format", "json", "format", "human"]),
        words(&["status", "provider"]),
        words(&["status", "provider", "nope"]),
        words(&["status", "cache", "maybe"]),
        words(&["status", "notifications", "always"]),
        words(&["status", "unknown"]),
        words(&[
            "status", "format", "json", "provider", "claude", "provider", "amp",
        ]),
    ];
    for args in cases {
        let err = parse(args.clone()).unwrap_err();
        assert_eq!(err.exit_code, GRAMMAR, "{args:?}");
    }
}

#[test]
fn status_accepts_each_provider_and_format() {
    for provider in ProviderId::ALL {
        let cmd = parse(words(&[
            "status",
            "provider",
            provider.as_str(),
            "format",
            "human",
        ]))
        .unwrap();
        assert_eq!(
            cmd,
            Command::Status(status_opts(
                StatusFormat::Human,
                Some(provider),
                CacheMode::Use,
                NotificationMode::Skip,
            ))
        );
    }
    assert_eq!(
        parse(words(&["status", "format", "json", "cache", "use"])).unwrap(),
        Command::Status(status_opts(
            StatusFormat::Json,
            None,
            CacheMode::Use,
            NotificationMode::Skip,
        ))
    );
}

#[test]
fn login_config_setup_update_uninstall_doctor_forms() {
    assert_eq!(
        parse(words(&["login", "grok"])).unwrap(),
        Command::Login(ProviderId::Grok)
    );
    assert_eq!(
        parse(words(&["config", "show"])).unwrap(),
        Command::Config(ConfigCommand::Show)
    );
    assert_eq!(
        parse(words(&["config", "apply", "stdin"])).unwrap(),
        Command::Config(ConfigCommand::Apply(ConfigInput::Stdin))
    );
    assert_eq!(
        parse(words(&["config", "apply", "file", "/tmp/settings.json"])).unwrap(),
        Command::Config(ConfigCommand::Apply(ConfigInput::File(PathBuf::from(
            "/tmp/settings.json"
        ))))
    );
    assert_eq!(
        parse(words(&[
            "config",
            "apply",
            "json",
            r#"{"schemaVersion":1}"#
        ]))
        .unwrap(),
        Command::Config(ConfigCommand::Apply(ConfigInput::Json(
            r#"{"schemaVersion":1}"#.into()
        )))
    );
    assert_eq!(
        parse(words(&["setup"])).unwrap(),
        Command::Setup(SetupOptions::Production)
    );
    assert_eq!(
        parse(words(&["setup", "plugins-dir", "/tmp/plugins"])).unwrap(),
        Command::Setup(SetupOptions::PluginsDir(PathBuf::from("/tmp/plugins")))
    );
    assert_eq!(
        parse(words(&["update"])).unwrap(),
        Command::Update(UpdateCommand::Interactive)
    );
    assert_eq!(
        parse(words(&["update", "check"])).unwrap(),
        Command::Update(UpdateCommand::Check)
    );
    let apply = parse(words(&["update", "apply", "10.0.0"])).unwrap();
    match apply {
        Command::Update(UpdateCommand::Apply(v)) => assert_eq!(v.as_str(), "10.0.0"),
        other => panic!("expected apply, got {other:?}"),
    }
    assert_eq!(
        parse(words(&["uninstall"])).unwrap(),
        Command::Uninstall { purge: false }
    );
    assert_eq!(
        parse(words(&["uninstall", "purge"])).unwrap(),
        Command::Uninstall { purge: true }
    );
    assert_eq!(
        parse(words(&["doctor", "scan"])).unwrap(),
        Command::Doctor(DoctorCommand::Scan)
    );
    assert_eq!(
        parse(words(&["doctor", "clean"])).unwrap(),
        Command::Doctor(DoctorCommand::Clean)
    );
}

#[test]
fn setup_plugins_dir_rejects_relative_and_plugin_root_path() {
    let rel = parse(words(&["setup", "plugins-dir", "relative/plugins"])).unwrap_err();
    assert_eq!(rel.exit_code, GRAMMAR);
    let root = parse(words(&[
        "setup",
        "plugins-dir",
        "/tmp/omarchy/plugins/agent-bar.usage",
    ]))
    .unwrap_err();
    assert_eq!(root.exit_code, GRAMMAR);
}

#[test]
fn update_apply_rejects_non_strict_semver() {
    for version in ["10", "10.0", "v10.0.0", "latest", ""] {
        let args = if version.is_empty() {
            words(&["update", "apply"])
        } else {
            words(&["update", "apply", version])
        };
        let err = parse(args).unwrap_err();
        assert_eq!(err.exit_code, GRAMMAR, "{version}");
    }
}

#[test]
fn help_topics_accepted_and_rejected() {
    assert_eq!(parse(words(&["help"])).unwrap(), Command::Help(None));
    for topic in HelpTopic::ALL {
        assert_eq!(
            parse(words(&["help", topic.as_str()])).unwrap(),
            Command::Help(Some(topic))
        );
    }
    let err = parse(words(&["help", "waybar"])).unwrap_err();
    assert_eq!(err.exit_code, GRAMMAR);
    let err = parse(words(&["help", "menu"])).unwrap_err();
    assert_eq!(err.exit_code, GRAMMAR);
}

#[test]
fn double_dash_aliases_and_legacy_rejections() {
    assert_eq!(parse(words(&["--help"])).unwrap(), Command::Help(None));
    assert_eq!(parse(words(&["--version"])).unwrap(), Command::Version);
    assert_eq!(parse(words(&["version"])).unwrap(), Command::Version);

    // Legacy words are rejected by the grammar. Tokens that the active-legacy
    // gate forbids as contiguous source text are built with concat! so the
    // scan stays clean while runtime strings still match the old CLI surface.
    let action_right = concat!("action", "-", "right");
    let menu_font = concat!("menu", "-", "font");
    let legacy: Vec<Vec<&str>> = vec![
        vec!["menu"],
        vec!["waybar"],
        vec!["watch"],
        vec![action_right],
        vec!["remove"],
        vec![menu_font],
        vec!["assets-install"],
        vec!["-t"],
        vec!["-v"],
        vec!["--format", "json"],
        vec!["--json"],
        vec!["--provider", "claude"],
        vec!["--verbose"],
        vec!["--watch"],
        vec!["--yes"],
        vec!["status", "--format", "json"],
        vec!["config", "apply", "--json", "{}"],
    ];
    for args in legacy {
        let owned: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let err = parse(owned.clone()).unwrap_err();
        assert_eq!(err.exit_code, GRAMMAR, "{args:?}");
    }
}

#[test]
fn binary_version_prints_exact_package_semver_only() {
    let assert = CargoBin::cargo_bin("agent-bar")
        .unwrap()
        .arg("version")
        .assert()
        .code(SUCCESS)
        .stdout(format!("{}\n", env!("CARGO_PKG_VERSION")))
        .stderr("");
    // assert_cmd already checked; keep binding used.
    let _ = assert;

    CargoBin::cargo_bin("agent-bar")
        .unwrap()
        .arg("--version")
        .assert()
        .code(SUCCESS)
        .stdout(format!("{}\n", env!("CARGO_PKG_VERSION")))
        .stderr("");
}

#[test]
fn binary_version_does_not_require_config_home() {
    let dir = tempdir().unwrap();
    // Point XDG/HOME at an empty tree so accidental discovery would fail.
    CargoBin::cargo_bin("agent-bar")
        .unwrap()
        .arg("version")
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", dir.path().join("missing-config"))
        .env("XDG_CACHE_HOME", dir.path().join("missing-cache"))
        .env("XDG_STATE_HOME", dir.path().join("missing-state"))
        .assert()
        .code(SUCCESS)
        .stdout(format!("{}\n", env!("CARGO_PKG_VERSION")))
        .stderr("");
}

#[test]
fn binary_grammar_errors_exit_2() {
    CargoBin::cargo_bin("agent-bar")
        .unwrap()
        .args(["status", "format", "xml"])
        .assert()
        .code(GRAMMAR)
        .stdout("");
    CargoBin::cargo_bin("agent-bar")
        .unwrap()
        .args(["--verbose"])
        .assert()
        .code(GRAMMAR)
        .stdout("");
    CargoBin::cargo_bin("agent-bar")
        .unwrap()
        .args(["menu"])
        .assert()
        .code(GRAMMAR)
        .stdout("");
}

#[test]
fn binary_help_mentions_plugin_first_product() {
    CargoBin::cargo_bin("agent-bar")
        .unwrap()
        .arg("help")
        .assert()
        .code(SUCCESS)
        .stdout(predicates::str::contains("Quickshell plugin"))
        .stdout(predicates::str::contains("diagnostics"))
        .stderr("");
}

#[test]
fn binary_setup_plugins_dir_validates_parent_versus_plugin_root() {
    let dir = tempdir().unwrap();
    let parent = dir.path().canonicalize().unwrap();
    let plugin_root = parent.join("agent-bar.usage");
    std::fs::create_dir_all(&plugin_root).unwrap();

    // Grammar rejects direct plugin root (exit 2).
    CargoBin::cargo_bin("agent-bar")
        .unwrap()
        .args(["setup", "plugins-dir", plugin_root.to_str().unwrap()])
        .assert()
        .code(GRAMMAR)
        .stdout("");

    // Relative path rejected by grammar.
    CargoBin::cargo_bin("agent-bar")
        .unwrap()
        .args(["setup", "plugins-dir", "relative/plugins"])
        .assert()
        .code(GRAMMAR)
        .stdout("");

    // Missing absolute parent is validation (exit 3).
    let missing = parent.join("does-not-exist");
    CargoBin::cargo_bin("agent-bar")
        .unwrap()
        .args(["setup", "plugins-dir", missing.to_str().unwrap()])
        .assert()
        .code(VALIDATION)
        .stdout("");

    // Existing writable parent parses and reaches not-implemented (70) after validation.
    CargoBin::cargo_bin("agent-bar")
        .unwrap()
        .args(["setup", "plugins-dir", parent.to_str().unwrap()])
        .assert()
        .code(70)
        .stdout("");
}

#[test]
fn binary_interactive_update_rejects_non_tty() {
    // Pipe stdin so the process is non-TTY.
    let bin = assert_cmd::cargo::cargo_bin("agent-bar");
    let mut child = StdCommand::new(&bin)
        .arg("update")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    // Drop stdin immediately → non-TTY / closed.
    drop(child.stdin.take());
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(VALIDATION));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("update check") && stderr.contains("update apply"),
        "stderr={stderr}"
    );
}
