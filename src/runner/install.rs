//! Installing the runner files into a destination directory.

use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use crate::error::{Error, Result};
use crate::github::Client;
use crate::runner::{RunnerPlatform, download_with_cancel, extract, sha256_file};

/// Install runner files into `dest`.
///
/// `location` may be an archive URL, a local archive, or a directory. When it
/// is `None`, the latest `platform` release is resolved through `api` and its
/// published digest is verified.
///
/// # Errors
///
/// Returns an error if the release cannot be resolved, the download fails, the
/// archive does not match its digest, the archive cannot be extracted, or the
/// source is missing.
pub fn install(
    api: &Client,
    location: Option<&str>,
    dest: &Path,
    platform: RunnerPlatform,
    cancel: Option<&AtomicBool>,
) -> Result<()> {
    match location {
        Some(location) if is_url(location) => install_from_url(location, dest, None, cancel),
        Some(location) => install_from_path(Path::new(location), dest),
        None => {
            let release = api.latest_runner_release()?;
            let archive = platform.archive_name(release.version());

            let (url, digest) = if let Some(asset) = release.asset(&archive) {
                (asset.browser_download_url.clone(), asset.digest.clone())
            } else {
                let tag = &release.tag_name;
                (
                    format!("https://github.com/actions/runner/releases/download/{tag}/{archive}"),
                    None,
                )
            };

            install_from_url(&url, dest, digest.as_deref(), cancel)
        }
    }
}

fn install_from_path(source: &Path, dest: &Path) -> Result<()> {
    if source.is_dir() {
        copy_dir(source, dest)
    } else if source.is_file() {
        extract(source, dest)
    } else {
        Err(Error::Config(format!(
            "runner location not found: {}",
            source.display()
        )))
    }
}

fn install_from_url(
    url: &str,
    dest: &Path,
    digest: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> Result<()> {
    let created_dest = !dest.exists();
    fs::create_dir_all(dest)?;

    let file_name = archive_file_name(url);
    let archive_path = dest.join(format!(".octorun-{file_name}"));

    if let Err(error) = download_with_cancel(url, &archive_path, cancel) {
        if created_dest {
            let _ = fs::remove_dir_all(dest);
        }
        return Err(error);
    }

    let result = (|| {
        if let Some(digest) = digest {
            verify_digest(&archive_path, digest)?;
        }
        extract(&archive_path, dest)
    })();

    if let Err(error) = fs::remove_file(&archive_path) {
        tracing::warn!(
            "could not remove temporary archive {}: {error}",
            archive_path.display()
        );
    }

    if result.is_err() && created_dest {
        let _ = fs::remove_dir_all(dest);
    }

    result
}

fn verify_digest(path: &Path, digest: &str) -> Result<()> {
    let Some(expected) = digest.strip_prefix("sha256:") else {
        tracing::warn!("unsupported digest `{digest}`, skipping verification");
        return Ok(());
    };

    let actual = sha256_file(path)?;
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(Error::Checksum(format!(
            "expected sha256 {expected}, got {actual}"
        )));
    }

    Ok(())
}

fn copy_dir(source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let target = dest.join(entry.file_name());

        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }

    Ok(())
}

fn is_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn archive_file_name(url: &str) -> &str {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    path.rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("runner.tar.gz")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::{archive_file_name, copy_dir, install, is_url, verify_digest};
    use crate::github::Client;
    use crate::runner::RunnerPlatform;

    fn serve_once(body: Vec<u8>) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();

        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 1024];
            let _ = stream.read(&mut buffer);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(&body).unwrap();
            stream.flush().unwrap();
        });

        (format!("http://{address}/runner.tar.gz"), handle)
    }

    #[test]
    fn recognizes_urls() {
        assert!(is_url("https://example.com/runner.tar.gz"));
        assert!(is_url("http://example.com/runner.zip"));
        assert!(!is_url("./runner.tar.gz"));
        assert!(!is_url("runner.zip"));
    }

    #[test]
    fn extracts_file_name_from_url() {
        assert_eq!(
            archive_file_name("https://example.com/actions-runner-linux-x64-2.1.0.tar.gz"),
            "actions-runner-linux-x64-2.1.0.tar.gz"
        );
        assert_eq!(
            archive_file_name("https://example.com/runner.zip?token=abc"),
            "runner.zip"
        );
        assert_eq!(archive_file_name("https://example.com/"), "runner.tar.gz");
    }

    #[test]
    fn copies_directory_tree() {
        let source = tempfile::tempdir().unwrap();
        let dest = tempfile::tempdir().unwrap();

        fs::create_dir_all(source.path().join("bin")).unwrap();
        fs::write(source.path().join("config.sh"), b"#!/bin/sh").unwrap();
        fs::write(source.path().join("bin").join("Runner.Listener"), b"bin").unwrap();

        copy_dir(source.path(), dest.path()).unwrap();

        assert_eq!(
            fs::read(dest.path().join("config.sh")).unwrap(),
            b"#!/bin/sh"
        );
        assert_eq!(
            fs::read(dest.path().join("bin").join("Runner.Listener")).unwrap(),
            b"bin"
        );
    }

    #[test]
    fn verifies_sha256_digests() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data");
        fs::write(&path, b"abc").unwrap();

        verify_digest(
            &path,
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        )
        .unwrap();
        assert!(verify_digest(&path, "sha256:deadbeef").is_err());
    }

    #[test]
    fn installs_from_a_url_with_a_staging_archive_inside_dest() {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        {
            let mut builder = tar::Builder::new(&mut encoder);
            let data = b"#!/bin/sh\n";
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder
                .append_data(&mut header, "config.sh", &data[..])
                .unwrap();
            builder.finish().unwrap();
        }
        let bytes = encoder.finish().unwrap();

        let (url, server) = serve_once(bytes);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("runner");
        let client = Client::with_base_url("http://127.0.0.1:1", "token").unwrap();

        install(&client, Some(&url), &dest, RunnerPlatform::LinuxAmd64, None).unwrap();
        server.join().unwrap();

        assert_eq!(
            fs::read_to_string(dest.join("config.sh")).unwrap(),
            "#!/bin/sh\n"
        );
        assert!(!dest.join(".octorun-runner.tar.gz").exists());
        assert!(!dir.path().join(".octorun-runner.tar.gz").exists());
    }
}
