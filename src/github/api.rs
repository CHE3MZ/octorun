//! A small GitHub REST client for runner operations.

use std::time::Duration;

use reqwest::blocking::{Client as HttpClient, Response};
use reqwest::header::{ACCEPT, AUTHORIZATION};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

const API_VERSION: &str = "2022-11-28";
const ACCEPT_JSON: &str = "application/vnd.github+json";

/// The scope a runner is registered in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// A single repository.
    Repository {
        /// Repository owner.
        owner: String,
        /// Repository name.
        name: String,
    },
    /// An organization.
    Organization(String),
    /// An enterprise.
    Enterprise(String),
}

impl Scope {
    fn path(&self) -> String {
        match self {
            Self::Repository { owner, name } => format!("repos/{owner}/{name}"),
            Self::Organization(organization) => format!("orgs/{organization}"),
            Self::Enterprise(enterprise) => format!("enterprises/{enterprise}"),
        }
    }
}

/// The kind of runner token to request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// Token used to register a new runner.
    Registration,
    /// Token used to remove an existing runner.
    Remove,
}

impl TokenKind {
    fn path(self) -> &'static str {
        match self {
            Self::Registration => "registration-token",
            Self::Remove => "remove-token",
        }
    }
}

/// The latest release of the `actions/runner` repository.
#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    /// The release tag, e.g. `v2.300.0`.
    #[serde(rename = "tag_name")]
    pub tag_name: String,
}

impl Release {
    /// The tag with any leading `v` removed.
    #[must_use]
    pub fn version(&self) -> &str {
        self.tag_name.trim_start_matches('v')
    }
}

/// GitHub REST client used for runner operations.
pub struct Client {
    http: HttpClient,
    base_url: String,
    token: String,
}

impl Client {
    /// Create a client for `host`.
    ///
    /// `github.com` uses the public API. Any other host is treated as a GitHub
    /// Enterprise instance served from `/api/v3`.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built.
    pub fn new(host: &str, token: impl Into<String>) -> Result<Self> {
        let base_url = if host == "github.com" {
            "https://api.github.com".to_owned()
        } else {
            format!("https://{host}/api/v3")
        };
        Self::with_base_url(base_url, token)
    }

    /// Create a client with an explicit API base URL.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built.
    pub fn with_base_url(base_url: impl Into<String>, token: impl Into<String>) -> Result<Self> {
        let http = HttpClient::builder()
            .user_agent(concat!("octorun/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            token: token.into(),
        })
    }

    /// Fetch the latest `actions/runner` release.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails or the response is not successful.
    pub fn latest_runner_release(&self) -> Result<Release> {
        let base = &self.base_url;
        let url = format!("{base}/repos/actions/runner/releases/latest");
        let response = self.authorize(self.http.get(url)).send()?;
        decode(response)
    }

    /// Request a runner token for `scope`.
    ///
    /// # Errors
    ///
    /// Returns an error if the request fails, the response is not successful,
    /// or the returned token is empty.
    pub fn runner_token(&self, scope: &Scope, kind: TokenKind) -> Result<String> {
        #[derive(Deserialize)]
        struct Token {
            token: String,
        }

        let base = &self.base_url;
        let scope = scope.path();
        let kind = kind.path();
        let url = format!("{base}/{scope}/actions/runners/{kind}");

        let response = self
            .authorize(self.http.post(url))
            .json(&serde_json::json!({}))
            .send()?;
        let Token { token } = decode(response)?;

        if token.is_empty() {
            return Err(Error::Api(
                "GitHub returned an empty runner token".to_owned(),
            ));
        }
        Ok(token)
    }

    fn authorize(
        &self,
        request: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        let token = &self.token;
        request
            .header(ACCEPT, ACCEPT_JSON)
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header("X-GitHub-Api-Version", API_VERSION)
    }
}

fn decode<T: DeserializeOwned>(response: Response) -> Result<T> {
    let status = response.status();
    if !status.is_success() {
        let code = status.as_u16();
        let body = response.text().unwrap_or_default();
        let body = body.trim();
        return Err(Error::Api(format!(
            "request failed with status {code}: {body}"
        )));
    }
    Ok(response.json()?)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread::JoinHandle;

    use super::{Client, Scope, TokenKind};
    use crate::error::Error;

    fn serve_once(status: &str, body: &str) -> (String, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let status = status.to_owned();
        let body = body.to_owned();

        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&mut stream);
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.flush().unwrap();
            String::from_utf8_lossy(&request).into_owned()
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
    fn fetches_latest_runner_release() {
        let (base, server) = serve_once("200 OK", r#"{"tag_name":"v2.300.0"}"#);
        let client = Client::with_base_url(base, "secret").unwrap();

        let release = client.latest_runner_release().unwrap();
        let request = server.join().unwrap();

        assert_eq!(release.version(), "2.300.0");
        assert!(request.starts_with("GET /repos/actions/runner/releases/latest "));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer secret")
        );
    }

    #[test]
    fn requests_repository_registration_token() {
        let (base, server) = serve_once("200 OK", r#"{"token":"runner-token"}"#);
        let client = Client::with_base_url(base, "secret").unwrap();
        let scope = Scope::Repository {
            owner: "acme".to_owned(),
            name: "app".to_owned(),
        };

        let token = client
            .runner_token(&scope, TokenKind::Registration)
            .unwrap();
        let request = server.join().unwrap();

        assert_eq!(token, "runner-token");
        assert!(request.starts_with("POST /repos/acme/app/actions/runners/registration-token "));
    }

    #[test]
    fn reports_api_errors() {
        let (base, server) = serve_once("403 Forbidden", r#"{"message":"forbidden"}"#);
        let client = Client::with_base_url(base, "secret").unwrap();

        let error = client.latest_runner_release().unwrap_err();
        server.join().unwrap();

        match error {
            Error::Api(message) => {
                assert!(message.contains("403") && message.contains("forbidden"));
            }
            other => panic!("expected api error, got {other:?}"),
        }
    }

    #[test]
    fn builds_scope_paths() {
        assert_eq!(
            Scope::Repository {
                owner: "acme".to_owned(),
                name: "app".to_owned(),
            }
            .path(),
            "repos/acme/app"
        );
        assert_eq!(Scope::Organization("acme".to_owned()).path(), "orgs/acme");
        assert_eq!(
            Scope::Enterprise("acme".to_owned()).path(),
            "enterprises/acme"
        );
    }

    #[test]
    fn builds_token_kinds() {
        assert_eq!(TokenKind::Registration.path(), "registration-token");
        assert_eq!(TokenKind::Remove.path(), "remove-token");
    }
}
