//! Goal: `propose` against a fake OpenAI-style server that is slow, stalled or busy: a request that outlives
//! `--request-timeout` is cancelled and retried a bounded number of times, a card that never answers is a failure the run
//! goes past and `--resume` asks again.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use mtg_cards::{PrintedCard, PrintedFace};
use mtg_cli::progress::{Captured, Progress};
use mtg_cli::propose::budget::Stop;
use mtg_cli::propose::c11i_http::HttpConfig;
use mtg_cli::propose::c11i_openai::{OpenAi, OpenAiClient};
use mtg_cli::propose::model::{Message, ModelClient, ModelError, Request};
use mtg_cli::propose::outcome::ProposeReport;
use mtg_cli::propose::session::Options;
use mtg_cli::propose::{dry_run, propose_cards};
use mtg_cli::propose::{select, store::Store};
use rstest::rstest;
use serde_json::{Value, json};

use crate::common::server::{Reply, Request as Seen, Server, start};

const FLYING: &str = r#"{"faces":[{"abilities":[{"Keyword":"Flying"}]}]}"#;

/// What a chat completions server answers with `text`.
fn chat_reply(seen: &Seen, text: &str) -> Reply {
    let body = json!({
        "choices": [{"index": 0, "finish_reason": "stop", "message": {"role": "assistant", "content": text}}],
        "usage": {"prompt_tokens": seen.body.len() / 4, "completion_tokens": 25},
    });
    Reply::ok(body.to_string()).header("Content-Type", "application/json")
}

/// The card a request is about: the first line of its user message is `Card: <name>`.
fn card_in(seen: &Seen) -> String {
    let body: Value = serde_json::from_slice(&seen.body).unwrap_or_default();
    let message = body["messages"]
        .as_array()
        .and_then(|messages| messages.iter().find(|m| m["role"] == "user"))
        .and_then(|message| message["content"].as_str())
        .unwrap_or_default();
    message
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("Card: "))
        .unwrap_or_default()
        .to_owned()
}

fn drake(name: &str) -> PrintedCard {
    PrintedCard {
        oracle_id: format!(
            "00000000-0000-0000-0000-{:012}",
            name.bytes()
                .fold(7u64, |sum, byte| (sum * 31 + u64::from(byte))
                    % 1_000_000_000_000)
        ),
        name: name.to_owned(),
        keywords: vec!["Flying".into()],
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            mana_cost: "{2}{U}".into(),
            type_line: "Creature — Drake".into(),
            oracle_text: "Flying".into(),
            power: Some("2".into()),
            toughness: Some("2".into()),
            ..PrintedFace::default()
        }],
    }
}

fn drakes(count: usize) -> Vec<PrintedCard> {
    (0..count)
        .map(|n| drake(&format!("Drake {n:02}")))
        .collect()
}

fn options() -> Options {
    Options {
        model: "gpt-oss:20b".into(),
        examples: 4,
        resume: false,
        max_requests: 100,
        max_input_tokens: 50_000_000,
        jobs: 1,
        ..Options::default()
    }
}

fn config(server: &Server, timeout: Duration, retries: u32) -> HttpConfig {
    HttpConfig {
        base_url: format!("{}/v1", server.url("/").as_str().trim_end_matches('/')),
        attempts: retries + 1,
        backoff: Duration::from_millis(1),
        timeout,
    }
}

fn client(server: &Server, timeout: Duration, retries: u32) -> OpenAiClient {
    let dialect = OpenAi {
        json_mode: true,
        schema: false,
        max_answer_tokens: 16_384,
    };
    OpenAiClient::new(
        dialect,
        None,
        &config(server, timeout, retries),
        tokio::runtime::Handle::current(),
        Progress::buffer().0,
    )
    .unwrap_or_else(|e| panic!("client builds: {e}"))
}

fn request() -> Request {
    Request {
        model: "gpt-oss:20b".into(),
        system: "the rules ".repeat(3000),
        messages: vec![Message::user("Card: Lightning Bolt")],
        max_tokens: 100,
    }
}

