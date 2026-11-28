//! A library interface for creating and managing runners.

use std::path::{Path, PathBuf};

use crate::cli::{Cli, Command, CreateArgs, RemoveArgs};
use crate::commands;
use crate::error::Result;
use crate::github::Client;
use crate::runner::RunnerPlatform;

/// Options for [`Runner::create`].
///
/// ```
/// use octorun::CreateOptions;
///
/// let options = CreateOptions::new("my-runner")
///     .repo("acme/app")
///     .dir("/tmp/runners");
/// assert_eq!(options.repo.as_deref(), Some("acme/app"));
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CreateOptions {
    /// Display name for the runner.
    pub name: String,
    /// Directory that holds the runner (defaults to the current directory).
    pub dir: Option<PathBuf>,
    /// Repository in `OWNER/REPO` form (defaults to the current repository).
    pub repo: Option<String>,
    /// GitHub token (defaults to the token found automatically).
    pub token: Option<String>,
    /// Archive URL, archive path, or directory to install instead of downloading.
    pub location: Option<String>,
    /// Runner platform (defaults to the detected platform).
    pub platform: Option<RunnerPlatform>,
    /// Comma separated list of extra labels.
    pub labels: Option<String>,
    /// Enterprise name.
    pub enterprise: Option<String>,
    /// Organization name.
    pub org: Option<String>,
    /// Replace an existing runner directory.
    pub overwrite: bool,
}

impl Default for CreateOptions {
    fn default() -> Self {
        Self {
            name: "actions-runner".to_owned(),
            dir: None,
            repo: None,
            token: None,
            location: None,
            platform: None,
            labels: None,
            enterprise: None,
            org: None,
            overwrite: false,
        }
    }
}

impl CreateOptions {
    /// Create options for a runner with `name`.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }

    /// Set the directory that holds the runner.
    #[must_use]
    pub fn dir(mut self, dir: impl AsRef<Path>) -> Self {
        self.dir = Some(dir.as_ref().to_path_buf());
        self
    }

    /// Set the repository in `OWNER/REPO` form.
    #[must_use]
    pub fn repo(mut self, repo: impl Into<String>) -> Self {
        self.repo = Some(repo.into());
        self
    }

    /// Set the GitHub token.
    #[must_use]
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Install from a URL, a local archive, or a directory.
    #[must_use]
    pub fn location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    /// Set the runner platform.
    #[must_use]
    pub fn platform(mut self, platform: RunnerPlatform) -> Self {
        self.platform = Some(platform);
        self
    }

    /// Set extra labels.
    #[must_use]
    pub fn labels(mut self, labels: impl Into<String>) -> Self {
        self.labels = Some(labels.into());
        self
    }

    /// Set the enterprise name.
    #[must_use]
    pub fn enterprise(mut self, enterprise: impl Into<String>) -> Self {
        self.enterprise = Some(enterprise.into());
        self
    }

    /// Set the organization name.
    #[must_use]
    pub fn org(mut self, org: impl Into<String>) -> Self {
        self.org = Some(org.into());
        self
    }

    /// Replace an existing runner directory.
    #[must_use]
    pub fn overwrite(mut self) -> Self {
        self.overwrite = true;
        self
    }

    fn create_args(&self) -> CreateArgs {
        CreateArgs {
            overwrite: self.overwrite,
            location: self.location.clone(),
            platform: self.platform,
            labels: self.labels.clone(),
            enterprise: self.enterprise.clone(),
            org: self.org.clone(),
            no_configuration: false,
        }
    }

    fn cli(&self, command: Command) -> Cli {
        Cli {
            verbose: false,
            name: self.name.clone(),
            dir: self.dir.clone(),
            repo: self.repo.clone(),
            token: self.token.clone(),
            command,
        }
    }
}

/// A runner directory managed by octorun.
///
/// ```no_run
/// use octorun::{CreateOptions, Runner};
///
/// # fn main() -> octorun::Result<()> {
/// let runner = Runner::create(CreateOptions::new("my-runner").repo("acme/app"))?;
/// # let _ = runner;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
#[non_exhaustive]
pub struct Runner {
    options: CreateOptions,
    runner_dir: PathBuf,
}

impl Runner {
    /// Create and configure a runner.
    ///
    /// The runner files are downloaded, or read from
    /// [`CreateOptions::location`], and registered with GitHub.
    ///
    /// # Errors
    ///
    /// Returns an error if the repository, target, or token cannot be
    /// resolved, the runner files cannot be installed, or configuration fails.
    pub fn create(options: CreateOptions) -> Result<Self> {
        Self::create_with(options, |cli, host| {
            let tokens = crate::github::resolve_tokens_with(cli.token.as_deref(), host)?;
            Client::with_tokens(host, tokens)
        })
    }

    pub(crate) fn create_with(
        options: CreateOptions,
        client_for: impl FnOnce(&Cli, &str) -> Result<Client>,
    ) -> Result<Self> {
        let args = options.create_args();
        let cli = options.cli(Command::Create(args.clone()));
        let context = commands::create::prepare_with(&cli, &args, false, None, true, client_for)?;
        Ok(Self {
            options,
            runner_dir: context.runner_dir,
        })
    }

