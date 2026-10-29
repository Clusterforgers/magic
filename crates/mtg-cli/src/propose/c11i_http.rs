//! The HTTP side every model API shares: the key that never shows, the client, and 429 and 5xx answers retried with a
//! growing wait. A [`Dialect`] supplies the endpoint, the headers, the body and the reading of an answer.

use std::time::Duration;

use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue, RETRY_AFTER};
use reqwest::{Client, StatusCode};
use serde_json::Value;
use tokio::runtime::Handle;

use super::model::{ModelClient, ModelError, Request, Response};
use crate::progress::Progress;

/// The address of the Anthropic API, the default of [`HttpConfig`].
pub const DEFAULT_BASE: &str = "https://api.anthropic.com";

/// The longest wait between two tries.
const MAX_WAIT: Duration = Duration::from_secs(60);

/// Time beyond the client timeout after which a request is dropped by force, so that nothing a connection does can hang a run.
const DEADLINE_GRACE: Duration = Duration::from_secs(5);

/// How much of an unparsed error body is kept in a message.
const BODY_KEPT: usize = 300;

/// An API key. It has no `Display`, its `Debug` hides it and the client scrubs it from every message it returns.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

impl ApiKey {
    /// The key in the variable `name` as `lookup` sees the environment; empty counts as missing.
    pub fn c11i_from_variable(lookup: impl Fn(&str) -> Option<String>, name: &str) -> Option<Self> {
        lookup(name)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .map(ApiKey)
    }

    /// The key as a header value marked sensitive, after `prefix` (such as `Bearer `).
    pub fn c11i_header(&self, prefix: &str) -> Result<HeaderValue, ModelError> {
        let mut value = HeaderValue::from_str(&format!("{prefix}{}", self.0))
            .map_err(|_| ModelError::Client("the API key is not a valid header value".into()))?;
        value.set_sensitive(true);
        Ok(value)
    }

    /// `text` with every occurrence of the key replaced.
    pub fn scrub(&self, text: &str) -> String {
        text.replace(&self.0, "[redacted]")
    }
}

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApiKey([redacted])")
    }
}

/// How the client reaches an API.
#[derive(Debug, Clone)]
pub struct HttpConfig {
    /// The address without the endpoint's path; tests point it at a local server.
    pub base_url: String,
    /// Tries per request, the first included.
    pub attempts: u32,
    /// The wait before the second try; it doubles for each further one.
    pub backoff: Duration,
    /// How long the API may take to answer.
    pub timeout: Duration,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE.to_owned(),
            attempts: 4,
            backoff: Duration::from_secs(2),
            timeout: Duration::from_secs(300),
        }
    }
}

/// What differs between two model APIs.
pub trait Dialect: std::fmt::Debug + Send + Sync {
    /// The endpoint's path after the base address, such as `/v1/messages`.
    fn path(&self) -> &'static str;
    /// The headers beside the content type, the key's among them.
    fn headers(&self, key: Option<&ApiKey>) -> Result<HeaderMap, ModelError>;
    /// The JSON body of a request.
    fn body(&self, request: &Request) -> Value;
    /// Reads a successful answer to `request`.
    fn parse(&self, request: &Request, body: &str) -> Result<Response, ModelError>;
}

/// The HTTP client of a model API. [`ModelClient::complete`] blocks, so call it from a blocking thread
/// (`tokio::task::spawn_blocking`), not from async code.
#[derive(Debug)]
pub struct HttpClient<D> {
    client: Client,
    handle: Handle,
    endpoint: String,
    key: Option<ApiKey>,
    attempts: u32,
    backoff: Duration,
    timeout: Duration,
    progress: Progress,
    dialect: D,
}

impl<D: Dialect> HttpClient<D> {
    /// Builds a client that speaks `dialect` and runs its requests on the runtime of `handle`.
    pub fn c11i_build(
        dialect: D,
        key: Option<ApiKey>,
        config: &HttpConfig,
        handle: Handle,
        progress: Progress,
    ) -> Result<Self, ModelError> {
        let mut headers = dialect.headers(key.as_ref())?;
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let client = Client::builder()
            .user_agent(format!("mtg-cli/{}", env!("CARGO_PKG_VERSION")))
            .default_headers(headers)
            .timeout(config.timeout)
            .connect_timeout(Duration::from_secs(15))
            .pool_max_idle_per_host(0)
            .build()
            .map_err(|error| ModelError::Client(error.to_string()))?;
        Ok(Self {
            client,
            handle,
            endpoint: format!(
                "{}{}",
                config.base_url.trim_end_matches('/'),
                dialect.path()
            ),
            key,
            attempts: config.attempts.max(1),
            backoff: config.backoff,
            timeout: config.timeout,
            progress,
            dialect,
        })
    }

