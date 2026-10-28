//! Goal: the OpenAI-style client of `propose` against a local server standing in for Ollama or llama.cpp: the request
//! leads with the system prompt and carries the cache and JSON-mode fields, the key is an optional Bearer header that never
//! shows, 5xx answers are retried, and a refused request, a cut prompt and an unreachable server end the run clearly.

use crate::common;
use crate::propose_http::{args, loaded};

use std::time::Duration;

use mtg_cli::progress::Progress;
use mtg_cli::propose::c11i_http::{ApiKey, HttpConfig};
use mtg_cli::propose::c11i_openai::{C11I_KEY_VARIABLE, OpenAi, OpenAiClient};
use mtg_cli::propose::model::{Message, ModelClient, ModelError, Request, Response};
use mtg_cli::propose::run_command;
use serde_json::{Value, json};
use sqlx::PgPool;

use common::progress;
use common::server::{Reply, Request as Seen, Server, bind, start};

const KEY: &str = "local-SECRET-0123456789";
const BOLT: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}]}]}"#;

/// What a chat completions server answers with `text`, having read `prompt_tokens` of the prompt.
fn chat_reply(text: &str, prompt_tokens: u64) -> Reply {
    let body = json!({
        "id": "chatcmpl-1", "object": "chat.completion", "model": "gpt-oss:20b",
        "choices": [{"index": 0, "finish_reason": "stop", "message": {"role": "assistant", "content": text}}],
        "usage": {"prompt_tokens": prompt_tokens, "completion_tokens": 25, "total_tokens": prompt_tokens + 25},
    });
    Reply::ok(body.to_string()).header("Content-Type", "application/json")
}

/// A reply that read the whole prompt: about 4 bytes a token, as tokenizers take.
fn whole_reply(seen: &Seen, text: &str) -> Reply {
    chat_reply(text, (seen.body.len() / 4) as u64)
}

fn error_reply(status: u16, message: &str) -> Reply {
    let body = json!({"error": {"type": "api_error", "message": message}});
    Reply::status(status).body(body.to_string())
}

fn config(base: &str) -> HttpConfig {
    HttpConfig {
        base_url: format!("{}/v1", base.trim_end_matches('/')),
        attempts: 4,
        backoff: Duration::from_millis(1),
        timeout: Duration::from_secs(20),
    }
}

fn request() -> Request {
    Request {
        model: "gpt-oss:20b".into(),
        system: "the rules ".repeat(3000),
        messages: vec![Message::user("Card: Lightning Bolt")],
        max_tokens: 100,
    }
}

/// Sends `request` from a blocking thread, as the command does.
async fn send(
    base: &str,
    json_mode: bool,
    key: Option<&str>,
    progress: Progress,
) -> Result<Response, ModelError> {
    let key = key.and_then(|key| ApiKey::c11i_from_variable(|_| Some(key.to_owned()), "ANY"));
    let client = OpenAiClient::new(
        OpenAi {
            json_mode,
            schema: false,
            max_answer_tokens: 16_384,
        },
        key,
        &config(base),
        tokio::runtime::Handle::current(),
        progress,
    )
    .unwrap_or_else(|e| panic!("client builds: {e}"));
    tokio::task::spawn_blocking(move || client.complete(&request()))
        .await
        .unwrap_or_else(|e| panic!("the blocking task runs: {e}"))
}

fn base(server: &Server) -> String {
    server.url("/").as_str().trim_end_matches('/').to_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_leads_with_the_system_prompt_and_has_no_key_header_without_a_key() {
    let server = start(|seen: &Seen| whole_reply(seen, BOLT)).await;
    let (progress, _) = progress();
    let response = send(&base(&server), true, None, progress).await.unwrap();

    assert_eq!(response.text, BOLT);
    assert_eq!(response.usage.output_tokens, 25);
    let seen = server.log();
    assert_eq!(seen.len(), 1);
    assert_eq!(
        (seen[0].method.as_str(), seen[0].path.as_str()),
        ("POST", "/v1/chat/completions")
    );
    assert_eq!(seen[0].header("authorization"), None);
    assert_eq!(seen[0].header("content-type"), Some("application/json"));
    let body: Value = serde_json::from_str(&seen[0].body_text()).unwrap();
    assert_eq!(body["model"], "gpt-oss:20b");
    assert_eq!(body["max_tokens"], 16_384);
    assert_eq!(body["cache_prompt"], true);
    assert_eq!(body["response_format"]["type"], "json_object");
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], request().system.as_str());
    assert_eq!(body["messages"][1]["content"], "Card: Lightning Bolt");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_key_is_a_bearer_header_and_json_mode_can_be_switched_off() {
    let server = start(|seen: &Seen| whole_reply(seen, "{}")).await;
    let (progress, _) = progress();
    send(&base(&server), false, Some(KEY), progress)
        .await
        .unwrap();

    let seen = server.log();
    let expected = format!("Bearer {KEY}");
    assert_eq!(seen[0].header("authorization"), Some(expected.as_str()));
    let body: Value = serde_json::from_str(&seen[0].body_text()).unwrap();
    assert!(body.get("response_format").is_none(), "{body}");
    assert!(!seen[0].body_text().contains(KEY));
}

