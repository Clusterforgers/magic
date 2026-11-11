//! Goal: a paid `propose` run is capped and accounted against a mock Messages API and Batches API: the spend cap stops the
//! run before the request that would pass it, every billed request lands in the ledger and counts toward the next run,
//! a batch is split to fit the cap, settled once and resumed after a stop, and the key appears in no output.

#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mtg_cards::{PrintedCard, PrintedFace};
use mtg_cli::error::CliError;
use mtg_cli::progress::{Captured, Progress};
use mtg_cli::propose::anthropic::{AnthropicClient, AnthropicConfig, ApiKey};
use mtg_cli::propose::batch::{BatchApi, BatchRun};
use mtg_cli::propose::budget::Stop;
use mtg_cli::propose::ledger::Ledger;
use mtg_cli::propose::outcome::ProposeReport;
use mtg_cli::propose::price::{Billing, Usd, price_of};
use mtg_cli::propose::prompt::first_request;
use mtg_cli::propose::session::Options;
use mtg_cli::propose::spend::Spend;
use mtg_cli::propose::store::Store;
use mtg_cli::propose::{context, propose_cards, select};
use serde_json::{Value, json};

use crate::common::server::{Reply, Request as Seen, Server, start};
use crate::propose_http::{args, loaded};
use mtg_cli::args::ProposeOrderArgs;
use mtg_cli::propose::run_command;
use mtg_db::Database;
use sqlx::PgPool;

const KEY: &str = "sk-ant-api03-SECRET-ap23-0123456789";
const MODEL: &str = "claude-sonnet-5-5";
const FLYING: &str = r#"{"faces":[{"abilities":[{"Keyword":"Flying"}]}]}"#;
const KORMUS_BELL: &str = r#"{"faces":[{"abilities":[{"Static":{"affected":{"Each":[{"Subtype":"Swamp"}]},"mods":[{"AddType":"Creature"},{"SetColors":["Black"]},{"SetPT":{"power":1,"toughness":1}}]}}]}]}"#;

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

fn kormus_bell() -> PrintedCard {
    PrintedCard {
        oracle_id: "d2d477a5-f055-4d47-a852-26e8239f2a01".into(),
        name: "Kormus Bell".into(),
        keywords: Vec::new(),
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: "Kormus Bell".into(),
            mana_cost: "{4}".into(),
            type_line: "Artifact".into(),
            oracle_text: "All Swamps are 1/1 black creatures that are still lands.".into(),
            ..PrintedFace::default()
        }],
    }
}