    fn scrub(&self, text: &str) -> String {
        self.key
            .as_ref()
            .map_or_else(|| text.to_owned(), |key| key.scrub(text))
    }

    async fn send_once(&self, request: &Request) -> Result<Response, ModelError> {
        let body = self.dialect.body(request).to_string();
        let response = self
            .client
            .post(&self.endpoint)
            .body(body)
            .send()
            .await
            .map_err(|error| self.transport_error(error))?;
        let status = response.status();
        let retry_after = retry_after(response.headers());
        let text = response
            .text()
            .await
            .map_err(|error| self.transport_error(error))?;
        if status.is_success() {
            self.dialect.parse(request, &text)
        } else {
            Err(status_error(status, &text, retry_after))
        }
    }

    /// One try, cancelled when it outlives the timeout (and a grace for the client's own timer to fire first).
    async fn send_bounded(&self, request: &Request) -> Result<Response, ModelError> {
        tokio::time::timeout(self.timeout + DEADLINE_GRACE, self.send_once(request))
            .await
            .unwrap_or(Err(ModelError::Timeout {
                after: self.timeout,
            }))
    }

    fn transport_error(&self, error: reqwest::Error) -> ModelError {
        let error = error.without_url();
        if error.is_timeout() {
            ModelError::Timeout {
                after: self.timeout,
            }
        } else if error.is_connect() {
            ModelError::Unreachable {
                endpoint: self.endpoint.clone(),
                detail: error_chain(&error),
            }
        } else {
            ModelError::Transport(error_chain(&error))
        }
    }

    async fn send_with_retries(&self, request: &Request) -> Result<Response, ModelError> {
        let mut attempt = 1;
        loop {
            match self.send_bounded(request).await {
                Err(error) if attempt < self.attempts && error.is_retryable() => {
                    let wait = error
                        .retry_after()
                        .unwrap_or_else(|| self.backoff.saturating_mul(1 << (attempt - 1).min(10)))
                        .min(MAX_WAIT);
                    self.progress.line(format_args!(
                        "retrying in {:.1} s after attempt {attempt} of {}: {}",
                        wait.as_secs_f64(),
                        self.attempts,
                        self.scrub(&error.to_string())
                    ));
                    tokio::time::sleep(wait).await;
                    attempt += 1;
                }
                other => return other,
            }
        }
    }
}

impl<D: Dialect> ModelClient for HttpClient<D> {
    fn complete(&self, request: &Request) -> Result<Response, ModelError> {
        self.handle
            .block_on(self.send_with_retries(request))
            .map_err(|error| match error {
                ModelError::Status {
                    status,
                    message,
                    retry_after,
                } => ModelError::Status {
                    status,
                    message: self.scrub(&message),
                    retry_after,
                },
                ModelError::Transport(message) => ModelError::Transport(self.scrub(&message)),
                ModelError::Unreachable { endpoint, detail } => ModelError::Unreachable {
                    endpoint,
                    detail: self.scrub(&detail),
                },
                other => other,
            })
    }
}

/// The error and its causes on one line, without the address (the caller knows it).
fn error_chain(error: &reqwest::Error) -> String {
    let mut text = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

/// The error of a failed request: the API's own message when the body has one (`{"error":{"type","message"}}`, which
/// the Anthropic and the OpenAI-style APIs share), else the start of the body.
pub fn status_error(status: StatusCode, body: &str, retry_after: Option<Duration>) -> ModelError {
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            let error = value.get("error")?;
            let kind = error.get("type").and_then(Value::as_str).unwrap_or("error");
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .or_else(|| error.as_str())?;
            Some(format!("{kind}: {message}"))
        })
        .unwrap_or_else(|| body.chars().take(BODY_KEPT).collect());
    ModelError::Status {
        status: status.as_u16(),
        message,
        retry_after,
    }
}

fn retry_after(headers: &HeaderMap) -> Option<Duration> {
    headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}
