//! Downloading remote files.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

const MAX_ATTEMPTS: u32 = 3;
const BACKOFF: Duration = Duration::from_millis(200);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(600);
const BUFFER_SIZE: usize = 64 * 1024;

/// Download `url` into `dest` and return the number of bytes written.
///
/// # Errors
///
/// Returns an error if the request fails, the response is not successful, or
/// the destination cannot be written.
pub fn download(url: &str, dest: &Path) -> Result<u64> {
    download_with_cancel(url, dest, None)
}

/// Download `url` into `dest`, stopping early when `cancel` is set.
///
/// The request has a connect timeout and a total timeout. Transient failures
/// are retried a few times, and a partial file is removed after a failure.
///
/// # Errors
///
/// Returns [`Error::Cancelled`] when cancellation is requested, or an error if
/// the request fails, the response is not successful, or the destination cannot
/// be written.
pub fn download_with_cancel(url: &str, dest: &Path, cancel: Option<&AtomicBool>) -> Result<u64> {
    tracing::info!("downloading {url}");

    let client = Client::builder()
        .user_agent(concat!("octorun/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()?;

    let mut attempt = 0;
    loop {
        attempt += 1;
        if is_cancelled(cancel) {
            return Err(Error::Cancelled);
        }

        match transfer(&client, url, dest, cancel) {
            Ok(bytes) => {
                tracing::info!("downloaded {bytes} bytes to {}", dest.display());
                return Ok(bytes);
            }
            Err(error) => {
                let _ = fs::remove_file(dest);
                if retryable(&error) && attempt < MAX_ATTEMPTS {
                    let delay = BACKOFF.saturating_mul(attempt);
                    tracing::debug!("retrying download after {delay:?}: {error}");
                    thread::sleep(delay);
                } else {
                    return Err(error);
                }
            }
        }
    }
}

/// Hash a file with SHA-256 and return the digest as lowercase hex.
///
/// # Errors
///
/// Returns an error if the file cannot be read.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; BUFFER_SIZE];

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(hex::encode(hasher.finalize()))
}

fn transfer(client: &Client, url: &str, dest: &Path, cancel: Option<&AtomicBool>) -> Result<u64> {
    let mut response = client.get(url).send()?.error_for_status()?;
    let mut file = File::create(dest)?;
    let mut buffer = vec![0_u8; BUFFER_SIZE];
    let mut bytes = 0_u64;

    loop {
        if is_cancelled(cancel) {
            return Err(Error::Cancelled);
        }

        let read = response.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])?;
        bytes += read as u64;
    }

    file.flush()?;
    Ok(bytes)
}

fn is_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.is_some_and(|flag| flag.load(Ordering::SeqCst))
}

fn retryable(error: &Error) -> bool {
    match error {
        Error::Http(http) => {
            http.status().is_some_and(|status| {
                status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
            }) || http.is_timeout()
                || http.is_connect()
                || http.is_request()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::AtomicBool;
    use std::thread::JoinHandle;

    use super::{download, download_with_cancel, sha256_file};
    use crate::error::Error;

    fn serve_sequence(
        responses: Vec<(&'static str, &'static str)>,
    ) -> (String, JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();

        let handle = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_request(&mut stream);
                requests.push(String::from_utf8_lossy(&request).into_owned());

                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
                stream.flush().unwrap();
            }
            requests
        });

        (format!("http://{address}"), handle)
    }

    fn read_request(stream: &mut TcpStream) -> Vec<u8> {
        let mut data = Vec::new();
        let mut buffer = [0_u8; 1024];

        loop {
            let read = match stream.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(read) => read,
            };
            data.extend_from_slice(&buffer[..read]);

            if let Some(header_end) = data.windows(4).position(|window| window == b"\r\n\r\n") {
                let header_len = header_end + 4;
                let headers = String::from_utf8_lossy(&data[..header_end]).to_ascii_lowercase();
                let content_length = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if data.len() >= header_len + content_length {
                    break;
                }
            }
        }

        data
    }

    #[test]
    fn downloads_response_body_to_file() {
        let (base, server) = serve_sequence(vec![("200 OK", "runner-bytes")]);

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("runner.tar.gz");
        let url = format!("{base}/runner.tar.gz");

        let bytes = download(&url, &dest).unwrap();
        server.join().unwrap();

        assert_eq!(bytes, 12);
        assert_eq!(fs::read(&dest).unwrap(), b"runner-bytes");
    }

    #[test]
    fn retries_server_errors() {
        let (base, server) = serve_sequence(vec![
            ("500 Internal Server Error", ""),
            ("200 OK", "runner-bytes"),
        ]);

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("runner.tar.gz");
        let url = format!("{base}/runner.tar.gz");

        let bytes = download(&url, &dest).unwrap();
        let requests = server.join().unwrap();

        assert_eq!(bytes, 12);
        assert_eq!(fs::read(&dest).unwrap(), b"runner-bytes");
        assert_eq!(requests.len(), 2);
    }

    #[test]
    fn leaves_no_file_on_http_error() {
        let (base, server) = serve_sequence(vec![("404 Not Found", "")]);

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("missing.tar.gz");
        let url = format!("{base}/missing.tar.gz");

        assert!(download(&url, &dest).is_err());
        server.join().unwrap();
        assert!(!dest.exists());
    }

    #[test]
    fn stops_before_connecting_when_cancelled() {
        let flag = AtomicBool::new(true);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("runner.tar.gz");

        let error = download_with_cancel("http://127.0.0.1:1/runner.tar.gz", &dest, Some(&flag))
            .unwrap_err();

        assert!(matches!(error, Error::Cancelled));
        assert!(!dest.exists());
    }

    #[test]
    fn hashes_file_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("abc");
        fs::write(&path, b"abc").unwrap();

        assert_eq!(
            sha256_file(&path).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
