//! `octorun create` — create a new runner.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use super::cleanup::{DirGuard, cancel_flag};
use super::{runner_dir, runner_url, scope_for, validate_name};
use crate::cli::{Cli, CreateArgs};
use crate::error::{Error, Result};
use crate::github::{Client, Repository, Scope, TokenKind, resolve_tokens_with};
use crate::runner::RunnerPlatform;
use crate::runner::config::{self, Options};
use crate::runner::install;

/// Details about a runner that was just created.
#[derive(Debug)]
pub(crate) struct Context {
    pub runner_dir: PathBuf,
    pub host: String,
    pub scope: Scope,
}

/// Create a runner from the given arguments.
///
/// With `--no-configuration`, download and extract the runner and write the
/// `.gitignore`, then skip registration and configuration.
///
/// # Errors
///
/// Returns an error if the repository, target, or token cannot be resolved, the
/// runner files cannot be installed, or the configuration script fails.
pub fn run(cli: &Cli, args: &CreateArgs) -> Result<()> {
    let cancelled = cancel_flag()?;
    let mut guard = DirGuard::new(runner_dir(cli));

    if args.no_configuration {
        let runner_dir = prepare_dir(cli, args, Some(&cancelled))?;
        let platform = resolve_platform(args)?;
        let api = Client::anonymous("github.com")?;
        install_runner(&api, args, &runner_dir, platform, Some(&cancelled))?;

        guard.disarm();
        tracing::info!(
            "downloaded runner `{}` to {} without configuring it",
            cli.name,
            runner_dir.display()
        );
        return Ok(());
    }

    let context = prepare(cli, args, false, Some(&cancelled), false)?;
    guard.disarm();

    tracing::info!(
        "runner `{}` is ready at {}",
        cli.name,
        context.runner_dir.display()
    );
    Ok(())
}

/// Install and configure a runner, returning the details needed to manage it.
///
/// `cancel` lets a caller stop long downloads, and `cleanup_on_error` removes
/// the runner directory and registration when setup fails after registration.
///
/// # Errors
///
/// Returns [`Error::Cancelled`] when cancellation is requested, or an error if
/// the repository, target, or token cannot be resolved, the runner files cannot
/// be installed, or the configuration script fails.
pub(crate) fn prepare(
    cli: &Cli,
    args: &CreateArgs,
    ephemeral: bool,
    cancel: Option<&AtomicBool>,
    cleanup_on_error: bool,
) -> Result<Context> {
    prepare_with(
        cli,
        args,
        ephemeral,
        cancel,
        cleanup_on_error,
        |cli, host| {
            let tokens = resolve_tokens_with(cli.token.as_deref(), host)?;
            Client::with_tokens(host, tokens)
        },
    )
}

