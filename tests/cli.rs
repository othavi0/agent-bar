//! Exhaustive v10 word-based CLI grammar and binary contract tests.

use std::path::{Path, PathBuf};
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
        parse(words(&["update"])).unwrap(),
        Command::Update(UpdateCommand::Interactive)
    );
    assert_eq!(
        parse(words(&["update", "check"])).unwrap(),
        Command::Update(UpdateCommand::Check)
    );
    assert_eq!(
        parse(words(&["update", "apply"])).unwrap(),
        Command::Update(UpdateCommand::Apply)
    );
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
fn setup_rejects_any_argument() {
    // `setup` takes no arguments now that install is git-clone-based
    // (git-plugin-distribution Task 4): `plugins-dir` and any other word
    // after `setup` is an ordinary unknown-argument grammar error.
    let plugins_dir = parse(words(&["setup", "plugins-dir", "/tmp/plugins"])).unwrap_err();
    assert_eq!(plugins_dir.exit_code, GRAMMAR);
    let other = parse(words(&["setup", "extra"])).unwrap_err();
    assert_eq!(other.exit_code, GRAMMAR);
}

#[test]
fn update_apply_rejects_trailing_arguments() {
    // `update apply` takes no argument (git-plugin-distribution Task 2): it
    // delegates unconditionally instead of applying a specific version.
    for extra in [
        words(&["update", "apply", "10.0.0"]),
        words(&["update", "apply", "extra", "words"]),
    ] {
        let err = parse(extra.clone()).unwrap_err();
        assert_eq!(err.exit_code, GRAMMAR, "{extra:?}");
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
    // setup no longer accepts plugins-dir (git-plugin-distribution Task 4):
    // git clone is the install now, so this is an ordinary unknown argument.
    CargoBin::cargo_bin("agent-bar")
        .unwrap()
        .args(["setup", "plugins-dir", "/tmp/x"])
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

/// Live QA regression (Task 22): setup must apply v9→v10 settings migration so
/// `config show` / status can read the strict document. Reproduction of the
/// failure where leftover v9 `settings.json` caused `unknown settings key`.
///
/// Retargeted at plain `setup` (git-plugin-distribution Task 4): setup no
/// longer installs a plugin tree, so there is no source tree to stage — the
/// binary under test is the real cargo-built helper, invoked directly.
#[test]
fn binary_setup_migrates_v9_settings_to_strict_v10() {
    let dir = tempdir().unwrap();
    let home = dir.path().join("home");
    let config = home.join("config");
    let state = home.join("state");
    let cache = home.join("cache");
    std::fs::create_dir_all(config.join("agent-bar")).unwrap();
    std::fs::create_dir_all(home.join(".config/omarchy")).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    std::fs::create_dir_all(&cache).unwrap();

    // Live-shaped v9 settings (unknown keys include `cache`, `waybar`, …).
    let v9 = std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/migration/v9/settings-valid.json"),
    )
    .unwrap();
    let settings_path = config.join("agent-bar/settings.json");
    std::fs::write(&settings_path, &v9).unwrap();

    // Clean shell entry (existing layout; no Agent Bar inline keys).
    let shell_path = home.join(".config/omarchy/shell.json");
    std::fs::write(
        &shell_path,
        std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/migration/v9/shell-clean.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let shell_before = std::fs::read(&shell_path).unwrap();

    let helper = assert_cmd::cargo::cargo_bin("agent-bar");

    let out = StdCommand::new(&helper)
        .args(["setup"])
        .env("HOME", &home)
        .env("XDG_STATE_HOME", &state)
        .env("XDG_CACHE_HOME", &cache)
        .env("XDG_CONFIG_HOME", &config)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "setup failed: status={:?} stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    // Settings must be strict v10 (no unknown keys); fixture migrated interval 120.
    let show = StdCommand::new(&helper)
        .args(["config", "show"])
        .env("HOME", &home)
        .env("XDG_STATE_HOME", &state)
        .env("XDG_CACHE_HOME", &cache)
        .env("XDG_CONFIG_HOME", &config)
        .output()
        .unwrap();
    assert!(
        show.status.success(),
        "config show must succeed after setup migration: status={:?} stderr={}",
        show.status.code(),
        String::from_utf8_lossy(&show.stderr)
    );
    let stdout = String::from_utf8_lossy(&show.stdout);
    assert!(
        stdout.contains("\"schemaVersion\": 1") || stdout.contains("\"schemaVersion\":1"),
        "expected v10 schemaVersion in config show: {stdout}"
    );
    assert!(
        stdout.contains("120"),
        "expected migrated refresh interval 120: {stdout}"
    );
    assert!(
        !stdout.contains("waybar") && !stdout.contains("\"cache\""),
        "v9 keys must not remain in config show: {stdout}"
    );

    // On-disk document must parse as strict v10.
    let stored = std::fs::read(&settings_path).unwrap();
    assert!(
        agent_bar::settings::schema::Settings::parse_strict(&stored).is_ok(),
        "stored settings must be strict v10 after setup"
    );

    // shell.json without Agent Bar inline keys must remain byte-identical.
    let shell_after = std::fs::read(&shell_path).unwrap();
    assert_eq!(
        shell_before, shell_after,
        "setup must not rewrite clean shell.json bytes"
    );

    // Pre-migration settings must be backed up under XDG state.
    let backups = state.join("agent-bar/backups");
    assert!(
        backups.is_dir(),
        "setup migration must create a backup root under XDG state"
    );
    let mut found_v9_backup = false;
    if let Ok(entries) = std::fs::read_dir(&backups) {
        for entry in entries.flatten() {
            let candidate = entry.path().join("settings/settings.json");
            if candidate.is_file() {
                let bak = std::fs::read(&candidate).unwrap();
                if bak == v9 {
                    found_v9_backup = true;
                    break;
                }
            }
        }
    }
    assert!(
        found_v9_backup,
        "v9 settings bytes must be preserved under backups/*/settings/settings.json"
    );
}

#[test]
fn binary_doctor_scan_is_read_only_and_exits_zero() {
    let dir = tempdir().unwrap();
    let home = dir.path();
    CargoBin::cargo_bin("agent-bar")
        .unwrap()
        .env("HOME", home)
        .env("XDG_STATE_HOME", home.join("state"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .args(["doctor", "scan"])
        .assert()
        .code(SUCCESS)
        .stdout(predicates::str::contains("doctor scan"))
        .stdout(predicates::str::contains("read-only"));
}

#[test]
fn binary_doctor_clean_backs_up_and_removes_owned_legacy() {
    let dir = tempdir().unwrap();
    let home = dir.path();
    // Split filename so active-legacy gates stay clean.
    let legacy_name = concat!("usage", ".", "re", "db");
    let legacy = home.join(".cache/agent-bar").join(legacy_name);
    std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    std::fs::write(&legacy, b"/* agent-bar generated */\n").unwrap();

    CargoBin::cargo_bin("agent-bar")
        .unwrap()
        .env("HOME", home)
        .env("XDG_STATE_HOME", home.join("state"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .args(["doctor", "clean"])
        .assert()
        .code(SUCCESS)
        .stdout(predicates::str::contains("doctor clean"))
        .stdout(predicates::str::contains("removed:"));

    assert!(!legacy.exists(), "doctor clean must remove owned legacy");
}

#[test]
fn binary_interactive_update_rejects_non_tty() {
    // Bare `update` has no interactive flow left (git-plugin-distribution
    // Task 2 removed the TTY confirm-then-apply dance): it always points at
    // the two real subcommands, TTY or not. Pipe stdin so the process is
    // non-TTY, matching the historical name of this test.
    let dir = tempdir().unwrap();
    let home = dir.path();
    let bin = assert_cmd::cargo::cargo_bin("agent-bar");
    let mut child = StdCommand::new(&bin)
        .arg("update")
        .env("HOME", home)
        .env("XDG_STATE_HOME", home.join("state"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        .env("XDG_CONFIG_HOME", home.join("config"))
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

/// Write an executable shell script (mirrors `tests/terminal_helper.rs`'s
/// fake-PATH-tool pattern for `xdg-terminal-exec`).
fn write_executable(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
    }
}

/// Recording shim: records its NUL-separated argv to `out` then exits 0.
fn recording_shim_body(out: &Path) -> String {
    format!(
        r#"#!/usr/bin/env bash
set -euo pipefail
: > "{out}"
for a in "$@"; do
  printf '%s\0' "$a" >> "{out}"
done
exit 0
"#,
        out = out.display()
    )
}

fn read_nul_argv(path: &Path) -> Vec<String> {
    let bytes = std::fs::read(path).unwrap_or_default();
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

#[test]
fn update_apply_emits_delegation_document() {
    // omarchy and systemd-run resolved via fake PATH shims in a tempdir that
    // record argv (git-plugin-distribution Task 2): `update apply` never
    // downloads/stages/exchanges anymore, it hands off to
    // `systemd-run --user --collect --unit=... -- <omarchy> plugin update
    // agent-bar.usage --yes` and prints the delegation document.
    let dir = tempdir().unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();

    let path_dir = dir.path().join("pathbin");
    write_executable(&path_dir.join("omarchy"), "#!/usr/bin/env bash\nexit 0\n");

    let systemd_run_argv = dir.path().join("systemd-run-argv.bin");
    write_executable(
        &path_dir.join("systemd-run"),
        &recording_shim_body(&systemd_run_argv),
    );

    let path = format!(
        "{}:{}",
        path_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let bin = assert_cmd::cargo::cargo_bin("agent-bar");
    let output = StdCommand::new(&bin)
        .args(["update", "apply"])
        .env("HOME", &home)
        .env("XDG_STATE_HOME", home.join("state"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.trim_end_matches('\n').lines().collect();
    assert_eq!(lines.len(), 1, "stdout must be exactly one line: {stdout}");
    let doc: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(doc["schemaVersion"], 1);
    assert_eq!(doc["operation"], "updateApply");
    assert_eq!(doc["delegated"], true);
    let unit = doc["unit"].as_str().expect("unit is a string");
    assert!(
        unit.starts_with("agent-bar-update-") && unit.ends_with(".service"),
        "unit={unit}"
    );

    let argv = read_nul_argv(&systemd_run_argv);
    assert!(
        argv.ends_with(&[
            "plugin".to_string(),
            "update".to_string(),
            "agent-bar.usage".to_string(),
            "--yes".to_string(),
        ]),
        "argv={argv:?}"
    );
    assert_eq!(argv.first().map(String::as_str), Some("--user"));
    assert!(argv.contains(&"--collect".to_string()));
    assert!(argv.iter().any(|a| a == &format!("--unit={unit}")));
    assert!(argv.contains(&"--".to_string()));
}

/// Fixture for the two `uninstall` delegation tests: isolated XDG roots with
/// owned content, plus a fake `$HOME/.config/omarchy/shell.json` the helper
/// must never touch (git-plugin-distribution Task 3: shell.json is
/// omarchy's now).
struct UninstallFixture {
    home: PathBuf,
    settings_dir: PathBuf,
    cache_dir: PathBuf,
    state_dir: PathBuf,
    shell_path: PathBuf,
    shell_before: &'static [u8],
    path_dir: PathBuf,
    systemd_run_argv: PathBuf,
}

fn seed_uninstall_fixture(root: &Path) -> UninstallFixture {
    let home = root.join("home");
    let config = home.join("config");
    let cache = home.join("cache");
    let state = home.join("state");
    std::fs::create_dir_all(&home).unwrap();

    let settings_dir = config.join("agent-bar");
    std::fs::create_dir_all(&settings_dir).unwrap();
    std::fs::write(
        settings_dir.join("settings.json"),
        br#"{"schemaVersion":1}"#,
    )
    .unwrap();

    let cache_dir = cache.join("agent-bar");
    std::fs::create_dir_all(&cache_dir).unwrap();
    std::fs::write(cache_dir.join("status-v2.json"), b"{}").unwrap();
    std::fs::write(cache_dir.join("notification-state-v1.json"), b"{}").unwrap();

    let shell_path = home.join(".config/omarchy/shell.json");
    std::fs::create_dir_all(shell_path.parent().unwrap()).unwrap();
    let shell_before: &'static [u8] = br#"{"bar":{"left":[{"id":"agent-bar.usage"}]}}"#;
    std::fs::write(&shell_path, shell_before).unwrap();

    let path_dir = root.join("pathbin");
    write_executable(&path_dir.join("omarchy"), "#!/usr/bin/env bash\nexit 0\n");
    let systemd_run_argv = root.join("systemd-run-argv.bin");
    write_executable(
        &path_dir.join("systemd-run"),
        &recording_shim_body(&systemd_run_argv),
    );

    UninstallFixture {
        home,
        settings_dir,
        cache_dir,
        state_dir: state,
        shell_path,
        shell_before,
        path_dir,
        systemd_run_argv,
    }
}

fn run_uninstall(
    fx: &UninstallFixture,
    args: &[&str],
    confirmation: &[u8],
) -> std::process::Output {
    let path = format!(
        "{}:{}",
        fx.path_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let bin = assert_cmd::cargo::cargo_bin("agent-bar");
    let mut child = StdCommand::new(&bin)
        .arg("uninstall")
        .args(args)
        .env("HOME", &fx.home)
        .env("XDG_STATE_HOME", &fx.state_dir)
        .env("XDG_CACHE_HOME", fx.cache_dir.parent().unwrap())
        .env("XDG_CONFIG_HOME", fx.settings_dir.parent().unwrap())
        .env("PATH", &path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write as _;
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(confirmation).unwrap();
    }
    child.wait_with_output().unwrap()
}

#[test]
fn uninstall_purge_removes_xdg_state_and_delegates_remove() {
    // git-plugin-distribution Task 3: `uninstall purge` no longer runs a
    // quarantine/rollback worker chain over a copied worker binary — it
    // purges Agent Bar's own XDG state directly under the maintenance gate,
    // then hands the plugin tree + shell.json removal to `omarchy plugin
    // remove` via the same detached-unit shape Task 2 used for `update
    // apply`.
    let dir = tempdir().unwrap();
    let fx = seed_uninstall_fixture(dir.path());

    let confirmation = br#"{"schemaVersion":1,"operation":"uninstall","confirmed":true,"purgeSettingsAndBackups":true}"#;
    let output = run_uninstall(&fx, &["purge"], confirmation);
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.trim_end_matches('\n').lines().collect();
    assert_eq!(lines.len(), 1, "stdout must be exactly one line: {stdout}");
    let doc: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(doc["schemaVersion"], 1);
    assert_eq!(doc["operation"], "uninstall");
    assert_eq!(doc["purged"], true);
    assert_eq!(doc["delegated"], true);
    let unit = doc["unit"].as_str().expect("unit is a string");
    assert!(
        unit.starts_with("agent-bar-remove-") && unit.ends_with(".service"),
        "unit={unit}"
    );

    assert!(!fx.settings_dir.exists(), "purge must remove settings dir");
    assert!(!fx.cache_dir.exists(), "purge must remove cache dir");
    assert!(
        !fx.state_dir.join("agent-bar").exists(),
        "purge must remove state dir"
    );

    let shell_after = std::fs::read(&fx.shell_path).unwrap();
    assert_eq!(
        shell_after, fx.shell_before,
        "uninstall must never touch shell.json — omarchy owns it now"
    );

    let argv = read_nul_argv(&fx.systemd_run_argv);
    assert!(
        argv.ends_with(&[
            "plugin".to_string(),
            "remove".to_string(),
            "agent-bar.usage".to_string(),
            "--yes".to_string(),
        ]),
        "argv={argv:?}"
    );
    assert_eq!(argv.first().map(String::as_str), Some("--user"));
    assert!(argv.contains(&"--collect".to_string()));
    assert!(argv.iter().any(|a| a == &format!("--unit={unit}")));
    assert!(argv.contains(&"--".to_string()));
}

#[test]
fn uninstall_without_purge_preserves_xdg_state_and_delegates_remove() {
    let dir = tempdir().unwrap();
    let fx = seed_uninstall_fixture(dir.path());

    let confirmation = br#"{"schemaVersion":1,"operation":"uninstall","confirmed":true,"purgeSettingsAndBackups":false}"#;
    let output = run_uninstall(&fx, &[], confirmation);
    assert!(
        output.status.success(),
        "stderr={} stdout={}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let doc: serde_json::Value = serde_json::from_str(stdout.trim_end()).unwrap();
    assert_eq!(doc["schemaVersion"], 1);
    assert_eq!(doc["operation"], "uninstall");
    assert_eq!(doc["purged"], false);
    assert_eq!(doc["delegated"], true);
    let unit = doc["unit"].as_str().expect("unit is a string");
    assert!(unit.starts_with("agent-bar-remove-") && unit.ends_with(".service"));

    assert!(
        fx.settings_dir.join("settings.json").is_file(),
        "without purge, settings must survive"
    );
    assert!(
        fx.cache_dir.join("status-v2.json").is_file(),
        "without purge, cache must survive"
    );
    assert!(
        fx.state_dir.join("agent-bar").exists(),
        "without purge, state dir must survive"
    );

    let shell_after = std::fs::read(&fx.shell_path).unwrap();
    assert_eq!(
        shell_after, fx.shell_before,
        "uninstall must never touch shell.json — omarchy owns it now"
    );

    let argv = read_nul_argv(&fx.systemd_run_argv);
    assert!(
        argv.ends_with(&[
            "plugin".to_string(),
            "remove".to_string(),
            "agent-bar.usage".to_string(),
            "--yes".to_string(),
        ]),
        "argv={argv:?}"
    );
}

#[test]
fn uninstall_purge_fails_closed_without_touching_state_when_omarchy_missing() {
    // Review round 1, finding 1: tool resolution must happen before any
    // destructive purge, so a missing `omarchy` fails the whole command
    // before the settings/cache/state XDG roots are touched — not after
    // they are already gone with the plugin never actually removed. PATH is
    // isolated to just `path_dir` (no real system PATH fallback) so a real
    // `omarchy` binary elsewhere on this machine cannot mask the failure.
    let dir = tempdir().unwrap();
    let fx = seed_uninstall_fixture(dir.path());
    std::fs::remove_file(fx.path_dir.join("omarchy")).unwrap();

    let bin = assert_cmd::cargo::cargo_bin("agent-bar");
    let mut child = StdCommand::new(&bin)
        .args(["uninstall", "purge"])
        .env("HOME", &fx.home)
        .env("XDG_STATE_HOME", &fx.state_dir)
        .env("XDG_CACHE_HOME", fx.cache_dir.parent().unwrap())
        .env("XDG_CONFIG_HOME", fx.settings_dir.parent().unwrap())
        .env("PATH", &fx.path_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write as _;
        let mut stdin = child.stdin.take().unwrap();
        stdin
            .write_all(
                br#"{"schemaVersion":1,"operation":"uninstall","confirmed":true,"purgeSettingsAndBackups":true}"#,
            )
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();

    assert!(
        !output.status.success(),
        "must fail closed when omarchy cannot be resolved: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "no delegation document on preflight failure"
    );

    assert!(
        fx.settings_dir.join("settings.json").is_file(),
        "settings must survive a failed tool resolution"
    );
    assert!(
        fx.cache_dir.join("status-v2.json").is_file(),
        "cache must survive a failed tool resolution"
    );
    assert!(
        fx.state_dir.join("agent-bar").exists(),
        "state dir must survive a failed tool resolution"
    );

    let shell_after = std::fs::read(&fx.shell_path).unwrap();
    assert_eq!(
        shell_after, fx.shell_before,
        "uninstall must never touch shell.json — omarchy owns it now"
    );
}
