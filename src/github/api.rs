//! A small GitHub REST client for runner operations.

use std::thread;
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::blocking::{Client as HttpClient, RequestBuilder, Response};
use reqwest::header::{ACCEPT, AUTHORIZATION, RETRY_AFTER};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

const API_VERSION: &str = "2022-11-28";
const ACCEPT_JSON: &str = "application/vnd.github+json";
const MAX_ATTEMPTS: u32 = 3;
const BACKOFF: Duration = Duration::from_millis(200);

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
    /// Assets attached to the release.
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

/// A release asset, including its GitHub-published digest.
#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseAsset {
    /// Asset file name.
    pub name: String,
    /// Digest in the form `sha256:<hex>`, when GitHub provides one.
    pub digest: Option<String>,
    /// URL used to download the asset.
    pub browser_download_url: String,
}

impl Release {
    /// The tag with any leading `v` removed.
    #[must_use]
    pub fn version(&self) -> &str {
        self.tag_name.trim_start_matches('v')
    }

    /// Find an asset by file name.
    #[must_use]
    pub fn asset(&self, name: &str) -> Option<&ReleaseAsset> {
        self.assets.iter().find(|asset| asset.name == name)
    }
}

/// GitHub REST client used for runner operations.
pub struct Client {
    http: HttpClient,
    base_url: String,
    tokens: Vec<String>,
}

impl Client {
    /// Create a client for `host` with a single token.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built.
    pub fn new(host: &str, token: impl Into<String>) -> Result<Self> {
        Self::with_tokens(host, vec![token.into()])
    }

    /// Create a client for `host` with tokens to try in order.
    ///
    /// `github.com` uses the public API. Any other host is treated as a GitHub
    /// Enterprise instance served from `/api/v3`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::MissingToken`] for an empty token list, or an error if
    /// the HTTP client cannot be built.
    pub fn with_tokens(host: &str, tokens: Vec<String>) -> Result<Self> {
        let base_url = if host == "github.com" {
            "https://api.github.com".to_owned()
        } else {
            format!("https://{host}/api/v3")
        };
        Self::with_base_urls(base_url, tokens)
    }

    /// Create a client with an explicit API base URL and a single token.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built.
    pub fn with_base_url(base_url: impl Into<String>, token: impl Into<String>) -> Result<Self> {
        Self::with_base_urls(base_url, vec![token.into()])
    }

    /// Create a client with an explicit API base URL and tokens to try in order.
    ///
    /// # Errors
    ///
    /// Returns [`Error::MissingToken`] for an empty token list, or an error if
    /// the HTTP client cannot be built.
    pub fn with_base_urls(base_url: impl Into<String>, tokens: Vec<String>) -> Result<Self> {
        if tokens.is_empty() {
            return Err(Error::MissingToken);
        }
        Self::build(base_url, tokens)
    }

    /// Create a client that sends no token.
    ///
    /// Public endpoints such as the `actions/runner` releases accept anonymous
    /// requests.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be built.
    pub fn anonymous(host: &str) -> Result<Self> {
        let base_url = if host == "github.com" {
            "https://api.github.com".to_owned()
        } else {
            format!("https://{host}/api/v3")
        };
        Self::build(base_url, Vec::new())
    }

