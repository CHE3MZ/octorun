//! Downloading remote files.

use std::fs::{self, File};
use std::path::Path;

use reqwest::blocking::Client;

use crate::error::Result;

/// Download `url` into `dest` and return the number of bytes written.
///
/// A partially written file is removed if the transfer fails.
///
/// # Errors
///
/// Returns an error if the request fails, the response is not successful, or
/// the destination cannot be written.
pub fn download(url: &str, dest: &Path) -> Result<u64> {
    tracing::info!("downloading {url}");

    let client = Client::builder()
        .user_agent(concat!("octorun/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let mut response = client.get(url).send()?.error_for_status()?;

    let mut file = File::create(dest)?;
    match response.copy_to(&mut file) {
        Ok(bytes) => {
            tracing::info!("downloaded {bytes} bytes to {}", dest.display());
            Ok(bytes)
        }
        Err(error) => {
            drop(file);
            let _ = fs::remove_file(dest);
            Err(error.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::download;

    #[test]
    fn downloads_response_body_to_file() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);

            let body = b"runner-bytes";
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
            stream.flush().unwrap();
        });

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("runner.tar.gz");
        let url = format!("http://{address}/runner.tar.gz");

        let bytes = download(&url, &dest).unwrap();
        server.join().unwrap();

        assert_eq!(bytes, 12);
        assert_eq!(fs::read(&dest).unwrap(), b"runner-bytes");
    }

    #[test]
    fn leaves_no_file_on_http_error() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            stream
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
            stream.flush().unwrap();
        });

        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("missing.tar.gz");
        let url = format!("http://{address}/missing.tar.gz");

        assert!(download(&url, &dest).is_err());
        server.join().unwrap();
        assert!(!dest.exists());
    }
}