pub(crate) fn prepare_with(
    cli: &Cli,
    args: &CreateArgs,
    ephemeral: bool,
    cancel: Option<&AtomicBool>,
    cleanup_on_error: bool,
    client_for: impl FnOnce(&Cli, &str) -> Result<Client>,
) -> Result<Context> {
    let runner_dir = prepare_dir(cli, args, cancel)?;

    let repository = match &cli.repo {
        Some(reference) => Repository::parse(reference)?,
        None => Repository::current()?,
    };
    let host = repository.host();

    let scope = scope_for(&repository, args.org.as_deref(), args.enterprise.as_deref())?;
    let url = runner_url(host, &scope);
    let platform = resolve_platform(args)?;

    check_cancel(cancel)?;
    let api = client_for(cli, host)?;

    prepare_runner(
        &api,
        cli,
        args,
        runner_dir,
        host,
        scope,
        url,
        platform,
        ephemeral,
        cleanup_on_error,
        cancel,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_runner(
    api: &Client,
    cli: &Cli,
    args: &CreateArgs,
    runner_dir: PathBuf,
    host: &str,
    scope: Scope,
    url: String,
    platform: RunnerPlatform,
    ephemeral: bool,
    cleanup_on_error: bool,
    cancel: Option<&AtomicBool>,
) -> Result<Context> {
    install_runner(api, args, &runner_dir, platform, cancel)?;

    check_cancel(cancel)?;
    let registration_token = api.runner_token(&scope, TokenKind::Registration)?;
    check_cancel(cancel)?;

    let options = Options {
        url,
        token: registration_token,
        name: cli.name.clone(),
        replace: args.overwrite,
        labels: args.labels.clone(),
        ephemeral,
    };

    if let Err(error) = config::configure(&runner_dir, &options) {
        if cleanup_on_error || cancel.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
            cleanup_partial(&runner_dir, host, &scope, cli.token.as_deref());
        }
        return Err(error);
    }

    if let Err(error) = check_cancel(cancel) {
        cleanup_partial(&runner_dir, host, &scope, cli.token.as_deref());
        return Err(error);
    }

    Ok(Context {
        runner_dir,
        host: host.to_owned(),
        scope,
    })
}

fn prepare_dir(cli: &Cli, args: &CreateArgs, cancel: Option<&AtomicBool>) -> Result<PathBuf> {
    validate_name(&cli.name)?;
    check_cancel(cancel)?;

    let runner_dir = runner_dir(cli);
    if let Some(parent) = runner_dir.parent() {
        fs::create_dir_all(parent)?;
    }

    if runner_dir.exists() {
        if !args.overwrite {
            return Err(Error::Config(format!(
                "runner directory `{}` already exists; use --overwrite to replace it",
                runner_dir.display()
            )));
        }
        fs::remove_dir_all(&runner_dir)?;
        tracing::info!("removed existing runner directory {}", runner_dir.display());
    }

    Ok(runner_dir)
}

fn resolve_platform(args: &CreateArgs) -> Result<RunnerPlatform> {
    match args.platform {
        Some(platform) => Ok(platform),
        None => RunnerPlatform::detect(),
    }
}

fn install_runner(
    api: &Client,
    args: &CreateArgs,
    runner_dir: &Path,
    platform: RunnerPlatform,
    cancel: Option<&AtomicBool>,
) -> Result<()> {
    fs::create_dir_all(runner_dir)?;
    write_gitignore(runner_dir)?;
    install::install(api, args.location.as_deref(), runner_dir, platform, cancel)
}

fn check_cancel(cancel: Option<&AtomicBool>) -> Result<()> {
    if cancel.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
        Err(Error::Cancelled)
    } else {
        Ok(())
    }
}

fn write_gitignore(dir: &Path) -> Result<()> {
    fs::write(dir.join(".gitignore"), "*\n")?;
    Ok(())
}