    /// The directory that holds the runner.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.runner_dir
    }

    /// Start the runner and return its process id.
    ///
    /// # Errors
    ///
    /// Returns an error if the runner is already running or cannot be spawned.
    pub fn start(&self) -> Result<u32> {
        crate::runner::process::start(&self.runner_dir)
    }

    /// Stop the runner and return its process id.
    ///
    /// # Errors
    ///
    /// Returns an error if no process is recorded or it cannot be terminated.
    pub fn stop(&self) -> Result<u32> {
        crate::runner::process::stop(&self.runner_dir)
    }

    /// Restart the runner, starting it when stopped unless `if_running` is set.
    ///
    /// # Errors
    ///
    /// Returns an error if the old process cannot be stopped, it does not exit
    /// in time, or the new process cannot be started.
    pub fn restart(&self, if_running: bool) -> Result<()> {
        commands::restart::restart(&self.runner_dir, &self.options.name, if_running)
    }

    /// Remove the runner and its directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the runner cannot be deregistered or the directory
    /// cannot be removed.
    pub fn remove(self) -> Result<()> {
        let args = RemoveArgs {
            enterprise: self.options.enterprise.clone(),
            org: self.options.org.clone(),
        };
        let name = self.runner_dir.file_name().map_or_else(
            || self.options.name.clone(),
            |name| name.to_string_lossy().into_owned(),
        );
        let cli = Cli {
            verbose: false,
            name,
            dir: self.runner_dir.parent().map(Path::to_path_buf),
            repo: self.options.repo.clone(),
            token: self.options.token.clone(),
            command: Command::Remove(args.clone()),
        };
        commands::remove::run(&cli, &args)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::{Path, PathBuf};

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::{CreateOptions, Runner};
    use crate::github::Client;
    use crate::runner::RunnerPlatform;

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

    fn runner_archive() -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        {
            let mut builder = tar::Builder::new(&mut encoder);
            append_file(
                &mut builder,
                "config.sh",
                0o755,
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > args.txt\nexit 0\n",
            );
            append_file(
                &mut builder,
                "config.cmd",
                0o644,
                "@echo off\r\n(echo %*) > args.txt\r\nexit /b 0\r\n",
            );
            builder.finish().unwrap();
        }
        encoder.finish().unwrap()
    }

    fn write_archive(dir: &Path) -> PathBuf {
        let archive = dir.join("runner.tar.gz");
        fs::write(&archive, runner_archive()).unwrap();
        archive
    }

    fn serve_token() -> (String, std::thread::JoinHandle<()>) {
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

    fn write_run_script(dir: &Path) {
        let path = dir.join(if cfg!(windows) { "run.cmd" } else { "run.sh" });
        #[cfg(unix)]
        fs::write(&path, "#!/bin/sh\nsleep 30\n").unwrap();
        #[cfg(windows)]
        fs::write(&path, "@ping -n 30 127.0.0.1 >NUL\r\n").unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).unwrap();
        }
    }

    fn runner_with(dir: PathBuf) -> Runner {
        Runner {
            options: CreateOptions::new("runner"),
            runner_dir: dir,
        }
    }

    #[test]
    fn builds_options() {
        let options = CreateOptions::new("fast")
            .repo("acme/app")
            .dir("work")
            .platform(RunnerPlatform::LinuxAmd64)
            .labels("self-hosted,linux")
            .token("secret")
            .enterprise("acme-inc")
            .org("acme")
            .overwrite();

        assert_eq!(options.name, "fast");
        assert_eq!(options.repo.as_deref(), Some("acme/app"));
        assert_eq!(options.dir.as_deref(), Some(Path::new("work")));
        assert_eq!(options.platform, Some(RunnerPlatform::LinuxAmd64));
        assert_eq!(options.labels.as_deref(), Some("self-hosted,linux"));
        assert_eq!(options.token.as_deref(), Some("secret"));
        assert_eq!(options.enterprise.as_deref(), Some("acme-inc"));
        assert_eq!(options.org.as_deref(), Some("acme"));
        assert!(options.overwrite);
    }

    #[test]
    fn creates_a_runner_with_an_injected_client() {
        let dir = tempfile::tempdir().unwrap();
        let archive = write_archive(dir.path());
        let (base, server) = serve_token();
        let options = CreateOptions::new("runner")
            .repo("acme/app")
            .dir(dir.path())
            .location(archive.to_string_lossy().to_string())
            .platform(RunnerPlatform::LinuxAmd64);

        let runner =
            Runner::create_with(options, |_, _| Client::with_base_url(&base, "token")).unwrap();
        server.join().unwrap();

        assert_eq!(runner.dir().to_path_buf(), dir.path().join("runner"));
        assert!(runner.dir().join(".gitignore").exists());
    }

    #[test]
    fn starts_and_stops_a_runner() {
        let parent = tempfile::tempdir().unwrap();
        let runner_dir = parent.path().join("runner");
        fs::create_dir_all(&runner_dir).unwrap();
        write_run_script(&runner_dir);
        let runner = runner_with(runner_dir);

        let pid = runner.start().unwrap();

        assert_eq!(runner.stop().unwrap(), pid);
    }

    #[test]
    fn restarts_only_a_running_runner_when_asked() {
        let parent = tempfile::tempdir().unwrap();
        let runner_dir = parent.path().join("runner");
        fs::create_dir_all(&runner_dir).unwrap();
        write_run_script(&runner_dir);
        let runner = runner_with(runner_dir.clone());

        runner.restart(true).unwrap();

        assert!(!runner_dir.join(".octorun.pid").exists());
    }

    #[test]
    fn removes_the_runner_directory() {
        let parent = tempfile::tempdir().unwrap();
        let runner_dir = parent.path().join("runner");
        fs::create_dir_all(&runner_dir).unwrap();
        let runner = runner_with(runner_dir.clone());

        runner.remove().unwrap();

        assert!(!runner_dir.exists());
    }
}
