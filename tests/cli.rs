use assert_cmd::Command;
use predicates::str::{contains, diff};
use tempfile::tempdir;

#[test]
fn prints_version() {
    Command::cargo_bin("agent-bar")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(contains(env!("CARGO_PKG_VERSION")));
}

// ---------------------------------------------------------------------------
// menu-font — contrato do helper (scripts/agent-bar-open-terminal)
// ---------------------------------------------------------------------------

#[test]
fn menu_font_prints_family_and_size_from_settings() {
    let dir = tempdir().unwrap();
    let config_home = dir.path().join("config");
    let agent_bar_dir = config_home.join("agent-bar");
    std::fs::create_dir_all(&agent_bar_dir).unwrap();
    std::fs::write(
        agent_bar_dir.join("settings.json"),
        r#"{"menu":{"fontFamily":"Geist Mono","fontSize":13}}"#,
    )
    .unwrap();

    Command::cargo_bin("agent-bar")
        .unwrap()
        .arg("menu-font")
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_CACHE_HOME", dir.path().join("cache"))
        .assert()
        .success()
        .stdout(diff("Geist Mono\t13\n"));
}

#[test]
fn menu_font_prints_default_family_and_size() {
    let dir = tempdir().unwrap();
    let config_home = dir.path().join("config");

    Command::cargo_bin("agent-bar")
        .unwrap()
        .arg("menu-font")
        .env("HOME", dir.path())
        .env("XDG_CONFIG_HOME", &config_home)
        .env("XDG_CACHE_HOME", dir.path().join("cache"))
        .assert()
        .success()
        .stdout(diff("IBM Plex Mono\t12\n"));
}