    fn build(base_url: impl Into<String>, tokens: Vec<String>) -> Result<Self> {
        let http = HttpClient::builder()
            .user_agent(concat!("octorun/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            tokens,
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
        self.send(|token| Self::authorized(self.http.get(url.clone()), token))
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
        let body = serde_json::json!({});

        let Token { token } =
            self.send(|token| Self::authorized(self.http.post(url.clone()).json(&body), token))?;

        if token.is_empty() {
            return Err(Error::Api(
                "GitHub returned an empty runner token".to_owned(),
            ));
        }
        Ok(token)
    }

    fn send<T: DeserializeOwned>(
        &self,
        build: impl Fn(Option<&str>) -> RequestBuilder,
    ) -> Result<T> {
        if self.tokens.is_empty() {
            return decode(Self::send_with_retries(|| build(None))?);
        }

        let mut last_unauthorized = None;

        for token in &self.tokens {
            let response = Self::send_with_retries(|| build(Some(token)))?;
            if response.status() == StatusCode::UNAUTHORIZED {
                tracing::debug!("GitHub rejected a token, trying the next one");
                last_unauthorized = Some(response);
                continue;
            }
            return decode(response);
        }

        match last_unauthorized {
            Some(response) => decode(response),
            None => Err(Error::MissingToken),
        }
    }

    fn send_with_retries(build: impl Fn() -> RequestBuilder) -> Result<Response> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            match build().send() {
                Ok(response) => {
                    if is_retryable_status(response.status()) && attempt < MAX_ATTEMPTS {
                        let delay = retry_delay(&response, attempt);
                        tracing::debug!("retrying GitHub request after {delay:?}");
                        drop(response);
                        thread::sleep(delay);
                    } else {
                        return Ok(response);
                    }
                }
                Err(error) if retryable_transport(&error) && attempt < MAX_ATTEMPTS => {
                    let delay = backoff(attempt);
                    tracing::debug!("retrying GitHub request after {delay:?}: {error}");
                    thread::sleep(delay);
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    fn authorized(request: RequestBuilder, token: Option<&str>) -> RequestBuilder {
        let request = request
            .header(ACCEPT, ACCEPT_JSON)
            .header("X-GitHub-Api-Version", API_VERSION);
        match token {
            Some(token) => request.header(AUTHORIZATION, format!("Bearer {token}")),
            None => request,
        }
    }
}

fn is_retryable_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn retryable_transport(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect() || error.is_request()
}

fn backoff(attempt: u32) -> Duration {
    BACKOFF.saturating_mul(attempt)
}

fn retry_delay(response: &Response, attempt: u32) -> Duration {
    response
        .headers()
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map_or_else(
            || backoff(attempt),
            |seconds| Duration::from_secs(seconds.min(5)),
        )
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

    use super::{Client, Release, Scope, TokenKind};
    use crate::error::Error;

    fn serve_sequence(responses: Vec<(&'static str, String)>) -> (String, JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();

        let handle = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let request = read_request(&mut stream);
                requests.push(String::from_utf8_lossy(&request).into_owned());

                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
                stream.flush().unwrap();
            }
            requests
        });

        (format!("http://{address}"), handle)
    }

    fn serve_once(status: &'static str, body: &str) -> (String, JoinHandle<Vec<String>>) {
        serve_sequence(vec![(status, body.to_owned())])
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
        let requests = server.join().unwrap();
        let request = &requests[0];

        assert_eq!(release.version(), "2.300.0");
        assert!(request.starts_with("GET /repos/actions/runner/releases/latest "));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("authorization: bearer secret")
        );
    }

    #[test]
    fn anonymous_client_omits_authorization() {
        let (base, server) = serve_once("200 OK", r#"{"tag_name":"v2.300.0"}"#);
        let client = Client::build(base, Vec::new()).unwrap();

        let release = client.latest_runner_release().unwrap();
        let requests = server.join().unwrap();

        assert_eq!(release.version(), "2.300.0");
        assert!(!requests[0].to_ascii_lowercase().contains("authorization"));
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
        let requests = server.join().unwrap();
        let request = &requests[0];

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
    fn retries_server_errors() {
        let (base, server) = serve_sequence(vec![
            ("500 Internal Server Error", "try again".to_owned()),
            ("200 OK", r#"{"tag_name":"v2.300.0"}"#.to_owned()),
        ]);
        let client = Client::with_base_url(base, "secret").unwrap();

        let release = client.latest_runner_release().unwrap();
        let requests = server.join().unwrap();

        assert_eq!(release.version(), "2.300.0");
        assert_eq!(requests.len(), 2);
    }

    #[test]
    fn falls_back_to_the_next_token() {
        let (base, server) = serve_sequence(vec![
            (
                "401 Unauthorized",
                r#"{"message":"bad credentials"}"#.to_owned(),
            ),
            ("200 OK", r#"{"tag_name":"v2.300.0"}"#.to_owned()),
        ]);
        let client =
            Client::with_base_urls(base, vec!["bad".to_owned(), "good".to_owned()]).unwrap();

        let release = client.latest_runner_release().unwrap();
        let requests = server.join().unwrap();

        assert_eq!(release.version(), "2.300.0");
        assert!(
            requests[0]
                .to_ascii_lowercase()
                .contains("authorization: bearer bad")
        );
        assert!(
            requests[1]
                .to_ascii_lowercase()
                .contains("authorization: bearer good")
        );
    }

    #[test]
    fn finds_release_assets() {
        let release: Release = serde_json::from_str(
            r#"{"tag_name":"v1","assets":[{"name":"runner.zip","digest":"sha256:abc","browser_download_url":"https://example.com/runner.zip"}]}"#,
        )
        .unwrap();

        assert_eq!(
            release.asset("runner.zip").unwrap().digest.as_deref(),
            Some("sha256:abc")
        );
        assert!(release.asset("missing.zip").is_none());
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
