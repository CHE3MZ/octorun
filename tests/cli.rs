use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn help_lists_all_subcommands() {
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("remove"))
        .stdout(predicate::str::contains("start"))
        .stdout(predicate::str::contains("stop"))
        .stdout(predicate::str::contains("temp"));
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

#[test]
fn create_rejects_conflicting_scopes() {
    let dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.args(["-r", "acme/app", "-d"])
        .arg(dir.path())
        .args(["create", "--org", "acme", "--enterprise", "acme-inc"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used together"));
}

#[test]
fn create_rejects_unsafe_runner_name() {
    let dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.args(["-r", "acme/app", "-d"])
        .arg(dir.path())
        .args(["-n", "..", "create"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid runner name"));
}

#[test]
fn create_rejects_invalid_repository() {
    let dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.args(["-r", "not-a-repo", "-d"])
        .arg(dir.path())
        .arg("create")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid repository"));
}

#[test]
fn create_refuses_existing_directory_without_overwrite() {
    let dir = tempdir().unwrap();
    fs::create_dir(dir.path().join("actions-runner")).unwrap();

    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.args(["-r", "acme/app", "-d"])
        .arg(dir.path())
        .arg("create")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn start_requires_configured_runner() {
    let dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.args(["-d"])
        .arg(dir.path())
        .arg("start")
        .assert()
        .failure()
        .stderr(predicate::str::contains("not configured"));
}

#[test]
fn stop_requires_recorded_process() {
    let dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.args(["-d"])
        .arg(dir.path())
        .arg("stop")
        .assert()
        .failure()
        .stderr(predicate::str::contains("no runner process is recorded"));
}

#[test]
fn remove_requires_existing_runner() {
    let dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.args(["-d"])
        .arg(dir.path())
        .arg("remove")
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));
}

#[test]
fn temp_rejects_invalid_repository() {
    let dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.args(["-r", "not-a-repo", "-d"])
        .arg(dir.path())
        .arg("temp")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid repository"));
}

#[test]
fn temp_rejects_unsafe_runner_name() {
    let dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.args(["-r", "acme/app", "-d"])
        .arg(dir.path())
        .args(["-n", "..", "temp", "--ephemeral"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid runner name"));
}