/// One run over `cards` from a blocking thread, as the command does; a run that hangs fails the test.
async fn run(
    dir: &Path,
    cards: Vec<PrintedCard>,
    client: Arc<OpenAiClient>,
    options: Options,
) -> (ProposeReport, Captured) {
    let (progress, captured) = Progress::buffer();
    let out = dir.join("out");
    let task = tokio::task::spawn_blocking(move || {
        let names: Vec<String> = cards.iter().map(|card| card.name.clone()).collect();
        let targets = select::by_names(&cards, &names)?;
        propose_cards(
            &cards,
            &targets,
            &*client,
            &options,
            &Store::new(out),
            &progress,
        )
    });
    let report = tokio::time::timeout(Duration::from_secs(60), task)
        .await
        .unwrap_or_else(|_| panic!("the run hangs"))
        .unwrap_or_else(|e| panic!("the run task ends: {e}"))
        .unwrap_or_else(|e| panic!("the run succeeds: {e}"));
    (report, captured)
}

fn saved(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir.join("out"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| Path::new(name).extension().is_some_and(|ext| ext == "json"))
        .collect();
    names.sort();
    names
}

#[rstest]
#[case::no_retry(0)]
#[case::one_retry(1)]
#[case::two_retries(2)]
#[tokio::test(flavor = "multi_thread")]
async fn a_stalled_server_is_tried_once_more_per_retry_and_then_times_out(#[case] retries: u32) {
    let server = start(|seen: &Seen| chat_reply(seen, "{}").after(Duration::from_secs(60))).await;
    let client = client(&server, Duration::from_millis(300), retries);
    let started = std::time::Instant::now();
    let error = tokio::task::spawn_blocking(move || client.complete(&request()))
        .await
        .unwrap_or_else(|e| panic!("the blocking task runs: {e}"))
        .unwrap_err();

    assert!(matches!(error, ModelError::Timeout { .. }), "{error:?}");
    assert!(error.is_retryable() && !error.is_fatal());
    assert!(error.to_string().contains("--request-timeout"), "{error}");
    assert_eq!(server.log().len(), retries as usize + 1);
    assert!(started.elapsed() < Duration::from_secs(20));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_that_stalled_once_is_answered_by_the_retry() {
    let stalled = Arc::new(AtomicBool::new(true));
    let flag = stalled.clone();
    let server = start(move |seen: &Seen| {
        let reply = chat_reply(seen, "{}");
        if flag.swap(false, Ordering::SeqCst) {
            reply.after(Duration::from_secs(60))
        } else {
            reply
        }
    })
    .await;
    let client = client(&server, Duration::from_millis(300), 2);
    let response = tokio::task::spawn_blocking(move || client.complete(&request()))
        .await
        .unwrap_or_else(|e| panic!("the blocking task runs: {e}"))
        .unwrap();

    assert_eq!(response.text, "{}");
    assert_eq!(server.log().len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_card_that_times_out_is_a_failure_the_run_goes_past_and_resume_asks_again() {
    let dir = tempfile::tempdir().unwrap();
    let stalled = Arc::new(AtomicBool::new(true));
    let flag = stalled.clone();
    let server = start(move |seen: &Seen| {
        let reply = chat_reply(seen, FLYING);
        if flag.load(Ordering::SeqCst) && card_in(seen) == "Drake 00" {
            reply.after(Duration::from_secs(60))
        } else {
            reply
        }
    })
    .await;
    let client = Arc::new(client(&server, Duration::from_millis(300), 0));

    let (report, log) = run(dir.path(), drakes(3), client.clone(), options()).await;
    assert_eq!((report.tally.failed, report.tally.verified), (1, 2));
    assert_eq!(report.tally.not_run, 0);
    assert_eq!(saved(dir.path()), ["drake_01.json", "drake_02.json"]);
    let text = log.text();
    assert!(
        text.contains("Drake 00: no answer") && text.contains("--request-timeout"),
        "{text}"
    );

    stalled.store(false, Ordering::SeqCst);
    let before = server.log().len();
    let resume = Options {
        resume: true,
        ..options()
    };
    let (report, _) = run(dir.path(), drakes(3), client, resume).await;
    assert_eq!(
        (
            report.tally.skipped,
            report.tally.verified,
            report.tally.failed
        ),
        (2, 1, 0)
    );
    let asked: Vec<String> = server.log()[before..].iter().map(card_in).collect();
    assert_eq!(asked, ["Drake 00"]);
    assert_eq!(saved(dir.path()).len(), 3);
}

/// A server that answers every card with `FLYING` after `wait`.
async fn slow_server(wait: Duration) -> Server {
    start(move |seen: &Seen| chat_reply(seen, FLYING).after(wait)).await
}

/// Every saved file of a run by name, with its text.
fn contents(dir: &Path) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for name in saved(dir) {
        let text = std::fs::read_to_string(dir.join("out").join(&name)).unwrap_or_default();
        found.push((name, text));
    }
    found
}

#[rstest]
#[case::one_at_a_time(1)]
#[case::two(2)]
#[case::four(4)]
#[tokio::test(flavor = "multi_thread")]
async fn jobs_cards_are_in_flight_at_once_and_never_more(#[case] jobs: u32) {
    let dir = tempfile::tempdir().unwrap();
    let server = slow_server(Duration::from_millis(250)).await;
    let client = Arc::new(client(&server, Duration::from_secs(20), 0));
    let options = Options { jobs, ..options() };
    let (report, _) = run(dir.path(), drakes(8), client, options).await;

    assert_eq!(report.tally.verified, 8);
    assert_eq!(server.log().len(), 8);
    assert_eq!(server.peak_in_flight(), jobs as usize);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_files_of_a_parallel_run_are_those_of_a_sequential_one() {
    let (one, four) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    for (dir, jobs) in [(one.path(), 1), (four.path(), 4)] {
        let server = slow_server(Duration::from_millis(20)).await;
        let client = Arc::new(client(&server, Duration::from_secs(20), 0));
        let options = Options { jobs, ..options() };
        run(dir, drakes(10), client, options).await;
    }
    let (sequential, parallel) = (contents(one.path()), contents(four.path()));
    assert_eq!(sequential.len(), 10);
    assert_eq!(sequential, parallel);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_request_budget_is_shared_and_stops_the_run_exactly_at_the_limit() {
    let dir = tempfile::tempdir().unwrap();
    let server = slow_server(Duration::from_millis(50)).await;
    let client = Arc::new(client(&server, Duration::from_secs(20), 0));
    let options = Options {
        jobs: 4,
        max_requests: 5,
        ..options()
    };
    let (report, log) = run(dir.path(), drakes(12), client, options).await;

    assert_eq!(server.log().len(), 5, "the budget is never passed");
    assert_eq!((report.tally.verified, report.tally.not_run), (5, 7));
    assert_eq!(report.budget.requests(), 5);
    assert_eq!(report.stopped, Some(Stop::Requests(5)));
    assert!(log.text().contains("5/12 cards done"), "{}", log.text());
}

#[tokio::test(flavor = "multi_thread")]
async fn requests_in_flight_hold_their_tokens_against_the_input_budget() {
    let dir = tempfile::tempdir().unwrap();
    let server = slow_server(Duration::from_millis(300)).await;
    let client = Arc::new(client(&server, Duration::from_secs(20), 0));
    let cards = drakes(8);
    let text = dry_run(&cards, &[0], &options()).unwrap();
    let estimate: u64 = text
        .lines()
        .find_map(|line| line.strip_prefix("estimated input tokens of the first request: "))
        .and_then(|rest| rest.split(' ').next())
        .and_then(|digits| digits.parse().ok())
        .unwrap_or_else(|| panic!("the dry run states the estimate:\n{text}"));
    // Room for two requests by their estimates; all four workers ask at the same moment.
    let options = Options {
        jobs: 4,
        max_input_tokens: estimate * 2 + estimate / 4,
        ..options()
    };
    let (report, _) = run(dir.path(), cards, client, options.clone()).await;

    assert_eq!(
        server.log().len(),
        2,
        "two were in flight when the budget was reached"
    );
    assert_eq!((report.tally.verified, report.tally.not_run), (2, 6));
    assert!(report.budget.usage().input_tokens <= options.max_input_tokens);
    assert_eq!(
        report.stopped,
        Some(Stop::InputTokens(options.max_input_tokens))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_status_line_comes_every_interval_and_at_the_end() {
    let dir = tempfile::tempdir().unwrap();
    let server = slow_server(Duration::from_millis(500)).await;
    let client = Arc::new(client(&server, Duration::from_secs(20), 0));
    let options = Options {
        status_every: Duration::from_millis(100),
        ..options()
    };
    let (_, log) = run(dir.path(), drakes(2), client, options).await;

    let text = log.text();
    let lines: Vec<&str> = text.lines().filter(|l| l.starts_with("status: ")).collect();
    assert!(lines.len() >= 4, "{text}");
    assert!(
        lines[0].contains("0/2 cards done") && lines[0].contains("ETA unknown"),
        "{text}"
    );
    let last = lines[lines.len() - 1];
    assert!(
        last.contains("2/2 cards done, 0 failed; Verified 2,"),
        "{last}"
    );
    assert!(
        last.contains("2 requests") && last.contains("tok/s") && last.contains("cards/min"),
        "{last}"
    );
}
