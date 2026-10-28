//! The OpenAI-style chat completions API (`POST {base}/chat/completions`) as a [`Dialect`], as Ollama, llama.cpp's
//! `llama-server` and similar local servers offer it: the key is optional, the system prompt comes first and unchanged so
//! a server's prefix cache is reused, and a prompt the server cut to its context window ends the run.

use reqwest::header::{AUTHORIZATION, HeaderMap};
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::c11i_http::{ApiKey, Dialect, HttpClient, HttpConfig};
use super::model::{ModelError, Request, Response, Role, Usage};
use crate::progress::Progress;

/// The environment variable that may hold a key for the server; nothing else supplies it.
pub const C11I_KEY_VARIABLE: &str = "MTG_PROPOSE_API_KEY";

/// The address of a local Ollama server's OpenAI-style API, the default of `--base-url` for this provider.
pub const C11I_DEFAULT_BASE: &str = "http://localhost:11434/v1";

/// A server that reports fewer than one in this many of the estimated prompt tokens cut the prompt. The estimate is an
/// upper one (3 bytes a token where tokenizers take about 4), so a whole prompt reads as three quarters of it.
const C11I_TRUNCATION_FACTOR: u64 = 2;

/// The chat completions API.
#[derive(Debug, Clone, Copy)]
pub struct OpenAi {
    /// Ask for a JSON object only (`response_format`); some servers reject the field.
    pub json_mode: bool,
}

/// The HTTP client of the chat completions API.
pub type OpenAiClient = HttpClient<OpenAi>;

impl OpenAiClient {
    /// Builds a client that runs its requests on the runtime of `handle`; the key, when there is one, is a Bearer header.
    pub fn new(
        dialect: OpenAi,
        key: Option<ApiKey>,
        config: &HttpConfig,
        handle: Handle,
        progress: Progress,
    ) -> Result<Self, ModelError> {
        HttpClient::c11i_build(dialect, key, config, handle, progress)
    }
}

/// The key from `MTG_PROPOSE_API_KEY` as `lookup` sees the environment, if it is set and not empty.
pub fn c11i_key_from_env(lookup: impl Fn(&str) -> Option<String>) -> Option<ApiKey> {
    ApiKey::c11i_from_variable(lookup, C11I_KEY_VARIABLE)
}

impl Dialect for OpenAi {
    fn path(&self) -> &'static str {
        "/chat/completions"
    }

    fn headers(&self, key: Option<&ApiKey>) -> Result<HeaderMap, ModelError> {
        let mut headers = HeaderMap::new();
        if let Some(key) = key {
            headers.insert(AUTHORIZATION, key.c11i_header("Bearer ")?);
        }
        Ok(headers)
    }

    fn body(&self, request: &Request) -> Value {
        c11i_request_body(request, self.json_mode)
    }

    fn parse(&self, request: &Request, body: &str) -> Result<Response, ModelError> {
        c11i_parse_response(body, request.estimated_tokens())
    }
}

/// The JSON body of a chat completions request: the system prompt is the first message, `cache_prompt` asks llama.cpp to
/// keep the shared prefix, and `json_mode` asks for a JSON object.
pub fn c11i_request_body(request: &Request, json_mode: bool) -> Value {
    let mut messages = vec![json!({"role": "system", "content": request.system})];
    messages.extend(request.messages.iter().map(|message| {
        json!({
            "role": match message.role {
                Role::User => "user",
                Role::Assistant => "assistant",
            },
            "content": message.text,
        })
    }));
    let mut body = json!({
        "model": request.model,
        "max_tokens": request.max_tokens,
        "stream": false,
        "cache_prompt": true,
        "messages": messages,
    });
    if json_mode {
        body["response_format"] = json!({"type": "json_object"});
    }
    body
}

/// Reads a successful answer: the first choice's text and the tokens used. `sent` is the estimate of the prompt tokens;
/// a server that reports far fewer cut the prompt, which is an error.
pub fn c11i_parse_response(body: &str, sent: u64) -> Result<Response, ModelError> {
    let value: Value =
        serde_json::from_str(body).map_err(|error| ModelError::Answer(error.to_string()))?;
    let message = value
        .pointer("/choices/0/message")
        .ok_or_else(|| ModelError::Answer("no `choices[0].message` in the answer".into()))?;
    let text = message
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let usage = value.get("usage");
    let count = |pointer: &str| {
        usage
            .and_then(|usage| usage.pointer(pointer))
            .and_then(Value::as_u64)
    };
    let read = count("/prompt_tokens");
    if let Some(read) = read.filter(|read| *read > 0)
        && read.saturating_mul(C11I_TRUNCATION_FACTOR) < sent
    {
        return Err(ModelError::Truncated { read, sent });
    }
    if text.trim().is_empty() {
        return Err(ModelError::Answer("the answer holds no text".into()));
    }
    let input_tokens = read.unwrap_or(sent);
    Ok(Response {
        text,
        usage: Usage {
            input_tokens,
            cache_read_tokens: count("/prompt_tokens_details/cached_tokens")
                .unwrap_or(0)
                .min(input_tokens),
            output_tokens: count("/completion_tokens").unwrap_or(0),
        },
    })
}

