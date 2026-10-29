//! Goal: `propose --schema` sends the rules schema as a strict `json_schema` `response_format` to an OpenAI-style server
//! in place of the JSON-object mode, the plain switch still works without it, and the Anthropic provider says it ignores
//! it.

#![allow(clippy::unwrap_used)]

use crate::common;
use crate::propose_http::args;

use std::time::Duration;

use mtg_cli::propose::c11i_http::HttpConfig;
use mtg_cli::propose::c11i_openai::{OpenAi, OpenAiClient};
use mtg_cli::propose::model::{Message, ModelClient, Request};
use mtg_cli::propose::{W3A_SCHEMA_IGNORED, c11i_client_of};
use rstest::rstest;
use serde_json::{Value, json};

use common::progress;
use common::server::{Reply, Request as Seen, start};

const W3A_BOLT: &str = r#"{"faces":[{"abilities":[{"Spell":{"effects":[{"Draw":{}}]}}]}]}"#;

fn w3a_reply(seen: &Seen) -> Reply {
    let body = json!({
        "choices": [{"index": 0, "finish_reason": "stop", "message": {"role": "assistant", "content": W3A_BOLT}}],
        "usage": {"prompt_tokens": seen.body.len() / 4, "completion_tokens": 25},
    });
    Reply::ok(body.to_string()).header("Content-Type", "application/json")
}

/// The `response_format` of one request sent with the given switches.
async fn w3a_sent_format(json_mode: bool, schema: bool) -> Value {
    let server = start(w3a_reply).await;
    let (progress, _) = progress();
    let config = HttpConfig {
        base_url: format!("{}v1", server.url("/")),
        attempts: 1,
        backoff: Duration::from_millis(1),
        timeout: Duration::from_secs(20),
    };
    let dialect = OpenAi {
        json_mode,
        schema,
        max_answer_tokens: 1_000,
        reasoning: None,
    };
    let client = OpenAiClient::new(
        dialect,
        None,
        &config,
        tokio::runtime::Handle::current(),
        progress,
    )
    .unwrap();
    let request = Request {
        model: "gpt-oss:20b".into(),
        system: "the rules".into(),
        messages: vec![Message::user("Card: Opt")],
        max_tokens: 100,
    };
    let response = tokio::task::spawn_blocking(move || client.complete(&request))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(response.text, W3A_BOLT);
    let seen = server.log();
    assert_eq!(seen.len(), 1);
    let body: Value = serde_json::from_str(&seen[0].body_text()).unwrap();
    body["response_format"].clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_schema_is_sent_as_a_strict_json_schema_format() {
    let format = w3a_sent_format(true, true).await;
    assert_eq!(format["type"], "json_schema");
    assert_eq!(format["json_schema"]["name"], "card_rules");
    assert_eq!(format["json_schema"]["strict"], true);
    let schema = &format["json_schema"]["schema"];
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["required"], json!(["faces"]));
    assert!(schema["$defs"]["Ability"].is_object());
    assert!(schema["$defs"]["Effect"]["anyOf"].is_array());
}

#[rstest]
#[case::json_object(true, json!({"type": "json_object"}))]
#[case::no_format(false, Value::Null)]
#[tokio::test(flavor = "multi_thread")]
async fn without_the_schema_the_json_mode_switch_works_as_before(
    #[case] json_mode: bool,
    #[case] expected: Value,
) {
    assert_eq!(w3a_sent_format(json_mode, false).await, expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_anthropic_provider_says_it_ignores_the_schema() {
    let env = |name: &str| (name == "ANTHROPIC_API_KEY").then(|| "sk-test".to_owned());
    for (extra, said) in [(&["--schema"][..], true), (&[][..], false)] {
        let (progress, captured) = progress();
        let mut words = vec!["--cards", "Opt"];
        words.extend_from_slice(extra);
        c11i_client_of(&args(&words), &env, &HttpConfig::default(), progress).unwrap();
        assert_eq!(captured.text().contains(W3A_SCHEMA_IGNORED), said);
    }
}
