//! The Anthropic Messages API as a [`Dialect`]: the key comes from `ANTHROPIC_API_KEY` and goes into one header only,
//! and the system prompt is marked for caching.

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{Value, json};
use tokio::runtime::Handle;

pub use super::c11i_http::{ApiKey, DEFAULT_BASE, status_error};
use super::c11i_http::{Dialect, HttpClient, HttpConfig};
use super::model::{ModelError, Request, Response, Role, Usage};
use crate::progress::Progress;

/// The environment variable that holds the API key; nothing else supplies it.
pub const KEY_VARIABLE: &str = "ANTHROPIC_API_KEY";

/// The API version header value.
const API_VERSION: &str = "2023-06-01";

/// How the client reaches the API; the default is the Anthropic API itself.
pub type AnthropicConfig = HttpConfig;

/// The HTTP client of the Messages API.
pub type AnthropicClient = HttpClient<Anthropic>;

impl ApiKey {
    /// The key from `ANTHROPIC_API_KEY` as `lookup` sees the environment; empty counts as missing.
    pub fn from_env(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ModelError> {
        ApiKey::c11i_from_variable(lookup, KEY_VARIABLE).ok_or(ModelError::MissingKey)
    }
}

/// The Messages API.
#[derive(Debug, Clone, Copy, Default)]
pub struct Anthropic;

impl Dialect for Anthropic {
    fn path(&self) -> &'static str {
        "/v1/messages"
    }

    fn headers(&self, key: Option<&ApiKey>) -> Result<HeaderMap, ModelError> {
        let key = key.ok_or(ModelError::MissingKey)?;
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("anthropic-version"),
            HeaderValue::from_static(API_VERSION),
        );
        headers.insert(HeaderName::from_static("x-api-key"), key.c11i_header("")?);
        Ok(headers)
    }

    fn body(&self, request: &Request) -> Value {
        request_body(request)
    }

    fn parse(&self, _request: &Request, body: &str) -> Result<Response, ModelError> {
        parse_response(body)
    }
}

impl AnthropicClient {
    /// Builds a client that runs its requests on the runtime of `handle`.
    pub fn new(
        key: ApiKey,
        config: &AnthropicConfig,
        handle: Handle,
        progress: Progress,
    ) -> Result<Self, ModelError> {
        HttpClient::c11i_build(Anthropic, Some(key), config, handle, progress)
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

#[cfg(test)]
mod tests {
    //! Goal: the request body has the shape of the Messages API with a cached system block, an answer is read into its text
    //! and token counts, failures keep the API's words, and the key is read from the environment only and never shown.
    use std::time::Duration;

    use reqwest::StatusCode;
    use rstest::rstest;

    use super::*;

    fn key(text: &str) -> ApiKey {
        ApiKey::c11i_from_variable(|_| Some(text.to_owned()), "ANY").unwrap()
    }
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
        assert_eq!(key.unwrap(), self::key("sk-test"));
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
        let key = self::key("sk-ant-SECRET");
        assert!(!format!("{key:?}").contains("SECRET"));
        assert_eq!(key.scrub("bad key sk-ant-SECRET."), "bad key [redacted].");
    }
}