fn cleanup_partial(dir: &Path, host: &str, scope: &Scope, token: Option<&str>) {
    if dir.join(".runner").exists()
        && let Err(error) = super::deregister(dir, host, scope, token)
    {
        tracing::warn!("could not deregister runner: {error}");
    }

    if let Err(error) = fs::remove_dir_all(dir)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(
            "could not remove runner directory {}: {error}",
            dir.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::{Path, PathBuf};

    use clap::Parser;
    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::{
        install_runner, prepare_dir, prepare_runner, prepare_with, resolve_platform,
        write_gitignore,
    };
    use crate::cli::{Cli, Command, CreateArgs};
    use crate::github::{Client, Scope};
    use crate::runner::RunnerPlatform;

    fn create_args(dir: &Path, archive: &Path) -> (Cli, CreateArgs) {
        let cli = Cli::try_parse_from([
            "octorun",
            "-r",
            "acme/app",
            "-t",
            "secret",
            "-d",
            dir.to_str().unwrap(),
            "-n",
            "runner",
            "create",
            "-l",
            archive.to_str().unwrap(),
            "-p",
            "linux-amd64",
        ])
        .unwrap();
        let args = match &cli.command {
            Command::Create(args) => args.clone(),
            _ => panic!("expected create"),
        };
        (cli, args)
    }

    fn append_file(
        builder: &mut tar::Builder<&mut GzEncoder<Vec<u8>>>,
        name: &str,
        mode: u32,
        body: &str,
    ) {
        let data = body.as_bytes();
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(mode);
        header.set_cksum();
        builder.append_data(&mut header, name, data).unwrap();
    }

    fn runner_archive(exit_code: i32) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        {
            let mut builder = tar::Builder::new(&mut encoder);
            append_file(
                &mut builder,
                "config.sh",
                0o755,
                &format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > args.txt\nexit {exit_code}\n"),
            );
            append_file(
                &mut builder,
                "config.cmd",
                0o644,
                &format!("@echo off\r\n(echo %*) > args.txt\r\nexit /b {exit_code}\r\n"),
            );
            append_file(&mut builder, "run.sh", 0o755, "#!/bin/sh\nexit 0\n");
            append_file(&mut builder, "run.cmd", 0o644, "@exit /b 0\r\n");
            builder.finish().unwrap();
        }
        encoder.finish().unwrap()
    }

    fn write_archive(dir: &Path, exit_code: i32) -> PathBuf {
        let archive = dir.join("runner.tar.gz");
        fs::write(&archive, runner_archive(exit_code)).unwrap();
        archive
    }

    fn serve_registration_token() -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 512];
            let _ = stream.read(&mut buffer);
            let body = r#"{"token":"registration-token"}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            stream.flush().unwrap();
        });
        (format!("http://{address}"), handle)
    }

    fn request_context(dir: &Path, archive: &Path) -> (Cli, CreateArgs, PathBuf) {
        let (cli, args) = create_args(dir, archive);
        let runner_dir = dir.join("runner");
        (cli, args, runner_dir)
    }

    #[test]
    fn ignores_the_runner_directory_contents() {
        let dir = tempfile::tempdir().unwrap();
        write_gitignore(dir.path()).unwrap();

        assert_eq!(
            fs::read_to_string(dir.path().join(".gitignore")).unwrap(),
            "*\n"
        );
    }

    #[test]
    fn prepare_dir_refuses_to_replace_without_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let (cli, args) = create_args(dir.path(), &dir.path().join("runner.tar.gz"));
        fs::create_dir_all(dir.path().join("runner")).unwrap();

        assert!(prepare_dir(&cli, &args, None).is_err());
    }

    #[test]
    fn prepare_dir_replaces_with_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let (cli, mut args) = create_args(dir.path(), &dir.path().join("runner.tar.gz"));
        let runner_dir = dir.path().join("runner");
        fs::create_dir_all(&runner_dir).unwrap();
        fs::write(runner_dir.join("old"), b"old").unwrap();
        args.overwrite = true;

        assert_eq!(prepare_dir(&cli, &args, None).unwrap(), runner_dir);
        assert!(!runner_dir.exists());
    }

    #[test]
    fn prepare_dir_rejects_invalid_names() {
        let dir = tempfile::tempdir().unwrap();
        let cli = Cli::try_parse_from([
            "octorun",
            "-d",
            dir.path().to_str().unwrap(),
            "-n",
            "..",
            "create",
        ])
        .unwrap();
        let Command::Create(args) = &cli.command else {
            panic!("expected create");
        };

        assert!(prepare_dir(&cli, args, None).is_err());
    }

    #[test]
    fn resolves_the_requested_or_detected_platform() {
        let dir = tempfile::tempdir().unwrap();
        let (_, mut args) = create_args(dir.path(), &dir.path().join("runner.tar.gz"));
        assert_eq!(resolve_platform(&args).unwrap(), RunnerPlatform::LinuxAmd64);

        args.platform = None;
        assert_eq!(
            resolve_platform(&args).unwrap(),
            RunnerPlatform::detect().unwrap()
        );
    }

    #[test]
    fn prepares_a_registered_runner() {
        let dir = tempfile::tempdir().unwrap();
        let archive = write_archive(dir.path(), 0);
        let (cli, args, runner_dir) = request_context(dir.path(), &archive);
        let (base, server) = serve_registration_token();
        let client = Client::with_base_url(base, "secret").unwrap();
        let scope = Scope::Repository {
            owner: "acme".to_owned(),
            name: "app".to_owned(),
        };

        let context = prepare_runner(
            &client,
            &cli,
            &args,
            runner_dir.clone(),
            "github.com",
            scope,
            "https://github.com/acme/app".to_owned(),
            RunnerPlatform::LinuxAmd64,
            false,
            true,
            None,
        )
        .unwrap();
        server.join().unwrap();

        assert_eq!(context.runner_dir, runner_dir);
        assert_eq!(
            fs::read_to_string(runner_dir.join(".gitignore")).unwrap(),
            "*\n"
        );
        let recorded = fs::read_to_string(runner_dir.join("args.txt")).unwrap();
        assert!(recorded.contains("--url"));
        assert!(recorded.contains("https://github.com/acme/app"));
        assert!(recorded.contains("--token"));
        assert!(recorded.contains("registration-token"));
    }

    #[test]
    fn prepares_a_runner_with_an_injected_client() {
        let dir = tempfile::tempdir().unwrap();
        let archive = write_archive(dir.path(), 0);
        let (cli, args) = create_args(dir.path(), &archive);
        let (base, server) = serve_registration_token();

        let context = prepare_with(&cli, &args, false, None, true, |_, _| {
            Client::with_base_url(&base, "secret")
        })
        .unwrap();
        server.join().unwrap();

        assert_eq!(
            fs::read_to_string(context.runner_dir.join(".gitignore")).unwrap(),
            "*\n"
        );
        assert!(context.runner_dir.join("args.txt").exists());
    }

    #[test]
    fn cleans_up_when_configuration_fails() {
        let dir = tempfile::tempdir().unwrap();
        let archive = write_archive(dir.path(), 1);
        let (cli, args, runner_dir) = request_context(dir.path(), &archive);
        let (base, server) = serve_registration_token();
        let client = Client::with_base_url(base, "secret").unwrap();

        let error = prepare_runner(
            &client,
            &cli,
            &args,
            runner_dir.clone(),
            "github.com",
            Scope::Organization("acme".to_owned()),
            "https://github.com/acme".to_owned(),
            RunnerPlatform::LinuxAmd64,
            false,
            true,
            None,
        )
        .unwrap_err();
        server.join().unwrap();

        assert!(matches!(error, crate::error::Error::Command(_)));
        assert!(!runner_dir.exists());
    }

    #[test]
    fn keeps_the_directory_when_cleanup_is_not_requested() {
        let dir = tempfile::tempdir().unwrap();
        let archive = write_archive(dir.path(), 1);
        let (cli, args, runner_dir) = request_context(dir.path(), &archive);
        let (base, server) = serve_registration_token();
        let client = Client::with_base_url(base, "secret").unwrap();

        let error = prepare_runner(
            &client,
            &cli,
            &args,
            runner_dir.clone(),
            "github.com",
            Scope::Organization("acme".to_owned()),
            "https://github.com/acme".to_owned(),
            RunnerPlatform::LinuxAmd64,
            false,
            false,
            None,
        )
        .unwrap_err();
        server.join().unwrap();

        assert!(matches!(error, crate::error::Error::Command(_)));
        assert!(runner_dir.exists());
    }

    #[test]
    fn installs_a_local_runner_archive() {
        let dir = tempfile::tempdir().unwrap();
        let archive = write_archive(dir.path(), 0);
        let runner_dir = dir.path().join("runner");
        let client = Client::anonymous("github.com").unwrap();

        install_runner(
            &client,
            &create_args(dir.path(), &archive).1,
            &runner_dir,
            RunnerPlatform::LinuxAmd64,
            None,
        )
        .unwrap();

        assert!(runner_dir.join("config.sh").exists());
        assert_eq!(
            fs::read_to_string(runner_dir.join(".gitignore")).unwrap(),
            "*\n"
        );
    }
}
