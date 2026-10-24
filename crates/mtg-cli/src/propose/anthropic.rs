//! The Anthropic Messages API as a [`ModelClient`]: the key comes from the environment and goes into one header only,
//! 429 and 5xx answers are retried with a growing wait, and the system prompt is marked for caching.

use std::time::Duration;

use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, RETRY_AFTER};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::model::{ModelClient, ModelError, Request, Response, Role, Usage};
use crate::progress::Progress;

/// The environment variable that holds the API key; nothing else supplies it.
pub const KEY_VARIABLE: &str = "ANTHROPIC_API_KEY";

/// The address of the API.
pub const DEFAULT_BASE: &str = "https://api.anthropic.com";

/// The API version header value.
const API_VERSION: &str = "2023-06-01";

/// The longest wait between two tries.
const MAX_WAIT: Duration = Duration::from_secs(60);

/// How much of an unparsed error body is kept in a message.
const BODY_KEPT: usize = 300;

/// The API key. It has no `Display`, its `Debug` hides it and the client scrubs it from every message it returns.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

impl ApiKey {
    /// The key from `ANTHROPIC_API_KEY` as `lookup` sees the environment; empty counts as missing.
    pub fn from_env(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ModelError> {
        lookup(KEY_VARIABLE)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .map(ApiKey)
            .ok_or(ModelError::MissingKey)
    }

    fn header(&self) -> Result<HeaderValue, ModelError> {
        let mut value = HeaderValue::from_str(&self.0)
            .map_err(|_| ModelError::Client("the API key is not a valid header value".into()))?;
        value.set_sensitive(true);
        Ok(value)
    }

    /// `text` with every occurrence of the key replaced.
    fn scrub(&self, text: &str) -> String {
        text.replace(&self.0, "[redacted]")
    }
}

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApiKey([redacted])")
    }
}

/// How the client reaches the API.
#[derive(Debug, Clone)]
pub struct AnthropicConfig {
    /// The address without a path; tests point it at a local server.
    pub base_url: String,
    /// Tries per request, the first included.
    pub attempts: u32,
    /// The wait before the second try; it doubles for each further one.
    pub backoff: Duration,
    /// How long the API may take to answer.
    pub timeout: Duration,
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE.to_owned(),
            attempts: 4,
            backoff: Duration::from_secs(2),
            timeout: Duration::from_secs(300),
        }
    }
}

/// The HTTP client of the Messages API. [`ModelClient::complete`] blocks, so call it from a blocking thread
/// (`tokio::task::spawn_blocking`), not from async code.
#[derive(Debug)]
pub struct AnthropicClient {
    client: Client,
    handle: Handle,
    endpoint: String,
    key: ApiKey,
    attempts: u32,
    backoff: Duration,
    progress: Progress,
}

impl AnthropicClient {
    /// Builds a client that runs its requests on the runtime of `handle`.
    pub fn new(
        key: ApiKey,
        config: &AnthropicConfig,
        handle: Handle,
        progress: Progress,
    ) -> Result<Self, ModelError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("anthropic-version"),
            HeaderValue::from_static(API_VERSION),
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(HeaderName::from_static("x-api-key"), key.header()?);
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
            endpoint: format!("{}/v1/messages", config.base_url.trim_end_matches('/')),
            key,
            attempts: config.attempts.max(1),
            backoff: config.backoff,
            progress,
        })
    }

    async fn send_once(&self, request: &Request) -> Result<Response, ModelError> {
        let body = request_body(request).to_string();
        let response = self
            .client
            .post(&self.endpoint)
            .body(body)
            .send()
            .await
            .map_err(|error| ModelError::Transport(error.without_url().to_string()))?;
        let status = response.status();
        let retry_after = retry_after(response.headers());
        let text = response
            .text()
            .await
            .map_err(|error| ModelError::Transport(error.without_url().to_string()))?;
        if status.is_success() {
            parse_response(&text)
        } else {
            Err(status_error(status, &text, retry_after))
        }
    }

    async fn send_with_retries(&self, request: &Request) -> Result<Response, ModelError> {
        let mut attempt = 1;
        loop {
            match self.send_once(request).await {
                Err(error) if attempt < self.attempts && error.is_retryable() => {
                    let wait = error
                        .retry_after()
                        .unwrap_or_else(|| self.backoff.saturating_mul(1 << (attempt - 1).min(10)))
                        .min(MAX_WAIT);
                    self.progress.line(format_args!(
                        "retrying in {:.1} s after attempt {attempt} of {}: {}",
                        wait.as_secs_f64(),
                        self.attempts,
                        self.key.scrub(&error.to_string())
                    ));
                    tokio::time::sleep(wait).await;
                    attempt += 1;
                }
                other => return other,
            }
        }
    }
}

