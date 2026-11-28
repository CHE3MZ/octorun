use std::fs;

use assert_cmd::Command;
use flate2::Compression;
use flate2::write::GzEncoder;
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
        .stdout(predicate::str::contains("restart"))
        .stdout(predicate::str::contains("temp"));
}

#[test]
fn help_shows_about_text() {
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Create self hosted GitHub Actions runners",
        ));
}

#[test]
fn help_hides_name_default() {
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("[default: actions-runner]").not());
}

#[test]
fn help_lists_dir_before_repo() {
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    let output = cmd.arg("help").output().unwrap();
    let text = String::from_utf8(output.stdout).unwrap();

    let dir = text.find("--dir").expect("--dir should be in help");
    let repo = text.find("--repo").expect("--repo should be in help");
    assert!(dir < repo, "--dir should appear before --repo");
}

#[test]
fn help_uses_styled_colors_when_forced() {
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    let output = cmd
        .env("CLICOLOR_FORCE", "1")
        .env("COLORTERM", "truecolor")
        .env_remove("NO_COLOR")
        .arg("help")
        .output()
        .unwrap();
    let text = String::from_utf8(output.stdout).unwrap();

    assert!(text.contains("38;2;41;184;219"), "accent blue is missing");
    assert!(text.contains("38;2;35;209;139"), "heading green is missing");
    assert!(text.contains("\u{1b}[1m"), "bold escape is missing");
}

#[test]
fn help_respects_no_color() {
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    let output = cmd
        .env("NO_COLOR", "1")
        .env_remove("CLICOLOR_FORCE")
        .env_remove("FORCE_COLOR")
        .arg("help")
        .output()
        .unwrap();
    let text = String::from_utf8(output.stdout).unwrap();

    assert!(!text.contains('\u{1b}'), "unexpected ansi escape in help");
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
fn create_rejects_invalid_platform() {
    let dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.args(["-r", "acme/app", "-d"])
        .arg(dir.path())
        .args(["create", "-p", "plan9-amd64"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("plan9-amd64"));
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

fn runner_archive() -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    {
        let mut builder = tar::Builder::new(&mut encoder);
        for (name, body) in [("runner", "binary"), ("config.sh", "#!/bin/sh\nexit 0\n")] {
            let data = body.as_bytes();
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder.append_data(&mut header, name, data).unwrap();
        }
        builder.finish().unwrap();
    }
    encoder.finish().unwrap()
}

#[test]
fn create_no_configuration_extracts_without_registering() {
    let dir = tempdir().unwrap();
    let archive = dir.path().join("runner.tar.gz");
    fs::write(&archive, runner_archive()).unwrap();

    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.args([
        "-d",
        dir.path().to_str().unwrap(),
        "-n",
        "demo",
        "create",
        "--no-configuration",
        "-l",
        archive.to_str().unwrap(),
        "-p",
        "linux-amd64",
    ])
    .assert()
    .success();

    let runner = dir.path().join("demo");
    assert_eq!(
        fs::read_to_string(runner.join(".gitignore")).unwrap(),
        "*\n"
    );
    assert_eq!(fs::read_to_string(runner.join("runner")).unwrap(), "binary");
    assert!(!runner.join(".runner").exists());
}

#[test]
fn temp_rejects_no_configuration() {
    let dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.args([
        "-d",
        dir.path().to_str().unwrap(),
        "temp",
        "--no-configuration",
    ])
    .assert()
    .failure()
    .stderr(predicate::str::contains("must be registered"));
}

#[test]
fn remove_deletes_a_runner_without_a_config_script() {
    let dir = tempdir().unwrap();
    let runner = dir.path().join("runner");
    fs::create_dir_all(&runner).unwrap();

    let mut cmd = Command::cargo_bin("octorun").unwrap();
    cmd.args(["-d", dir.path().to_str().unwrap(), "-n", "runner", "remove"])
        .assert()
        .success();

    assert!(!runner.exists());
}