#[tokio::test(flavor = "multi_thread")]
async fn server_errors_are_retried_and_a_refused_request_is_not() {
    let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = hits.clone();
    let flaky =
        start(
            move |seen: &Seen| match counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
                0 => error_reply(503, "loading model"),
                _ => whole_reply(seen, "{}"),
            },
        )
        .await;
    let (progress_a, captured) = progress();
    send(&base(&flaky), true, None, progress_a).await.unwrap();
    assert_eq!(flaky.log().len(), 2);
    assert!(captured.text().contains("retrying in"));

    // A server that repeats the key it was given in its complaint.
    let refusing = start(|seen: &Seen| {
        let header = seen.header("authorization").unwrap_or_default().to_owned();
        error_reply(401, &format!("bad token {header}"))
    })
    .await;
    let (progress_b, captured) = progress();
    let error = send(&base(&refusing), true, Some(KEY), progress_b)
        .await
        .unwrap_err();
    assert_eq!(refusing.log().len(), 1);
    assert!(error.is_fatal() && !error.is_retryable());
    let shown = format!("{error} {error:?}{}", captured.text());
    assert!(!shown.contains(KEY), "the key leaked: {shown}");
    assert!(shown.contains("[redacted]"), "{shown}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_that_cut_the_prompt_is_a_fatal_error_and_is_not_asked_again() {
    let server = start(|_| chat_reply("{}", 4096)).await;
    let (progress, _) = progress();
    let error = send(&base(&server), true, None, progress)
        .await
        .unwrap_err();
    assert!(
        matches!(error, ModelError::Truncated { read: 4096, .. }),
        "{error}"
    );
    assert!(error.is_fatal());
    assert!(
        error.to_string().contains("OLLAMA_CONTEXT_LENGTH"),
        "{error}"
    );
    assert_eq!(server.log().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unreachable_server_is_named_after_the_retries() {
    let bound = bind().await;
    let address = bound.base();
    drop(bound);
    let (progress, _) = progress();
    let error = send(&address, true, None, progress).await.unwrap_err();
    assert!(matches!(error, ModelError::Unreachable { .. }), "{error:?}");
    assert!(error.is_fatal());
    let text = error.to_string();
    assert!(
        text.contains(&address) && text.contains("is it running"),
        "{text}"
    );
}

#[sqlx::test]
async fn the_command_asks_a_local_server_without_a_key_and_stops_on_a_cut_prompt(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = loaded(pool, dir.path()).await;
    let server = start(|seen: &Seen| whole_reply(seen, BOLT)).await;
    let url = format!("{}/v1", base(&server));
    let out = dir.path().join("proposals");
    let out_arg = out.display().to_string();
    let no_env = |_: &str| None;
    let line = |model: &[&str]| {
        let mut all = vec![
            "--cards",
            "lightning bolt",
            "--out",
            &out_arg,
            "--provider",
            "openai",
            "--base-url",
            &url,
        ];
        all.extend_from_slice(model);
        args(&all)
    };
    let (progress, _) = progress();
    let api = HttpConfig::default();

    let error = run_command(&db, &cache, &line(&[]), progress.clone(), &no_env, &api)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("--model"), "{error}");
    assert!(server.log().is_empty());

    let summary = run_command(
        &db,
        &cache,
        &line(&["--model", "gpt-oss:20b"]),
        progress.clone(),
        &no_env,
        &api,
    )
    .await
    .unwrap();
    assert!(
        summary.contains("Accepted Verified                1"),
        "{summary}"
    );
    assert!(out.join("lightning_bolt.json").exists());
    let seen = server.log();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].header("authorization"), None);
    let body: Value = serde_json::from_str(&seen[0].body_text()).unwrap();
    assert!(
        body["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("enum Ability")
    );

    let cutting = start(|_| chat_reply(BOLT, 2048)).await;
    let cut_url = format!("{}/v1", base(&cutting));
    let with_key = |name: &str| (name == C11I_KEY_VARIABLE).then(|| KEY.to_owned());
    let error = run_command(
        &db,
        &cache,
        &args(&[
            "--cards",
            "Lightning Bolt",
            "--provider",
            "openai",
            "--base-url",
            &cut_url,
            "--model",
            "m",
            "--out",
            &out_arg,
        ]),
        progress,
        &with_key,
        &api,
    )
    .await
    .unwrap_err();
    let text = error.to_string();
    assert!(text.contains("raise the context"), "{text}");
    assert!(!text.contains(KEY));
    assert_eq!(cutting.log().len(), 1);
    let expected = format!("Bearer {KEY}");
    assert_eq!(
        cutting.log()[0].header("authorization"),
        Some(expected.as_str())
    );
}