#[cfg(test)]
mod tests {
    //! Goal: the body of a chat completions request leads with the unchanged system prompt and carries the JSON-mode and
    //! cache fields, an answer is read into its text and token counts, and a prompt the server cut is an error.
    use rstest::rstest;

    use super::*;
    use crate::propose::model::Message;

    fn request(card: &str) -> Request {
        Request {
            model: "gpt-oss:20b".into(),
            system: "rules".into(),
            messages: vec![
                Message::user(card),
                Message::assistant("{}"),
                Message::user("fix"),
            ],
            max_tokens: 1000,
        }
    }

    #[rstest]
    #[case::json(true)]
    #[case::plain(false)]
    fn the_body_leads_with_the_system_prompt_and_carries_the_flags(#[case] json_mode: bool) {
        let body = c11i_request_body(&request("card"), json_mode);
        assert_eq!(body["model"], "gpt-oss:20b");
        assert_eq!(body["max_tokens"], 1000);
        assert_eq!(body["cache_prompt"], true);
        assert_eq!(body["stream"], false);
        let roles: Vec<&str> = body["messages"]
            .as_array()
            .map(|all| all.iter().filter_map(|m| m["role"].as_str()).collect())
            .unwrap_or_default();
        assert_eq!(roles, ["system", "user", "assistant", "user"]);
        assert_eq!(body["messages"][0]["content"], "rules");
        assert_eq!(body.get("response_format").is_some(), json_mode, "{body}");
        if json_mode {
            assert_eq!(body["response_format"]["type"], "json_object");
        }
    }

    #[test]
    fn two_cards_share_the_same_leading_bytes() {
        let first = c11i_request_body(&request("Opt"), true).to_string();
        let second = c11i_request_body(&request("Wind Drake"), true).to_string();
        let system = c11i_request_body(&request(""), true)["messages"][0].to_string();
        let shared = first
            .bytes()
            .zip(second.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        let at = first.find(&system).unwrap();
        assert!(shared >= at + system.len(), "{first}\n{second}");
    }

    #[test]
    fn an_answer_gives_its_text_and_tokens() {
        let body = r#"{"id":"c1","object":"chat.completion","choices":[{"index":0,"finish_reason":"stop",
            "message":{"role":"assistant","content":"{\"faces\":[]}"}}],
            "usage":{"prompt_tokens":3000,"completion_tokens":42,"prompt_tokens_details":{"cached_tokens":2800}}}"#;
        let response = c11i_parse_response(body, 4000).unwrap();
        assert_eq!(response.text, r#"{"faces":[]}"#);
        assert_eq!(
            response.usage,
            Usage {
                input_tokens: 3000,
                cache_read_tokens: 2800,
                output_tokens: 42
            }
        );
    }

    #[test]
    fn an_answer_without_usage_counts_the_estimate_against_the_budget() {
        let body = r#"{"choices":[{"message":{"content":"{}"}}]}"#;
        let response = c11i_parse_response(body, 500).unwrap();
        assert_eq!(response.usage.input_tokens, 500);
    }

    #[rstest]
    #[case::ollama_default_window(4096, 30_000, true)]
    #[case::sixteen_k(16_384, 34_000, true)]
    #[case::whole_prompt(23_000, 30_000, false)]
    #[case::just_above_half(15_001, 30_000, false)]
    #[case::not_reported(0, 30_000, false)]
    fn a_server_that_read_far_fewer_tokens_cut_the_prompt(
        #[case] read: u64,
        #[case] sent: u64,
        #[case] cut: bool,
    ) {
        let body = json!({
            "choices": [{"message": {"content": "{}"}}],
            "usage": {"prompt_tokens": read, "completion_tokens": 1},
        })
        .to_string();
        let result = c11i_parse_response(&body, sent);
        assert_eq!(
            matches!(result, Err(ModelError::Truncated { .. })),
            cut,
            "{result:?}"
        );
        if let Err(error) = result {
            assert!(error.is_fatal() && !error.is_retryable());
            assert!(error.to_string().contains("OLLAMA_CONTEXT_LENGTH"));
        }
    }

    #[rstest]
    #[case::not_json("<html>")]
    #[case::no_choices(r#"{"usage":{}}"#)]
    #[case::empty_text(r#"{"choices":[{"message":{"content":""}}]}"#)]
    #[case::null_text(r#"{"choices":[{"message":{"content":null}}]}"#)]
    fn an_answer_without_text_is_an_error(#[case] body: &str) {
        assert!(matches!(
            c11i_parse_response(body, 1),
            Err(ModelError::Answer(_))
        ));
    }

    #[test]
    fn the_key_is_optional_and_comes_from_its_own_variable_only() {
        let set = |name: &str| (name == C11I_KEY_VARIABLE).then(|| " local ".to_owned());
        assert!(c11i_key_from_env(set).is_some());
        assert!(c11i_key_from_env(|_| None).is_none());
        assert!(c11i_key_from_env(|_| Some(" ".into())).is_none());
        let other = |name: &str| (name == "ANTHROPIC_API_KEY").then(|| "x".to_owned());
        assert!(c11i_key_from_env(other).is_none());
    }
}