/// The upper estimate of the input tokens of a request body (`system` and message texts, 3 bytes a token).
fn estimated_tokens(params: &Value) -> u64 {
    let system = params["system"][0]["text"].as_str().map_or(0, str::len);
    let messages: usize = params["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|message| message["content"].as_str())
        .map(str::len)
        .sum();
    u64::try_from((system + messages).div_ceil(3)).unwrap_or(u64::MAX)
}

/// The card a request body is about.
fn card_of(params: &Value) -> String {
    params["messages"][0]["content"]
        .as_str()
        .and_then(|text| text.lines().next())
        .and_then(|line| line.strip_prefix("Card: "))
        .unwrap_or_default()
        .to_owned()
}

/// A usage that costs as much as the request's worst case: every input token written to the cache, every answer token used.
fn worst_usage(params: &Value) -> Value {
    json!({
        "input_tokens": 0,
        "cache_creation_input_tokens": estimated_tokens(params),
        "cache_read_input_tokens": 0,
        "output_tokens": 8192,
    })
}

fn message(text: &str, usage: &Value) -> Value {
    json!({
        "id": "msg_1", "type": "message", "role": "assistant", "stop_reason": "end_turn",
        "content": [{"type": "text", "text": text}],
        "usage": usage,
    })
}

fn messages_reply(text: &str, usage: &Value) -> Reply {
    Reply::ok(message(text, usage).to_string()).header("Content-Type", "application/json")
}

fn config(server: &Server, timeout: Duration) -> AnthropicConfig {
    AnthropicConfig {
        base_url: server.url("/").as_str().trim_end_matches('/').to_owned(),
        attempts: 1,
        backoff: Duration::from_millis(1),
        timeout,
    }
}

fn spend(dir: &Path, cap: Usd, billing: Billing) -> Spend {
    Spend::new(Ledger::at(dir.join("api-ledger.tsv")), cap, MODEL, billing).unwrap()
}

fn options(spend: Spend) -> Options {
    Options {
        model: MODEL.into(),
        examples: 4,
        max_requests: 1000,
        max_input_tokens: 500_000_000,
        jobs: 1,
        spend: Some(spend),
        warm_cache: true,
        poll_every: Duration::from_millis(1),
        ..Options::default()
    }
}

/// The worst case of the first request about the first card, at the price of a request or of a batch.
fn worst_of(cards: &[PrintedCard], billing: Billing) -> Usd {
    let ctx = context(cards).unwrap();
    let text = ctx.card_message(&cards[0], 4).text;
    let request = first_request(MODEL, &ctx.system, text);
    price_of(MODEL).unwrap().worst_case(&request, billing)
}

fn client(server: &Server, timeout: Duration, progress: Progress) -> AnthropicClient {
    let key = ApiKey::from_env(|_| Some(KEY.to_owned())).unwrap_or_else(|e| panic!("key: {e}"));
    AnthropicClient::new(
        key,
        &config(server, timeout),
        tokio::runtime::Handle::current(),
        progress,
    )
    .unwrap_or_else(|e| panic!("client builds: {e}"))
}

/// One run over all `cards` from a blocking thread, as the command does.
async fn run_sync(
    dir: &Path,
    cards: Vec<PrintedCard>,
    server: &Server,
    timeout: Duration,
    options: Options,
) -> (Result<ProposeReport, CliError>, Captured) {
    let (progress, captured) = Progress::buffer();
    let client = client(server, timeout, progress.clone());
    let out = dir.join("out");
    let task = tokio::task::spawn_blocking(move || {
        let names: Vec<String> = cards.iter().map(|card| card.name.clone()).collect();
        let targets = select::by_names(&cards, &names)?;
        propose_cards(
            &cards,
            &targets,
            &client,
            &options,
            &Store::new(out),
            &progress,
        )
    });
    let result = tokio::time::timeout(Duration::from_mins(2), task)
        .await
        .unwrap_or_else(|_| panic!("the run hangs"))
        .unwrap_or_else(|e| panic!("the run task ends: {e}"));
    (result, captured)
}

/// One batch run over all `cards`.
async fn run_batch(
    dir: &Path,
    cards: Vec<PrintedCard>,
    server: &Server,
    options: Options,
) -> (Result<ProposeReport, CliError>, Captured) {
    let (progress, captured) = Progress::buffer();
    let api = BatchApi::new(
        client(server, Duration::from_secs(30), progress.clone()),
        None,
    );
    let out = dir.join("out");
    let task = tokio::task::spawn_blocking(move || {
        let ctx = context(&cards)?;
        let names: Vec<String> = cards.iter().map(|card| card.name.clone()).collect();
        let targets = select::by_names(&cards, &names)?;
        BatchRun {
            ctx: &ctx,
            api: &api,
            options: &options,
            store: &Store::new(out),
            progress: &progress,
        }
        .run(&cards, &targets)
    });
    let result = tokio::time::timeout(Duration::from_mins(5), task)
        .await
        .unwrap_or_else(|_| panic!("the batch run hangs"))
        .unwrap_or_else(|e| panic!("the run task ends: {e}"));
    (result, captured)
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

fn ledger_lines(dir: &Path) -> Vec<Vec<String>> {
    std::fs::read_to_string(dir.join("api-ledger.tsv"))
        .unwrap_or_default()
        .lines()
        .skip(1)
        .map(|line| line.split('\t').map(str::to_owned).collect())
        .collect()
}

// ---- the spend cap and the ledger of single requests ----

#[tokio::test(flavor = "multi_thread")]
async fn the_cap_stops_the_run_before_the_request_that_would_pass_it() {
    let dir = tempfile::tempdir().unwrap();
    let cards = drakes(6);
    let worst = worst_of(&cards, Billing::Standard);
    // Each answer is billed at its worst case, so a cap of two and a half of them admits two requests.
    let cap = Usd(worst.0 * 5 / 2);
    let server = start(|seen: &Seen| {
        let params: Value = serde_json::from_slice(&seen.body).unwrap_or_default();
        messages_reply(FLYING, &worst_usage(&params))
    })
    .await;
    let (result, captured) = run_sync(
        dir.path(),
        cards,
        &server,
        Duration::from_secs(30),
        options(spend(dir.path(), cap, Billing::Standard)),
    )
    .await;
    let report = result.unwrap();

    assert_eq!(
        server.log().len(),
        2,
        "the third request would pass the cap"
    );
    assert!(
        matches!(report.stopped, Some(Stop::Spend { .. })),
        "{:?}",
        report.stopped
    );
    assert_eq!((report.tally.verified, report.tally.not_run), (2, 4));
    let ledger = Ledger::at(dir.path().join("api-ledger.tsv"));
    let total = u64::try_from(ledger.total().unwrap()).unwrap();
    assert!(total <= cap.0, "{total} passes the cap {cap}");
    assert!(
        total >= worst.0 * 2 - 4,
        "two requests cost about two worst cases: {total}"
    );
    let spend = report.spend.unwrap();
    assert_eq!(
        (spend.run, spend.ledger, spend.cap),
        (Usd(total), Usd(total), cap)
    );
    let text = report.render();
    assert!(text.contains("stopped: the spend cap of $"), "{text}");
    assert!(text.contains("spent $"), "{text}");
    assert!(captured.text().contains("4 not run"), "{}", captured.text());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_ledger_holds_each_billed_request_and_counts_toward_the_next_run() {
    let dir = tempfile::tempdir().unwrap();
    let cards = drakes(2);
    let worst = worst_of(&cards, Billing::Standard);
    let usage = json!({"input_tokens": 100, "cache_creation_input_tokens": 200, "cache_read_input_tokens": 700, "output_tokens": 30});
    let server = start(move |_: &Seen| messages_reply(FLYING, &usage)).await;
    let (first, _) = run_sync(
        dir.path(),
        cards.clone(),
        &server,
        Duration::from_secs(30),
        options(spend(dir.path(), Usd(50_000_000), Billing::Standard)),
    )
    .await;
    first.unwrap();

    // $2 per million new tokens, $2.50 written, $0.20 read, $10 answered: 200 + 500 + 140 + 300 micro-dollars.
    let lines = ledger_lines(dir.path());
    assert_eq!(lines.len(), 2);
    assert_eq!(
        &lines[0][1..],
        ["Drake 00", MODEL, "100", "200", "700", "30", "0.001140"]
    );
    assert!(lines[0][0].ends_with('Z'), "{:?}", lines[0]);
    let header = std::fs::read_to_string(dir.path().join("api-ledger.tsv")).unwrap();
    assert!(header.starts_with("time\tcard\tmodel\tinput_tokens\tcache_write_tokens\tcache_read_tokens\toutput_tokens\tusd\n"));

    // A later run, a new process as far as the ledger knows, starts from what is spent: 2 280 micro-dollars of the cap.
    let before = server.log().len();
    let cap = Usd(2_280 + worst.0 / 2);
    let (second, _) = run_sync(
        dir.path(),
        drakes(3)[2..].to_vec(),
        &server,
        Duration::from_secs(30),
        options(spend(dir.path(), cap, Billing::Standard)),
    )
    .await;
    let report = second.unwrap();
    assert_eq!(
        server.log().len(),
        before,
        "nothing is sent that the ledger leaves no room for"
    );
    let Some(Stop::Spend { spent, next, .. }) = report.stopped else {
        panic!("the cap stops the run: {:?}", report.stopped);
    };
    assert_eq!((spent, next), (Usd(2_280), worst));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_that_never_answers_is_counted_at_its_worst_case_and_an_overload_is_not() {
    let dir = tempfile::tempdir().unwrap();
    let cards = drakes(2);
    let worst = worst_of(&cards, Billing::Standard);
    let server = start(|seen: &Seen| {
        let params: Value = serde_json::from_slice(&seen.body).unwrap_or_default();
        if card_of(&params) == "Drake 00" {
            Reply::status(529)
                .body(r#"{"type":"error","error":{"type":"overloaded_error","message":"busy"}}"#)
        } else {
            messages_reply(FLYING, &json!({})).after(Duration::from_secs(3))
        }
    })
    .await;
    let (result, _) = run_sync(
        dir.path(),
        cards,
        &server,
        Duration::from_millis(300),
        options(spend(dir.path(), Usd(50_000_000), Billing::Standard)),
    )
    .await;
    let report = result.unwrap();

    assert_eq!(report.tally.failed, 2);
    let lines = ledger_lines(dir.path());
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0][1], "Drake 01 (no answer, may be billed)");
    assert_eq!(lines[0][7], worst.plain());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_answer_without_text_is_billed_and_the_key_stays_out_of_every_output() {
    let dir = tempfile::tempdir().unwrap();
    let cards = drakes(1);
    let server = start(|seen: &Seen| {
        // The thinking used every answer token; the server also repeats the key it was sent, as a careless one might.
        let key = seen.header("x-api-key").unwrap_or_default().to_owned();
        let body = json!({
            "stop_reason": "max_tokens",
            "content": [{"type": "thinking", "thinking": format!("secret {key}")}],
            "usage": {"input_tokens": 1000, "output_tokens": 8192},
        });
        Reply::ok(body.to_string()).header("Content-Type", "application/json")
    })
    .await;
    let (result, captured) = run_sync(
        dir.path(),
        cards,
        &server,
        Duration::from_secs(30),
        options(spend(dir.path(), Usd(50_000_000), Billing::Standard)),
    )
    .await;
    let report = result.unwrap();

    assert_eq!(report.tally.failed, 1);
    let lines = ledger_lines(dir.path());
    assert_eq!(lines.len(), 1);
    // 1000 new tokens $0.002 and 8192 answer tokens $0.08192.
    assert_eq!(lines[0][7], "0.083920");
    let out = all_text_under(dir.path());
    for text in [&out, &captured.text(), &report.render()] {
        assert!(!text.contains(KEY), "the key leaks:\n{text}");
    }
    assert!(
        out.contains("--reasoning low"),
        "the summary says what to try: {out}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_key_ends_the_run_without_the_key_in_the_error_a_line_or_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let server = start(|seen: &Seen| {
        let key = seen.header("x-api-key").unwrap_or_default().to_owned();
        let body = json!({"type": "error", "error": {"type": "authentication_error", "message": format!("invalid x-api-key {key}")}});
        Reply::status(401).body(body.to_string())
    })
    .await;
    let (result, captured) = run_sync(
        dir.path(),
        drakes(2),
        &server,
        Duration::from_secs(30),
        options(spend(dir.path(), Usd(50_000_000), Billing::Standard)),
    )
    .await;
    let error = result.unwrap_err();

    for text in [
        error.to_string(),
        format!("{error:?}"),
        captured.text(),
        all_text_under(dir.path()),
    ] {
        assert!(!text.contains(KEY), "the key leaks:\n{text}");
    }
    assert!(error.to_string().contains("401"), "{error}");
    assert!(
        ledger_lines(dir.path()).is_empty(),
        "a refused request costs nothing"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_first_request_goes_alone_so_the_others_read_the_rules_from_the_cache() {
    let dir = tempfile::tempdir().unwrap();
    // The cache entry exists once the first request is answered, 200 ms after it arrived; a request that arrives before
    // that writes an entry of its own, as the API does.
    let first_arrival = Arc::new(Mutex::new(None::<std::time::Instant>));
    let clock = first_arrival.clone();
    let server = start(move |seen: &Seen| {
        let params: Value = serde_json::from_slice(&seen.body).unwrap_or_default();
        let tokens = estimated_tokens(&params);
        let started = *clock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_or_insert_with(std::time::Instant::now);
        let usage = if started.elapsed() < Duration::from_millis(180) {
            json!({"input_tokens": 50, "cache_creation_input_tokens": tokens, "output_tokens": 20})
        } else {
            json!({"input_tokens": 50, "cache_read_input_tokens": tokens, "output_tokens": 20})
        };
        messages_reply(FLYING, &usage).after(Duration::from_millis(200))
    })
    .await;
    let opts = Options {
        jobs: 4,
        ..options(spend(dir.path(), Usd(50_000_000), Billing::Standard))
    };
    let (result, _) = run_sync(
        dir.path(),
        drakes(6),
        &server,
        Duration::from_secs(30),
        opts,
    )
    .await;
    result.unwrap();

    let writes = ledger_lines(dir.path())
        .iter()
        .filter(|line| line[4] != "0")
        .count();
    assert_eq!(writes, 1, "one cache write, five reads");
    assert_eq!(server.log().len(), 6);
}

// ---- the batch mode ----

/// A mock of the Message Batches API: batches are made of what is posted, a batch ends on its second look, the answer to
/// a card comes from `answer`, and each answer is billed `usage`.
struct BatchMock {
    posted: Mutex<Vec<Vec<Value>>>,
    looks: AtomicUsize,
    healthy: AtomicBool,
}

impl BatchMock {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            posted: Mutex::new(Vec::new()),
            looks: AtomicUsize::new(0),
            healthy: AtomicBool::new(true),
        })
    }

    fn posts(&self) -> Vec<Vec<Value>> {
        self.posted
            .lock()
            .map(|posted| posted.clone())
            .unwrap_or_default()
    }

    async fn serve(
        self: &Arc<Self>,
        answer: impl Fn(&str, bool) -> String + Send + Sync + 'static,
        usage: impl Fn(&Value) -> Value + Send + Sync + 'static,
    ) -> Server {
        let mock = self.clone();
        start(move |seen: &Seen| {
            assert_eq!(seen.header("x-api-key"), Some(KEY));
            let batch_of = |path: &str| {
                path.strip_prefix("/v1/messages/batches/")
                    .and_then(|rest| rest.split('/').next())
                    .and_then(|id| id.strip_prefix("msgbatch_"))
                    .and_then(|n| n.parse::<usize>().ok())
            };
            match (seen.method.as_str(), seen.path.as_str()) {
                ("POST", "/v1/messages/batches") => {
                    let body: Value = serde_json::from_slice(&seen.body).unwrap_or_default();
                    let requests = body["requests"].as_array().cloned().unwrap_or_default();
                    let mut posted = mock.posted.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                    posted.push(requests);
                    batch_reply(posted.len(), false)
                }
                ("GET", path) if path.ends_with("/results") => {
                    let n = batch_of(path).unwrap_or(0);
                    let posted = mock.posted.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                    let lines: Vec<String> = posted[n - 1]
                        .iter()
                        .map(|request| {
                            let params = &request["params"];
                            let repair = params["messages"].as_array().is_some_and(|m| m.len() > 1);
                            let text = answer(&card_of(params), repair);
                            json!({
                                "custom_id": request["custom_id"],
                                "result": {"type": "succeeded", "message": message(&text, &usage(params))},
                            })
                            .to_string()
                        })
                        .collect();
                    Reply::ok(lines.join("\n")).header("Content-Type", "application/x-jsonl")
                }
                ("GET", path) => {
                    if !mock.healthy.load(Ordering::SeqCst) {
                        return Reply::status(400).body(r#"{"type":"error","error":{"type":"invalid_request_error","message":"broken"}}"#);
                    }
                    let n = batch_of(path).unwrap_or(0);
                    let looks = mock.looks.fetch_add(1, Ordering::SeqCst);
                    batch_reply(n, looks % 2 == 1)
                }
                _ => Reply::status(404),
            }
        })
        .await
    }
}

fn batch_reply(n: usize, ended: bool) -> Reply {
    let body = json!({
        "id": format!("msgbatch_{n}"), "type": "message_batch",
        "processing_status": if ended { "ended" } else { "in_progress" },
        "request_counts": {"processing": u32::from(!ended), "succeeded": u32::from(ended), "errored": 0, "canceled": 0, "expired": 0},
    });
    Reply::ok(body.to_string()).header("Content-Type", "application/json")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_batch_is_polled_settled_into_the_ledger_at_half_price_and_repaired_in_a_second_batch() {
    let dir = tempfile::tempdir().unwrap();
    let cards = drakes(3);
    let mock = BatchMock::new();
    let usage = json!({"input_tokens": 100, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 900, "output_tokens": 50});
    let reported = usage.clone();
    let server = mock
        .serve(
            |card, repair| {
                // Drake 01 is first answered with something that is no rules and repaired in the second batch.
                if card == "Drake 01" && !repair {
                    "I cannot write this.".to_owned()
                } else {
                    FLYING.to_owned()
                }
            },
            move |_| reported.clone(),
        )
        .await;
    let (result, captured) = run_batch(
        dir.path(),
        cards,
        &server,
        options(spend(dir.path(), Usd(50_000_000), Billing::Batch)),
    )
    .await;
    let report = result.unwrap();

    let posts = mock.posts();
    assert_eq!(posts.iter().map(Vec::len).collect::<Vec<_>>(), [3, 1]);
    let first = &posts[0][0];
    assert_eq!(first["custom_id"], "c0");
    assert_eq!(first["params"]["model"], MODEL);
    assert_eq!(
        first["params"]["system"][0]["cache_control"]["type"],
        "ephemeral"
    );
    let repair = &posts[1][0]["params"]["messages"];
    assert_eq!(
        repair.as_array().map(Vec::len),
        Some(3),
        "the repair carries the answer and the checker's words"
    );
    assert_eq!(repair[1]["content"], "I cannot write this.");

    assert_eq!(
        (
            report.tally.verified,
            report.budget.requests(),
            report.budget.repairs()
        ),
        (3, 4, 1)
    );
    let price = price_of(MODEL).unwrap();
    let one = price
        .cost(
            &mtg_cli::propose::model::Usage {
                input_tokens: 1000,
                cache_write_tokens: 0,
                cache_read_tokens: 900,
                output_tokens: 50,
            },
            Billing::Batch,
        )
        .0;
    // A batch is billed at half: 100 new tokens $0.0001, 900 read $0.00009, 50 answered $0.00025.
    assert_eq!(one, 100 + 90 + 250);
    let ledger = Ledger::at(dir.path().join("api-ledger.tsv"));
    assert_eq!(
        ledger.total().unwrap(),
        i64::try_from(one * 4).unwrap(),
        "the reservations are given back"
    );
    let lines = ledger_lines(dir.path());
    let labels: Vec<&str> = lines.iter().map(|line| line[1].as_str()).collect();
    assert_eq!(labels.iter().filter(|l| l.contains("reserved")).count(), 2);
    assert_eq!(labels.iter().filter(|l| l.contains("settled")).count(), 2);
    assert_eq!(lines.iter().filter(|l| l[7] == "0.000440").count(), 4);

    assert!(
        !dir.path().join("out/batch-state.json").exists(),
        "nothing is out any more"
    );
    let summary = std::fs::read_to_string(dir.path().join("out/summary.tsv")).unwrap();
    assert_eq!(summary.lines().count(), 4, "{summary}");
    assert!(
        summary.contains("Drake 01\tverified\t\t2\t0.000880"),
        "{summary}"
    );
    assert!(
        captured.text().contains("batch msgbatch_1"),
        "{}",
        captured.text()
    );
    for text in [
        &all_text_under(dir.path()),
        &captured.text(),
        &report.render(),
    ] {
        assert!(!text.contains(KEY));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_batch_that_is_out_is_collected_by_a_resumed_run_and_never_billed_twice() {
    let dir = tempfile::tempdir().unwrap();
    let cards = drakes(2);
    let mock = BatchMock::new();
    mock.healthy.store(false, Ordering::SeqCst);
    let server = mock
        .serve(
            |_, _| FLYING.to_owned(),
            |_| json!({"input_tokens": 100, "output_tokens": 40}),
        )
        .await;
    let first = options(spend(dir.path(), Usd(50_000_000), Billing::Batch));

    // The first run asks for the batch and then cannot look at it.
    let (stopped, _) = run_batch(dir.path(), cards.clone(), &server, first.clone()).await;
    assert!(
        stopped.is_err(),
        "the run ends when the API refuses to say where the batch is"
    );
    assert!(
        dir.path().join("out/batch-state.json").is_file(),
        "the batch that is out is remembered"
    );
    let reserved = ledger_lines(dir.path());
    assert_eq!(reserved.len(), 1);
    assert!(reserved[0][1].contains("reserved"), "{reserved:?}");
    let held = Ledger::at(dir.path().join("api-ledger.tsv"))
        .total()
        .unwrap();
    assert!(
        held > 0,
        "the worst case of the batch counts toward the cap while it is out"
    );

    // Without --resume a second run refuses to ask again and says how to collect the batch.
    let (refused, _) = run_batch(dir.path(), cards.clone(), &server, first.clone()).await;
    let message = refused.unwrap_err().to_string();
    assert!(
        message.contains("--resume") && message.contains("msgbatch_1"),
        "{message}"
    );
    assert_eq!(mock.posts().len(), 1);

    // With --resume it collects the same batch: no new batch, the reservation returned, each card billed once.
    mock.healthy.store(true, Ordering::SeqCst);
    let resumed = Options {
        resume: true,
        ..first
    };
    let (result, captured) = run_batch(dir.path(), cards.clone(), &server, resumed.clone()).await;
    let report = result.unwrap();
    assert_eq!(mock.posts().len(), 1, "the batch is not asked for again");
    assert_eq!(report.tally.verified, 2);
    assert!(
        captured.text().contains("resuming batch msgbatch_1"),
        "{}",
        captured.text()
    );
    let after = Ledger::at(dir.path().join("api-ledger.tsv"))
        .total()
        .unwrap();
    // 100 new tokens $0.0001 and 40 answered $0.0002 at half price: 100 + 200 micro-dollars for each of two cards.
    assert_eq!(after, 2 * (100 + 200));
    assert!(!dir.path().join("out/batch-state.json").exists());

    // And once more: nothing is out and every card has its file, so nothing is asked or billed.
    let (again, _) = run_batch(dir.path(), cards, &server, resumed).await;
    assert_eq!(again.unwrap().tally.skipped, 2);
    assert_eq!(mock.posts().len(), 1);
    assert_eq!(
        Ledger::at(dir.path().join("api-ledger.tsv"))
            .total()
            .unwrap(),
        after
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn the_cap_splits_a_batch_and_ends_the_run_when_the_money_is_spent() {
    let dir = tempfile::tempdir().unwrap();
    let cards = drakes(5);
    let worst = worst_of(&cards, Billing::Batch);
    // Each answer is billed at its worst case, so two and a half of them fit: two cards go in the first batch, and the
    // settled bill leaves no room for a card of the next.
    let cap = Usd(worst.0 * 5 / 2);
    let mock = BatchMock::new();
    let server = mock.serve(|_, _| FLYING.to_owned(), worst_usage).await;
    let (result, _) = run_batch(
        dir.path(),
        cards,
        &server,
        options(spend(dir.path(), cap, Billing::Batch)),
    )
    .await;
    let report = result.unwrap();

    assert_eq!(mock.posts().iter().map(Vec::len).collect::<Vec<_>>(), [2]);
    assert_eq!((report.tally.verified, report.tally.not_run), (2, 3));
    assert!(
        matches!(report.stopped, Some(Stop::Spend { .. })),
        "{:?}",
        report.stopped
    );
    let total = u64::try_from(
        Ledger::at(dir.path().join("api-ledger.tsv"))
            .total()
            .unwrap(),
    )
    .unwrap();
    assert!(
        total <= cap.0 && total + 4 >= worst.0 * 2,
        "{total} of {cap}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_batch_asks_for_the_cheaper_pieces_the_cap_allows_when_answers_cost_less_than_their_worst_case()
 {
    let dir = tempfile::tempdir().unwrap();
    let cards = drakes(5);
    let worst = worst_of(&cards, Billing::Batch);
    let mock = BatchMock::new();
    let server = mock
        .serve(
            |_, _| FLYING.to_owned(),
            |_| json!({"input_tokens": 10, "output_tokens": 10}),
        )
        .await;
    let (result, _) = run_batch(
        dir.path(),
        cards,
        &server,
        options(spend(dir.path(), Usd(worst.0 * 5 / 2), Billing::Batch)),
    )
    .await;
    let report = result.unwrap();

    // The pieces are as large as the cap's worst cases allow (two), and the money the real answers did not use goes to the next.
    assert_eq!(
        mock.posts().iter().map(Vec::len).collect::<Vec<_>>(),
        [2, 2, 1]
    );
    assert_eq!((report.tally.verified, report.tally.not_run), (5, 0));
    assert_eq!(report.stopped, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_batch_without_a_cap_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mock = BatchMock::new();
    let server = mock.serve(|_, _| FLYING.to_owned(), |_| json!({})).await;
    let opts = Options {
        spend: None,
        ..options(spend(dir.path(), Usd(1), Billing::Batch))
    };
    let (result, _) = run_batch(dir.path(), drakes(1), &server, opts).await;
    assert!(result.unwrap_err().to_string().contains("--max-spend"));
    assert_eq!(mock.posts().len(), 0);
}

// ---- outcomes: drafts, failures with the missing engine piece, the summary ----

#[test]
fn every_card_ends_as_a_draft_or_a_failure_with_the_engine_piece_it_misses_in_one_summary() {
    use mtg_cli::propose::fake::ScriptedClient;

    let dir = tempfile::tempdir().unwrap();
    let cards = vec![drake("Wind Drake"), kormus_bell(), drake("Odd Drake")];
    let verified = r#"{"faces":[{"abilities":[{"Keyword":"Flying"}]}],"missing":[]}"#;
    let draft = KORMUS_BELL.replacen(
        '{',
        r#"{"missing":["layer 4 effect: lands that are also creatures"],"#,
        1,
    );
    let nonsense = r#"{"faces":[{"abilities":[{"Keyword":"Gliding"}]}],"missing":["a keyword Gliding", "  a\ttab\nand a break  "]}"#;
    let client = ScriptedClient::new()
        .answer("Wind Drake", verified)
        .answer("Kormus Bell", &draft)
        .answer("Odd Drake", nonsense)
        .answer("Odd Drake", nonsense);
    let (progress, _) = Progress::buffer();
    let names: Vec<String> = cards.iter().map(|card| card.name.clone()).collect();
    let targets = select::by_names(&cards, &names).unwrap();
    let out = dir.path().join("out");
    let report = propose_cards(
        &cards,
        &targets,
        &client,
        &Options {
            model: MODEL.into(),
            examples: 4,
            max_requests: 100,
            max_input_tokens: 500_000_000,
            jobs: 1,
            ..Options::default()
        },
        &Store::new(&out),
        &progress,
    )
    .unwrap();

    // The model's `missing` list is not part of the rules: the checker never sees it.
    assert_eq!(
        (
            report.tally.verified,
            report.tally.unchecked,
            report.tally.invalid
        ),
        (1, 1, 1)
    );
    assert!(out.join("wind_drake.json").is_file());
    let draft_file = std::fs::read_to_string(out.join("drafts/kormus_bell.json")).unwrap();
    assert!(
        draft_file.contains("SetColors") && !draft_file.contains("missing"),
        "{draft_file}"
    );
    assert!(out.join("rejected/odd_drake.txt").is_file());
    assert!(
        !out.join("rejected/kormus_bell.json").exists(),
        "a draft is no rejection"
    );

    let summary = std::fs::read_to_string(out.join("summary.tsv")).unwrap();
    let lines: Vec<Vec<&str>> = summary
        .lines()
        .map(|line| line.split('\t').collect())
        .collect();
    assert_eq!(
        lines[0],
        [
            "card",
            "outcome",
            "missing_engine_piece",
            "requests",
            "usd",
            "detail",
            "agree"
        ]
    );
    assert!(lines.iter().all(|line| line.len() == 7), "{summary}");
    let by_card = |name: &str| lines.iter().find(|line| line[0] == name).unwrap().clone();
    assert_eq!(&by_card("Wind Drake")[1..4], ["verified", "", "1"]);
    let bell = by_card("Kormus Bell");
    assert_eq!(
        &bell[1..4],
        [
            "draft",
            "layer 4 effect: lands that are also creatures",
            "1"
        ]
    );
    assert!(bell[5].starts_with("no writer for:"), "{bell:?}");
    let odd = by_card("Odd Drake");
    assert_eq!(
        &odd[1..4],
        ["invalid", "a keyword Gliding | a tab and a break", "2"]
    );
    assert!(
        !odd[5].is_empty(),
        "the first thing the checker found wrong is kept: {odd:?}"
    );
    let order: Vec<&str> = lines[1..].iter().map(|line| line[1]).collect();
    assert_eq!(order, ["verified", "draft", "invalid"], "by outcome");
}

#[test]
fn a_run_resumed_over_the_same_folder_keeps_the_records_of_the_cards_it_skips() {
    use mtg_cli::propose::fake::ScriptedClient;

    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("out");
    let (progress, _) = Progress::buffer();
    let one = |name: &str, resume: bool| {
        let cards = vec![drake(name)];
        let client = ScriptedClient::new().answer(name, FLYING);
        propose_cards(
            &cards,
            &[0],
            &client,
            &Options {
                model: MODEL.into(),
                examples: 4,
                resume,
                max_requests: 10,
                max_input_tokens: 500_000_000,
                jobs: 1,
                ..Options::default()
            },
            &Store::new(&out),
            &progress,
        )
        .unwrap()
    };
    one("Drake 00", false);
    one("Drake 01", true);
    let summary = std::fs::read_to_string(out.join("summary.tsv")).unwrap();
    assert_eq!(
        summary.lines().count(),
        3,
        "both cards are in the summary: {summary}"
    );
}

// ---- the card list ----

#[test]
fn a_file_of_names_is_the_card_list_in_its_order_with_comments_and_blank_lines_left_out() {
    use clap::Parser;
    use mtg_cli::args::{Cli, Command};

    let dir = tempfile::tempdir().unwrap();
    let list = dir.path().join("order.txt");
    std::fs::write(
        &list,
        "# server decks first\nDrake 02\n\n  Drake 00  \nkormus bell\n",
    )
    .unwrap();
    let path = list.display().to_string();
    let Ok(Cli {
        command: Command::Propose(args),
        ..
    }) = Cli::try_parse_from(["mtg-cli", "propose", "--cards", &path, "--max-spend", "5"])
    else {
        panic!("propose parses");
    };
    let mut cards = drakes(3);
    cards.push(kormus_bell());
    let targets = mtg_cli::propose::targets_of(&cards, &args, None).unwrap();
    let names: Vec<&str> = targets.iter().map(|at| cards[*at].name.as_str()).collect();
    assert_eq!(names, ["Drake 02", "Drake 00", "Kormus Bell"]);

    // A name that no card has is an error that names it, as with names on the command line.
    std::fs::write(&list, "Drake 00\nNo Such Card\n").unwrap();
    let error = mtg_cli::propose::targets_of(&cards, &args, None).unwrap_err();
    assert!(error.to_string().contains("No Such Card"), "{error}");
}

// ---- the command: the cap is required, models have short names, the batch mode and the card order run end to end ----

const BOLT: &str = r#"{"faces":[{"abilities":[{"Spell":{"targets":[{"what":"AnyTarget"}],"effects":[{"DealDamage":{"to":{"Target":0},"amount":3}}]}}]}]}"#;

fn env_with_key(name: &str) -> Option<String> {
    (name == "ANTHROPIC_API_KEY").then(|| KEY.to_owned())
}

#[sqlx::test]
async fn a_run_needs_a_cap_before_it_needs_a_key_and_only_the_anthropic_api_has_one(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = loaded(pool, dir.path()).await;
    let server = start(|_: &Seen| messages_reply(BOLT, &json!({}))).await;
    let (progress, _) = Progress::buffer();
    let api = config(&server, Duration::from_secs(30));
    let none = |_: &str| None;
    let with_key = env_with_key;
    let run = |words: &[&str], key: bool| {
        let env: &dyn Fn(&str) -> Option<String> = if key { &with_key } else { &none };
        let (db, cache, progress, api) = (&db, &cache, progress.clone(), api.clone());
        let words: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
        async move {
            let refs: Vec<&str> = words.iter().map(String::as_str).collect();
            run_command(db, cache, &args(&refs), progress, env, &api)
                .await
                .unwrap_err()
                .to_string()
        }
    };

    let error = run(&["--cards", "Lightning Bolt"], true).await;
    assert!(error.contains("--max-spend"), "{error}");
    let error = run(&["--cards", "Lightning Bolt", "--max-spend", "5"], false).await;
    assert!(error.contains("ANTHROPIC_API_KEY"), "{error}");
    let error = run(
        &[
            "--cards",
            "Lightning Bolt",
            "--max-spend",
            "5",
            "--model",
            "gpt-5",
        ],
        true,
    )
    .await;
    assert!(
        error.contains("no price is known for the model `gpt-5`"),
        "{error}"
    );
    let error = run(
        &[
            "--cards",
            "Lightning Bolt",
            "--provider",
            "openai",
            "--model",
            "gpt-oss:20b",
            "--max-spend",
            "5",
        ],
        true,
    )
    .await;
    assert!(
        error.contains("--max-spend is for the Anthropic API"),
        "{error}"
    );

    assert!(server.log().is_empty(), "nothing was sent");
    assert!(!cache.path("api-ledger.tsv").exists());
}

#[sqlx::test]
async fn a_short_model_name_and_the_effort_reach_the_request_and_the_ledger_is_in_the_cache(
    pool: PgPool,
) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = loaded(pool, dir.path()).await;
    let usage = json!({"input_tokens": 3000, "output_tokens": 200});
    let server = start(move |_: &Seen| messages_reply(BOLT, &usage)).await;
    let out = dir.path().join("proposals");
    let out_arg = out.display().to_string();
    let (progress, captured) = Progress::buffer();

    let summary = run_command(
        &db,
        &cache,
        &args(&[
            "--cards",
            "Lightning Bolt",
            "--model",
            "haiku",
            "--reasoning",
            "low",
            "--max-spend",
            "5",
            "--out",
            &out_arg,
        ]),
        progress,
        &env_with_key,
        &config(&server, Duration::from_secs(30)),
    )
    .await
    .unwrap();

    let body: Value = serde_json::from_slice(&server.log()[0].body).unwrap();
    assert_eq!(body["model"], "claude-haiku-5-5");
    assert_eq!(body["output_config"]["effort"], "low");
    // Haiku: 3000 new tokens $0.0003 and 200 answered $0.0001.
    let text = std::fs::read_to_string(cache.path("api-ledger.tsv")).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[1].ends_with("\tclaude-haiku-5-5\t3000\t0\t0\t200\t0.000400"),
        "{}",
        lines[1]
    );
    assert!(
        summary.contains(
            "spent $0.000400 in this run; the ledger holds $0.000400 of the cap of $5.000000"
        ),
        "{summary}"
    );
    assert!(out.join("summary.tsv").is_file());
    for text in [&summary, &captured.text(), &all_text_under(dir.path())] {
        assert!(!text.contains(KEY));
    }
}

#[sqlx::test]
async fn a_dry_run_prices_the_run_from_the_size_of_the_prompts_and_needs_neither_key_nor_cap(
    pool: PgPool,
) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = loaded(pool, dir.path()).await;
    let server = start(|_: &Seen| messages_reply(BOLT, &json!({}))).await;
    let (progress, _) = Progress::buffer();
    let none = |_: &str| None;
    let mut texts = Vec::new();
    for words in [
        &["--cards", "Lightning Bolt", "--dry-run"][..],
        &[
            "--cards",
            "Lightning Bolt",
            "--dry-run",
            "--model",
            "opus",
            "--max-spend",
            "200",
        ][..],
    ] {
        texts.push(
            run_command(
                &db,
                &cache,
                &args(words),
                progress.clone(),
                &none,
                &config(&server, Duration::from_secs(30)),
            )
            .await
            .unwrap(),
        );
    }
    assert!(
        texts[0].contains("estimated cost (claude-sonnet-5-5,"),
        "{}",
        texts[0]
    );
    for part in [
        "a card: $",
        "with a repair round: $",
        "the first request writes the rules to the cache: $",
        "--batch: $",
        "worst case of one request: $",
    ] {
        assert!(texts[0].contains(part), "{part}: {}", texts[0]);
    }
    assert!(!texts[0].contains("--max-spend $"), "{}", texts[0]);
    assert!(
        texts[1].contains("estimated cost (claude-opus-5-5,"),
        "{}",
        texts[1]
    );
    assert!(
        texts[1].contains("--max-spend $200.000000: about "),
        "{}",
        texts[1]
    );
    assert!(server.log().is_empty());
    assert!(!cache.path("api-ledger.tsv").exists());
}

#[sqlx::test]
async fn the_batch_flag_runs_the_whole_command_through_the_batches_api(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = loaded(pool, dir.path()).await;
    let mock = BatchMock::new();
    let server = mock
        .serve(
            |_, _| BOLT.to_owned(),
            |_| json!({"input_tokens": 3000, "output_tokens": 200}),
        )
        .await;
    let out = dir.path().join("proposals");
    let out_arg = out.display().to_string();
    let (progress, captured) = Progress::buffer();

    let summary = run_command(
        &db,
        &cache,
        &args(&[
            "--cards",
            "Lightning Bolt",
            "--batch",
            "--poll-every",
            "1",
            "--max-spend",
            "5",
            "--out",
            &out_arg,
        ]),
        progress,
        &env_with_key,
        &config(&server, Duration::from_secs(30)),
    )
    .await
    .unwrap();

    assert!(
        summary.contains("Accepted Verified                1"),
        "{summary}"
    );
    assert_eq!(mock.posts().iter().map(Vec::len).collect::<Vec<_>>(), [1]);
    assert!(out.join("lightning_bolt.json").is_file());
    // Sonnet at half price: 3000 new tokens $0.003 and 200 answered $0.001.
    assert_eq!(Ledger::of(&cache).total().unwrap(), 4_000);
    assert!(
        captured.text().contains("batch msgbatch_1 ended"),
        "{}",
        captured.text()
    );
    assert!(!out.join("batch-state.json").exists());
}

/// A deck of `owner` holding one line for each named card.
async fn deck(db: &Database, owner: u128, cards: &[&str]) {
    let id: sqlx::types::uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO decks (owner, name, format) VALUES ($1, 'Deck', 'freeform') RETURNING id",
    )
    .bind(sqlx::types::uuid::Uuid::from_u128(owner))
    .fetch_one(db.pool())
    .await
    .unwrap();
    for (position, name) in cards.iter().enumerate() {
        sqlx::query(
            "INSERT INTO deck_cards (deck_id, position, zone, oracle_id, count)
             SELECT $1, $2, 'main', oracle_id, 1 FROM cards WHERE name = $3",
        )
        .bind(id)
        .bind(i32::try_from(position).unwrap())
        .bind(*name)
        .execute(db.pool())
        .await
        .unwrap();
    }
}

#[sqlx::test]
async fn the_order_helper_lists_the_cards_of_saved_decks_first_and_only_cards_without_rules(
    pool: PgPool,
) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = loaded(pool, dir.path()).await;
    deck(
        &db,
        1,
        &["Lightning Bolt", "Sol Ring", "Jace, the Mind Sculptor"],
    )
    .await;
    deck(&db, 2, &["Sol Ring", "Jace, the Mind Sculptor"]).await;
    deck(&db, 1, &["Jace, the Mind Sculptor"]).await;
    let file = dir.path().join("order.txt");

    let printed = mtg_cli::propose::order::command(
        &db,
        &cache,
        &ProposeOrderArgs {
            out: None,
            limit: None,
            lines: None,
        },
    )
    .await
    .unwrap();
    let names: Vec<&str> = printed.lines().collect();
    assert_eq!(
        names[0], "Jace, the Mind Sculptor",
        "the deck cards come first: {printed}"
    );
    assert!(
        !names.contains(&"Lightning Bolt") && !names.contains(&"Sol Ring"),
        "a card with rules is left out: {printed}"
    );
    assert!(
        names.len() > 1,
        "the other cards follow by popularity: {printed}"
    );

    let said = mtg_cli::propose::order::command(
        &db,
        &cache,
        &ProposeOrderArgs {
            out: Some(file.clone()),
            limit: Some(1),
            lines: None,
        },
    )
    .await
    .unwrap();
    assert!(
        said.contains("1 card written to") && said.contains("1 of the saved decks first"),
        "{said}"
    );
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "Jace, the Mind Sculptor\n"
    );
}

// ---- the workspace of a key that is not scoped to one ----

const WORKSPACE: &str = "wrkspc_01TestWorkspace";

fn env_with_workspace(name: &str) -> Option<String> {
    match name {
        "ANTHROPIC_API_KEY" => Some(KEY.to_owned()),
        "ANTHROPIC_WORKSPACE_ID" => Some(format!("  {WORKSPACE}\n")),
        _ => None,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_workspace_id_is_sent_with_every_request_of_the_messages_and_the_batches_api() {
    let mock = BatchMock::new();
    let server = mock
        .serve(
            |_, _| FLYING.to_owned(),
            |_| json!({"input_tokens": 10, "output_tokens": 10}),
        )
        .await;
    let make = |workspace: Option<&str>| {
        AnthropicClient::with_options(
            ApiKey::from_env(|_| Some(KEY.to_owned())).unwrap(),
            &config(&server, Duration::from_secs(30)),
            tokio::runtime::Handle::current(),
            Progress::buffer().0,
            None,
            workspace.map(str::to_owned),
        )
        .unwrap()
    };
    let with = make(Some(WORKSPACE));
    let without = make(None);
    let request = mtg_cli::propose::model::Request {
        model: MODEL.into(),
        system: "rules".into(),
        messages: vec![mtg_cli::propose::model::Message::user("Card: Opt")],
        max_tokens: 100,
    };
    tokio::task::spawn_blocking(move || {
        // The same client asks for a message, a batch, its state and its results.
        let api = BatchApi::new(with, None);
        let info = api.create(&[("c0".to_owned(), &request)]).unwrap();
        api.status(&info.id).unwrap();
        api.results(&info.id).unwrap();
        // One that has no workspace sends none.
        let plain = BatchApi::new(without, None);
        plain.status(&info.id).unwrap();
    })
    .await
    .unwrap();

    let seen = server.log();
    assert_eq!(seen.len(), 4, "{seen:?}");
    for request in &seen[..3] {
        assert_eq!(
            request.header("anthropic-workspace-id"),
            Some(WORKSPACE),
            "{} {}",
            request.method,
            request.path
        );
        assert_eq!(request.header("x-api-key"), Some(KEY));
    }
    assert_eq!(seen[3].header("anthropic-workspace-id"), None);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_message_request_names_the_workspace_too() {
    let server = start(|_: &Seen| messages_reply(FLYING, &json!({}))).await;
    let client = AnthropicClient::with_options(
        ApiKey::from_env(|_| Some(KEY.to_owned())).unwrap(),
        &config(&server, Duration::from_secs(30)),
        tokio::runtime::Handle::current(),
        Progress::buffer().0,
        None,
        Some(WORKSPACE.to_owned()),
    )
    .unwrap();
    let request = mtg_cli::propose::model::Request {
        model: MODEL.into(),
        system: "rules".into(),
        messages: vec![mtg_cli::propose::model::Message::user("Card: Opt")],
        max_tokens: 100,
    };
    tokio::task::spawn_blocking(move || {
        mtg_cli::propose::model::ModelClient::complete(&client, &request).unwrap()
    })
    .await
    .unwrap();
    let seen = server.log();
    assert_eq!(seen[0].header("anthropic-workspace-id"), Some(WORKSPACE));
    assert!(
        !seen[0].body_text().contains(WORKSPACE),
        "a header, not a body field"
    );
}

#[sqlx::test]
async fn the_command_reads_the_workspace_from_the_environment_for_a_run_and_a_batch(pool: PgPool) {
    let dir = tempfile::tempdir().unwrap();
    let (db, cache) = loaded(pool, dir.path()).await;
    let mock = BatchMock::new();
    let server = mock
        .serve(
            |_, _| BOLT.to_owned(),
            |_| json!({"input_tokens": 100, "output_tokens": 20}),
        )
        .await;
    let sync =
        start(|_: &Seen| messages_reply(BOLT, &json!({"input_tokens": 100, "output_tokens": 20})))
            .await;
    let out = dir.path().join("proposals");
    let out_arg = out.display().to_string();
    let (progress, captured) = Progress::buffer();

    let summary = run_command(
        &db,
        &cache,
        &args(&[
            "--cards",
            "Lightning Bolt",
            "--max-spend",
            "5",
            "--out",
            &out_arg,
        ]),
        progress.clone(),
        &env_with_workspace,
        &config(&sync, Duration::from_secs(30)),
    )
    .await
    .unwrap();
    let batch_out = dir.path().join("batch");
    let batch_arg = batch_out.display().to_string();
    let batched = run_command(
        &db,
        &cache,
        &args(&[
            "--cards",
            "Lightning Bolt",
            "--batch",
            "--poll-every",
            "1",
            "--max-spend",
            "5",
            "--out",
            &batch_arg,
        ]),
        progress,
        &env_with_workspace,
        &config(&server, Duration::from_secs(30)),
    )
    .await
    .unwrap();

    for log in [sync.log(), server.log()] {
        assert!(!log.is_empty());
        for request in &log {
            assert_eq!(
                request.header("anthropic-workspace-id"),
                Some(WORKSPACE),
                "{} {}",
                request.method,
                request.path
            );
        }
    }
    assert!(server.log().len() >= 3, "create, status and results");
    // The workspace is no secret, but nothing needs to print it.
    for text in [
        &summary,
        &batched,
        &captured.text(),
        &all_text_under(dir.path()),
    ] {
        assert!(!text.contains(KEY));
    }

    // Without the variable, or with an empty one, no header is sent.
    let plain = start(|_: &Seen| messages_reply(BOLT, &json!({}))).await;
    let (progress, _) = Progress::buffer();
    let empty = |name: &str| match name {
        "ANTHROPIC_API_KEY" => Some(KEY.to_owned()),
        "ANTHROPIC_WORKSPACE_ID" => Some("  ".to_owned()),
        _ => None,
    };
    let again_out = dir.path().join("again");
    let again_arg = again_out.display().to_string();
    run_command(
        &db,
        &cache,
        &args(&[
            "--cards",
            "Lightning Bolt",
            "--max-spend",
            "5",
            "--out",
            &again_arg,
        ]),
        progress,
        &empty,
        &config(&plain, Duration::from_secs(30)),
    )
    .await
    .unwrap();
    assert_eq!(plain.log()[0].header("anthropic-workspace-id"), None);
}

// ---- asking for a batch is not repeated when it may have been made ----

#[tokio::test(flavor = "multi_thread")]
async fn a_batch_is_not_asked_for_again_after_a_timeout_and_its_worst_case_stays_in_the_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let server = start(|seen: &Seen| match seen.method.as_str() {
        // The batch may well exist by the time the client gives up.
        "POST" => batch_reply(1, false).after(Duration::from_secs(3)),
        _ => Reply::status(404),
    })
    .await;
    let (progress, _) = Progress::buffer();
    let patient = AnthropicConfig {
        attempts: 3,
        ..config(&server, Duration::from_millis(300))
    };
    let api = BatchApi::new(
        AnthropicClient::new(
            ApiKey::from_env(|_| Some(KEY.to_owned())).unwrap(),
            &patient,
            tokio::runtime::Handle::current(),
            progress.clone(),
        )
        .unwrap(),
        None,
    );
    let out = dir.path().join("out");
    let opts = options(spend(dir.path(), Usd(50_000_000), Billing::Batch));
    let task = tokio::task::spawn_blocking(move || {
        let cards = drakes(1);
        let ctx = context(&cards).unwrap();
        BatchRun {
            ctx: &ctx,
            api: &api,
            options: &opts,
            store: &Store::new(out),
            progress: &progress,
        }
        .run(&cards, &[0])
    });
    let error = task.await.unwrap().unwrap_err();

    assert!(error.to_string().contains("--request-timeout"), "{error}");
    assert_eq!(
        server.log().len(),
        1,
        "a timed-out creation is not repeated"
    );
    let lines = ledger_lines(dir.path());
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0][1].contains("reserved"),
        "the batch may be billed: {lines:?}"
    );
    assert!(!dir.path().join("out/batch-state.json").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_overloaded_api_is_asked_again_and_a_refused_batch_gives_its_reservation_back() {
    let dir = tempfile::tempdir().unwrap();
    let asked = Arc::new(AtomicUsize::new(0));
    let counter = asked.clone();
    let server = start(move |_: &Seen| {
        if counter.fetch_add(1, Ordering::SeqCst) == 0 {
            Reply::status(529)
                .body(r#"{"type":"error","error":{"type":"overloaded_error","message":"busy"}}"#)
        } else {
            Reply::status(400).body(
                r#"{"type":"error","error":{"type":"invalid_request_error","message":"too large"}}"#,
            )
        }
    })
    .await;
    let (progress, _) = Progress::buffer();
    let patient = AnthropicConfig {
        attempts: 3,
        ..config(&server, Duration::from_secs(30))
    };
    let api = BatchApi::new(
        AnthropicClient::new(
            ApiKey::from_env(|_| Some(KEY.to_owned())).unwrap(),
            &patient,
            tokio::runtime::Handle::current(),
            progress.clone(),
        )
        .unwrap(),
        None,
    );
    let out = dir.path().join("out");
    let opts = options(spend(dir.path(), Usd(50_000_000), Billing::Batch));
    let task = tokio::task::spawn_blocking(move || {
        let cards = drakes(1);
        let ctx = context(&cards).unwrap();
        BatchRun {
            ctx: &ctx,
            api: &api,
            options: &opts,
            store: &Store::new(out),
            progress: &progress,
        }
        .run(&cards, &[0])
    });
    let error = task.await.unwrap().unwrap_err();

    assert!(error.to_string().contains("400"), "{error}");
    assert_eq!(
        asked.load(Ordering::SeqCst),
        2,
        "the overload is retried, the refusal is not"
    );
    let ledger = Ledger::at(dir.path().join("api-ledger.tsv"));
    assert_eq!(
        ledger.total().unwrap(),
        0,
        "nothing was made, so nothing is held"
    );
}