impl ModelClient for AnthropicClient {
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
                    message: self.key.scrub(&message),
                    retry_after,
                },
                ModelError::Transport(message) => ModelError::Transport(self.key.scrub(&message)),
                other => other,
            })
    }
}

/// The JSON body of a Messages API request. The system prompt is one block marked for caching, since it is the same
/// for every card.
pub fn request_body(request: &Request) -> Value {
    let messages: Vec<Value> = request
        .messages
        .iter()
        .map(|message| {
            json!({
                "role": match message.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                },
                "content": message.text,
            })
        })
        .collect();
    json!({
        "model": request.model,
        "max_tokens": request.max_tokens,
        "system": [{
            "type": "text",
            "text": request.system,
            "cache_control": {"type": "ephemeral"},
        }],
        "messages": messages,
    })
}

/// Reads a successful answer: the text blocks joined, and the tokens used (cache writes and reads count as input).
pub fn parse_response(body: &str) -> Result<Response, ModelError> {
    let value: Value =
        serde_json::from_str(body).map_err(|error| ModelError::Answer(error.to_string()))?;
    let blocks = value
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(|| ModelError::Answer("no `content` in the answer".into()))?;
    let text: String = blocks
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect();
    if text.is_empty() {
        return Err(ModelError::Answer("the answer holds no text".into()));
    }
    let usage = value.get("usage");
    let count = |key: &str| {
        usage
            .and_then(|usage| usage.get(key))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let cache_read = count("cache_read_input_tokens");
    Ok(Response {
        text,
        usage: Usage {
            input_tokens: count("input_tokens") + count("cache_creation_input_tokens") + cache_read,
            cache_read_tokens: cache_read,
            output_tokens: count("output_tokens"),
        },
    })
}

/// The error of a failed request: the API's own message when the body has one, else the start of the body.
pub fn status_error(status: StatusCode, body: &str, retry_after: Option<Duration>) -> ModelError {
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            let error = value.get("error")?;
            let kind = error.get("type").and_then(Value::as_str).unwrap_or("error");
            let message = error.get("message").and_then(Value::as_str)?;
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

#[cfg(test)]
mod tests {
    //! Goal: the request body has the shape of the Messages API with a cached system block, an answer is read into its text
    //! and token counts, failures keep the API's words, and the key is read from the environment only and never shown.
    use rstest::rstest;

    use super::*;
    use crate::propose::model::Message;

    fn request() -> Request {
        Request {
            model: "claude-sonnet-5-5".into(),
            system: "rules".into(),
            messages: vec![
                Message::user("card"),
                Message::assistant("{}"),
                Message::user("fix"),
            ],
            max_tokens: 1000,
        }
    }

    #[test]
    fn the_body_has_the_shape_of_the_messages_api() {
        let body = request_body(&request());
        assert_eq!(body["model"], "claude-sonnet-5-5");
        assert_eq!(body["max_tokens"], 1000);
        assert_eq!(body["system"][0]["text"], "rules");
        assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
        let roles: Vec<&str> = body["messages"]
            .as_array()
            .map(|all| all.iter().filter_map(|m| m["role"].as_str()).collect())
            .unwrap_or_default();
        assert_eq!(roles, ["user", "assistant", "user"]);
        assert_eq!(body["messages"][2]["content"], "fix");
    }

    #[test]
    fn an_answer_gives_its_text_and_every_kind_of_input_token() {
        let body = r#"{"id":"msg_1","type":"message","role":"assistant","stop_reason":"end_turn",
            "content":[{"type":"text","text":"{\"faces\":"},{"type":"text","text":"[]}"}],
            "usage":{"input_tokens":10,"cache_creation_input_tokens":200,"cache_read_input_tokens":3000,"output_tokens":42}}"#;
        let response = parse_response(body).unwrap();
        assert_eq!(response.text, r#"{"faces":[]}"#);
        assert_eq!(
            response.usage,
            Usage {
                input_tokens: 3210,
                cache_read_tokens: 3000,
                output_tokens: 42
            }
        );
    }

    #[rstest]
    #[case::not_json("<html>")]
    #[case::no_content(r#"{"usage":{}}"#)]
    #[case::no_text(r#"{"content":[{"type":"tool_use"}]}"#)]
    #[case::empty_text(r#"{"content":[{"type":"text","text":""}]}"#)]
    fn an_answer_without_text_is_an_error(#[case] body: &str) {
        assert!(matches!(parse_response(body), Err(ModelError::Answer(_))));
    }

    #[test]
    fn a_failure_keeps_the_apis_words_and_the_wait_it_asked_for() {
        let body = r#"{"type":"error","error":{"type":"rate_limit_error","message":"slow down"}}"#;
        let error = status_error(
            StatusCode::TOO_MANY_REQUESTS,
            body,
            Some(Duration::from_secs(7)),
        );
        assert_eq!(
            error.to_string(),
            "the API answered 429: rate_limit_error: slow down"
        );
        assert_eq!(error.retry_after(), Some(Duration::from_secs(7)));
        let plain = status_error(StatusCode::BAD_GATEWAY, "upstream gone", None);
        assert!(plain.to_string().ends_with("upstream gone"));
    }

    #[rstest]
    #[case(429, true, false)]
    #[case(500, true, false)]
    #[case(529, true, false)]
    #[case(408, true, false)]
    #[case(401, false, true)]
    #[case(403, false, true)]
    #[case(404, false, true)]
    #[case(400, false, true)]
    #[case(422, false, false)]
    fn only_overload_is_retried_and_only_a_bad_key_or_model_ends_the_run(
        #[case] status: u16,
        #[case] retry: bool,
        #[case] fatal: bool,
    ) {
        let error = ModelError::Status {
            status,
            message: String::new(),
            retry_after: None,
        };
        assert_eq!((error.is_retryable(), error.is_fatal()), (retry, fatal));
    }

    #[test]
    fn the_key_comes_from_the_environment_variable_only() {
        let key = ApiKey::from_env(|name| (name == KEY_VARIABLE).then(|| " sk-test \n".to_owned()));
        assert_eq!(key.unwrap(), ApiKey("sk-test".into()));
        assert!(matches!(
            ApiKey::from_env(|_| None),
            Err(ModelError::MissingKey)
        ));
        assert!(matches!(
            ApiKey::from_env(|_| Some("  ".into())),
            Err(ModelError::MissingKey)
        ));
        // Another variable does not stand in for it.
        assert!(
            ApiKey::from_env(|name| (name == "OPENAI_API_KEY").then(|| "x".to_owned())).is_err()
        );
    }

    #[test]
    fn the_key_is_hidden_from_debug_output_and_scrubbed_from_messages() {
        let key = ApiKey("sk-ant-SECRET".into());
        assert!(!format!("{key:?}").contains("SECRET"));
        assert_eq!(key.scrub("bad key sk-ant-SECRET."), "bad key [redacted].");
    }
}
