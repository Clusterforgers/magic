//! Goal: the HTTP client of `propose` against a local server standing in for the Messages API: the request has the headers and body the API documents, 429 and 5xx answers are retried and others are not, the key goes into one header and never into an error, a line of progress or a file, and the whole command works from stored cards to saved proposals without a key for a dry run.

use crate::common;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use mtg_cli::args::{Cli, Command, ProposeArgs};
use mtg_cli::cache::Cache;
use mtg_cli::ingest::{IngestOptions, ingest};
use mtg_cli::progress::Progress;
use mtg_cli::propose::anthropic::{AnthropicClient, AnthropicConfig, ApiKey};
use mtg_cli::propose::model::{Message, ModelClient, ModelError, Request};
use mtg_cli::propose::{default_out, run_command};
use mtg_db::Database;
use serde_json::{Value, json};
use sqlx::PgPool;

use common::server::{Reply, Request as Seen, Server, start};
use common::{fixture_cache, progress};

const KEY: &str = "sk-ant-api03-SECRET-0123456789";
const BOLT: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}]}]}"#;

/// What the Messages API answers with `text` as the model's words.
fn messages_reply(text: &str) -> Reply {
    let body = json!({
        "id": "msg_1", "type": "message", "role": "assistant", "stop_reason": "end_turn",
        "content": [{"type": "text", "text": text}],
        "usage": {"input_tokens": 120, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 80, "output_tokens": 25},
    });
    Reply::ok(body.to_string()).header("Content-Type", "application/json")
}

fn error_reply(status: u16, message: &str) -> Reply {
    let body = json!({"type": "error", "error": {"type": "api_error", "message": message}});
    Reply::status(status).body(body.to_string())
}

fn config(server: &Server) -> AnthropicConfig {
    AnthropicConfig {
        base_url: server.url("/").as_str().trim_end_matches('/').to_owned(),
        attempts: 4,
        backoff: Duration::from_millis(1),
        timeout: Duration::from_secs(20),
    }
}

fn request() -> Request {
    Request {
        model: "claude-sonnet-5-5".into(),
        system: "the rules".into(),
        messages: vec![Message::user("Card: Lightning Bolt")],
        max_tokens: 100,
    }
}

