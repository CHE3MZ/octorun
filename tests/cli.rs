use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_lists_all_subcommands() {
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("remove"))
        .stdout(predicate::str::contains("start"))
        .stdout(predicate::str::contains("stop"));
}

#[test]
fn version_is_reported() {
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn missing_subcommand_fails() {
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.assert().failure();
}
