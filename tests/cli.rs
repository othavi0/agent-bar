use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn prints_version() {
    Command::cargo_bin("agent-bar")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(contains(env!("CARGO_PKG_VERSION")));
}