/// Sends `request` from a blocking thread, as the command does.
async fn send(
    server: &Server,
    progress: Progress,
) -> Result<mtg_cli::propose::model::Response, ModelError> {
    let key = ApiKey::from_env(|_| Some(KEY.to_owned())).unwrap_or_else(|e| panic!("key: {e}"));
    let client = AnthropicClient::new(
        key,
        &config(server),
        tokio::runtime::Handle::current(),
        progress,
    )
    .unwrap_or_else(|e| panic!("client builds: {e}"));
    tokio::task::spawn_blocking(move || client.complete(&request()))
        .await
        .unwrap_or_else(|e| panic!("the blocking task runs: {e}"))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_has_the_headers_and_body_the_api_documents() {
    let server = start(|_| messages_reply(BOLT)).await;
    let (progress, _) = progress();
    let response = send(&server, progress).await.unwrap();

    assert_eq!(response.text, BOLT);
    assert_eq!(response.usage.input_tokens, 200);
    assert_eq!(response.usage.cache_read_tokens, 80);
    assert_eq!(response.usage.output_tokens, 25);
    let seen = server.log();
    assert_eq!(seen.len(), 1);
    let sent: &Seen = &seen[0];
    assert_eq!(
        (sent.method.as_str(), sent.path.as_str()),
        ("POST", "/v1/messages")
    );
    assert_eq!(sent.header("x-api-key"), Some(KEY));
    assert_eq!(sent.header("anthropic-version"), Some("2023-06-01"));
    assert_eq!(sent.header("content-type"), Some("application/json"));
    let body: Value = serde_json::from_str(&sent.body_text()).unwrap();
    assert_eq!(body["model"], "claude-sonnet-5-5");
    assert_eq!(body["max_tokens"], 100);
    assert_eq!(body["system"][0]["text"], "the rules");
    assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
    assert_eq!(body["messages"][0]["content"], "Card: Lightning Bolt");
    assert!(
        !sent.body_text().contains(KEY),
        "the key is a header, never in the body"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn overload_and_server_errors_are_retried_until_an_answer_comes() {
    let hits = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = hits.clone();
    let server = start(
        move |_| match counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
            0 => error_reply(429, "slow down").header("retry-after", "0"),
            1 => error_reply(503, "overloaded"),
            _ => messages_reply("{}"),
        },
    )
    .await;
    let (progress, captured) = progress();
    let response = send(&server, progress).await.unwrap();

    assert_eq!(response.text, "{}");
    assert_eq!(server.log().len(), 3);
    let lines = captured.text();
    assert!(lines.contains("retrying in"), "{lines}");
    assert!(lines.contains("429"), "{lines}");
    assert!(!lines.contains(KEY));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_client_gives_up_after_its_attempts_and_a_refused_key_is_not_retried() {
    let down = start(|_| error_reply(503, "overloaded")).await;
    let (progress_a, _) = progress();
    let error = send(&down, progress_a).await.unwrap_err();
    assert!(
        matches!(error, ModelError::Status { status: 503, .. }),
        "{error}"
    );
    assert_eq!(down.log().len(), 4);

    // A server that repeats the key it was given in its complaint.
    let refusing = start(|seen: &Seen| {
        let key = seen.header("x-api-key").unwrap_or_default().to_owned();
        error_reply(401, &format!("invalid x-api-key {key}"))
    })
    .await;
    let (progress_b, captured) = progress();
    let error = send(&refusing, progress_b).await.unwrap_err();
    assert_eq!(refusing.log().len(), 1);
    assert!(error.is_fatal() && !error.is_retryable());
    let shown = format!("{error} {error:?}{}", captured.text());
    assert!(!shown.contains(KEY), "the key leaked: {shown}");
    assert!(shown.contains("[redacted]"), "{shown}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_broken_connection_is_retried_as_a_transport_failure() {
    let server = start(|_| messages_reply("{}").cut_after(10)).await;
    let (progress, _) = progress();
    let error = send(&server, progress).await.unwrap_err();
    assert!(matches!(error, ModelError::Transport(_)), "{error}");
    assert_eq!(server.log().len(), 4);
}

async fn loaded(pool: PgPool, cache_dir: &Path) -> (Database, Cache) {
    let cache = fixture_cache(cache_dir);
    let db = Database::from_pool(pool);
    let (progress, _) = progress();
    let options = IngestOptions {
        force: false,
        batch_size: 500,
    };
    ingest(&cache, &db, &options, &progress)
        .await
        .unwrap_or_else(|e| panic!("ingest runs: {e}"));
    (db, cache)
}

fn args(extra: &[&str]) -> ProposeArgs {
    let mut line = vec!["mtg-cli", "propose"];
    line.extend_from_slice(extra);
    match Cli::try_parse_from(line) {
        Ok(Cli {
            command: Command::Propose(args),
            ..
        }) => args,
        other => panic!("expected propose, got {other:?}"),
    }
}

fn env_with_key(name: &str) -> Option<String> {
    (name == "ANTHROPIC_API_KEY").then(|| KEY.to_owned())
}

fn all_text_under(dir: &Path) -> String {
    let mut text = String::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in std::fs::read_dir(&next).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                text.push_str(&std::fs::read_to_string(&path).unwrap_or_default());
            }
        }
    }
    text
}

#[sqlx::test]
async fn the_command_asks_for_a_stored_card_and_keeps_the_accepted_proposal(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = loaded(pool, dir.path()).await;
    let server = start(|_| messages_reply(BOLT)).await;
    let out = dir.path().join("proposals");
    let out_arg = out.display().to_string();
    let (progress, captured) = progress();

    let summary = run_command(
        &db,
        &cache,
        &args(&[
            "--cards",
            "lightning bolt",
            "--out",
            &out_arg,
            "--examples",
            "3",
        ]),
        progress,
        &env_with_key,
        &config(&server),
    )
    .await
    .unwrap();

    assert!(
        summary.contains("Accepted Verified                1"),
        "{summary}"
    );
    assert!(summary.contains("requests 1 of 100"), "{summary}");
    let saved = std::fs::read_to_string(out.join("lightning_bolt.json")).unwrap();
    assert!(saved.contains("DealDamage"), "{saved}");
    let seen = server.log();
    assert_eq!(seen.len(), 1);
    let body: Value = serde_json::from_str(&seen[0].body_text()).unwrap();
    let user = body["messages"][0]["content"].as_str().unwrap();
    assert!(user.starts_with("Card: Lightning Bolt\n"), "{user}");
    assert!(
        user.contains("Oracle text:\nLightning Bolt deals 3 damage to any target."),
        "{user}"
    );
    assert!(
        body["system"][0]["text"]
            .as_str()
            .unwrap()
            .contains("enum Ability")
    );
    // The key reaches the server in its header and nowhere else.
    for text in [
        &summary,
        &captured.text(),
        &all_text_under(&out),
        &seen[0].body_text(),
    ] {
        assert!(!text.contains(KEY));
    }
}

#[sqlx::test]
async fn a_run_without_a_key_stops_before_any_request_and_a_dry_run_needs_none(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = loaded(pool, dir.path()).await;
    let server = start(|_| messages_reply(BOLT)).await;
    let (progress, _) = progress();

    let no_env = |_: &str| None;
    let error = run_command(
        &db,
        &cache,
        &args(&["--cards", "Lightning Bolt"]),
        progress.clone(),
        &no_env,
        &config(&server),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("ANTHROPIC_API_KEY"), "{error}");

    let text = run_command(
        &db,
        &cache,
        &args(&["--cards", "Lightning Bolt", "--dry-run"]),
        progress,
        &no_env,
        &config(&server),
    )
    .await
    .unwrap();
    assert!(text.contains("nothing is sent"), "{text}");
    assert!(text.contains("Card: Lightning Bolt"), "{text}");
    assert!(text.len() < 12_000);
    assert!(server.log().is_empty(), "nothing reached the server");
    assert!(!dir.path().join("proposals").exists());
}

#[sqlx::test]
async fn a_sample_takes_stored_cards_without_a_file_and_an_unknown_name_is_an_error(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = loaded(pool, dir.path()).await;
    let server = start(|_| messages_reply(BOLT)).await;
    let (progress, _) = progress();

    let sampled = run_command(
        &db,
        &cache,
        &args(&["--sample", "2", "--seed", "3", "--dry-run"]),
        progress.clone(),
        &env_with_key,
        &config(&server),
    )
    .await
    .unwrap();
    let first = sampled
        .lines()
        .find_map(|line| {
            line.split_once("(first: ")
                .map(|(_, rest)| rest.trim_end_matches(')').to_owned())
        })
        .unwrap();
    let library = mtg_cards::builtin::library().unwrap();
    assert!(
        library.card_by_name(&first).is_none(),
        "{first} has a hand-written file"
    );

    let error = run_command(
        &db,
        &cache,
        &args(&["--cards", "Lightning Bolt", "No Such Card"]),
        progress.clone(),
        &env_with_key,
        &config(&server),
    )
    .await
    .unwrap_err();
    assert_eq!(error.to_string(), "no stored card is named `No Such Card`");

    let error = run_command(
        &db,
        &cache,
        &args(&[]),
        progress,
        &env_with_key,
        &config(&server),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("--cards"), "{error}");
    assert!(server.log().is_empty());
}

#[test]
fn the_proposals_go_beside_the_downloads_unless_told_otherwise() {
    let cache = Cache::new("/data/mtg-cli");
    assert_eq!(
        default_out(&cache),
        std::path::PathBuf::from("/data/mtg-cli/proposals/propose")
    );
}
