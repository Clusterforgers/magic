//! Goal: line mode on a mock Anthropic API is capped and accounted like a whole-card run: the spend cap stops the run
//! before the request that would pass it, each request lands in the ledger, two gapped lines go out as two requests and
//! come back merged into one card, a batch asks the samples and the repairs round by round, is settled once and is
//! collected by a resumed run, and the key appears in no output.

#![allow(clippy::unwrap_used)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use mtg_cards::load::slug;
use mtg_cards::proposal::partial::LedgerLine;
use mtg_cards::{PrintedCard, PrintedFace};
use mtg_cli::error::CliError;
use mtg_cli::progress::{Captured, Progress};
use mtg_cli::propose::anthropic::{Anthropic, AnthropicClient, AnthropicConfig, ApiKey};
use mtg_cli::propose::batch::BatchApi;
use mtg_cli::propose::forge_hint::ForgeHints;
use mtg_cli::propose::ledger::Ledger;
use mtg_cli::propose::lines::batch::{BatchSettings, STATE_FILE};
use mtg_cli::propose::lines::paid::{Meter, PaidModels};
use mtg_cli::propose::lines::{LineCtx, LineOptions};
use mtg_cli::propose::price::{Billing, Usd};
use mtg_cli::propose::spend::Spend;
use serde_json::{Value, json};

use crate::common::server::{Reply, Request as Seen, Server, start};

const KEY: &str = "sk-ant-api03-SECRET-lf23-0123456789";
const MODEL: &str = "claude-sonnet-5-5";
const ATTACKS: &str = "Whenever Test Wyvern attacks, exile the top card of each library face down.";
const TAP: &str = "{T}: Draw a card.";
const SCRIPT: &str = "Name:Test Wyvern\nManaCost:2 U\nTypes:Creature Drake\nPT:2/2\nK:Flying\n\
T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ TrigDig | TriggerDescription$ Whenever Test Wyvern attacks, exile the top card of each library face down.\n\
SVar:TrigDig:DB$ Dig | Defined$ Player | DigNum$ 1\n\
A:AB$ Draw | Cost$ T | NumCards$ 1 | SpellDescription$ {T}: Draw a card.\nK:Trample\n";
const TRIGGER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}]}}"#;
const DRAW: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}"#;
const MISSING: &str = r#"{"missing": "a face-down exile no player may look at"}"#;

fn wyvern(name: &str) -> PrintedCard {
    PrintedCard {
        oracle_id: format!(
            "00000000-0000-4000-8000-{:012}",
            name.bytes().map(u64::from).sum::<u64>()
        ),
        name: name.to_owned(),
        keywords: Vec::new(),
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            mana_cost: "{2}{U}".into(),
            type_line: "Creature — Drake".into(),
            oracle_text: format!(
                "Flying\n{}\n{TAP}\nTrample",
                ATTACKS.replace("Test Wyvern", name)
            ),
            power: Some("2".into()),
            toughness: Some("2".into()),
            ..PrintedFace::default()
        }],
    }
}

fn row(tag: &str, text: &str, abilities: &[usize], gap: Option<&str>) -> LedgerLine {
    LedgerLine {
        face: 0,
        tag: tag.into(),
        text: text.into(),
        abilities: abilities.to_vec(),
        gap: gap.map(str::to_owned),
    }
}

fn write_file(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// Wyverns with two gapped lines each: the attack trigger and the tap ability.
struct Env {
    root: tempfile::TempDir,
    cards: Vec<PrintedCard>,
    translations: PathBuf,
}

impl Env {
    fn new(count: usize) -> Env {
        let root = tempfile::tempdir().unwrap();
        let forge_dir = root.path().join("cardsfolder");
        let translations = root.path().join("tr");
        let cards: Vec<PrintedCard> = (1..=count)
            .map(|n| wyvern(&format!("Test Wyvern {n:02}")))
            .collect();
        for card in &cards {
            let stem = slug(&card.name);
            write_file(
                &forge_dir.join(format!("t/{stem}.txt")),
                &SCRIPT.replace("Test Wyvern", &card.name),
            );
            write_file(
                &translations.join(format!("{stem}.json")),
                &json!({"faces": [{"abilities": [{"Keyword": "Flying"}, {"Keyword": "Trample"}]}]})
                    .to_string(),
            );
            let lines = vec![
                row("K", "Flying", &[0], None),
                row(
                    "T",
                    &ATTACKS.replace("Test Wyvern", &card.name),
                    &[],
                    Some("effect:Dig.Exile"),
                ),
                row("A", TAP, &[], Some("cost:Tap")),
                row("K", "Trample", &[1], None),
            ];
            write_file(
                &translations.join("meta").join(format!("{stem}.json")),
                &json!({"source": "forge", "lines": lines}).to_string(),
            );
        }
        Env {
            root,
            cards,
            translations,
        }
    }

    fn out(&self) -> PathBuf {
        self.root.path().join("out")
    }

    fn ledger(&self) -> Ledger {
        Ledger::at(self.root.path().join("api-ledger.tsv"))
    }

    fn spend(&self, cap: Usd, billing: Billing) -> Spend {
        Spend::new(self.ledger(), cap, MODEL, billing).unwrap()
    }

    fn names(&self) -> Vec<String> {
        self.cards.iter().map(|card| card.name.clone()).collect()
    }
}

/// The card a request body is about, and whether it asks for the trigger line.
fn about(params: &Value) -> (String, bool) {
    let text = params["messages"][0]["content"]
        .as_str()
        .unwrap_or_default();
    let card = text
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("Card: "))
        .unwrap_or_default()
        .to_owned();
    (card, text.contains("Oracle line to write: Whenever"))
}

fn cheap() -> Value {
    json!({"input_tokens": 100, "cache_creation_input_tokens": 0, "cache_read_input_tokens": 900, "output_tokens": 50})
}

fn message(text: &str, usage: &Value) -> Value {
    json!({
        "id": "msg_1", "type": "message", "role": "assistant", "stop_reason": "end_turn",
        "content": [{"type": "text", "text": text}],
        "usage": usage,
    })
}

fn config(server: &Server) -> AnthropicConfig {
    AnthropicConfig {
        base_url: server.url("/").as_str().trim_end_matches('/').to_owned(),
        attempts: 1,
        backoff: Duration::from_millis(1),
        timeout: Duration::from_secs(30),
    }
}

fn key() -> ApiKey {
    ApiKey::from_env(|_| Some(KEY.to_owned())).unwrap()
}

/// The answer to a request: (card, trigger line?, a repair?) to text.
type Answer = dyn Fn(&str, bool, bool) -> String + Send + Sync;

/// A mock of the Messages and Batches APIs: every request body is kept, a batch ends on its second look, and each
/// answer is billed `usage` (at first 100 new, 900 cached and 50 answer tokens).
struct Mock {
    usage: Mutex<Value>,
    messages: Mutex<Vec<Value>>,
    batches: Mutex<Vec<Vec<Value>>>,
    looks: AtomicUsize,
    healthy: AtomicBool,
}

impl Mock {
    fn new() -> Arc<Mock> {
        Arc::new(Mock {
            usage: Mutex::new(cheap()),
            messages: Mutex::new(Vec::new()),
            batches: Mutex::new(Vec::new()),
            looks: AtomicUsize::new(0),
            healthy: AtomicBool::new(true),
        })
    }

    fn messages(&self) -> Vec<Value> {
        self.messages
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn usage(&self) -> Value {
        self.usage
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn batches(&self) -> Vec<Vec<Value>> {
        self.batches
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    async fn serve(self: &Arc<Self>, answer: Arc<Answer>) -> Server {
        let mock = self.clone();
        start(move |seen: &Seen| {
            assert_eq!(seen.header("x-api-key"), Some(KEY));
            let body: Value = serde_json::from_slice(&seen.body).unwrap_or_default();
            let reply = |params: &Value| {
                let (card, trigger) = about(params);
                let repair = params["messages"].as_array().is_some_and(|m| m.len() > 1);
                answer(&card, trigger, repair)
            };
            let batch_of = |path: &str| {
                path.strip_prefix("/v1/messages/batches/")
                    .and_then(|rest| rest.split('/').next())
                    .and_then(|id| id.strip_prefix("msgbatch_"))
                    .and_then(|n| n.parse::<usize>().ok())
                    .unwrap_or(0)
            };
            let info = |n: usize, ended: bool| {
                let body = json!({
                    "id": format!("msgbatch_{n}"), "type": "message_batch",
                    "processing_status": if ended { "ended" } else { "in_progress" },
                    "request_counts": {"processing": 0, "succeeded": 0, "errored": 0, "canceled": 0, "expired": 0},
                });
                Reply::ok(body.to_string()).header("Content-Type", "application/json")
            };
            match (seen.method.as_str(), seen.path.as_str()) {
                ("POST", "/v1/messages") => {
                    let text = reply(&body);
                    mock.messages.lock().unwrap_or_else(PoisonError::into_inner).push(body);
                    Reply::ok(message(&text, &mock.usage()).to_string()).header("Content-Type", "application/json")
                }
                ("POST", "/v1/messages/batches") => {
                    let requests = body["requests"].as_array().cloned().unwrap_or_default();
                    let mut batches = mock.batches.lock().unwrap_or_else(PoisonError::into_inner);
                    batches.push(requests);
                    info(batches.len(), false)
                }
                ("GET", path) if path.ends_with("/results") => {
                    let batches = mock.batches.lock().unwrap_or_else(PoisonError::into_inner);
                    let lines: Vec<String> = batches[batch_of(path) - 1]
                        .iter()
                        .map(|request| {
                            json!({
                                "custom_id": request["custom_id"],
                                "result": {"type": "succeeded", "message": message(&reply(&request["params"]), &mock.usage())},
                            })
                            .to_string()
                        })
                        .collect();
                    Reply::ok(lines.join("\n")).header("Content-Type", "application/x-jsonl")
                }
                ("GET", path) => {
                    if !mock.healthy.load(Ordering::SeqCst) {
                        return Reply::status(400).body(
                            r#"{"type":"error","error":{"type":"invalid_request_error","message":"broken"}}"#,
                        );
                    }
                    let looks = mock.looks.fetch_add(1, Ordering::SeqCst);
                    info(batch_of(path), looks % 2 == 1)
                }
                _ => Reply::status(404),
            }
        })
        .await
    }
}

/// How the run asks.
enum Mode {
    Single,
    Batch,
}

async fn run(
    env: &Env,
    server: &Server,
    spend: Spend,
    mode: Mode,
    samples: usize,
    resume: bool,
) -> (Result<String, CliError>, Captured) {
    let (progress, captured) = Progress::buffer();
    let handle = tokio::runtime::Handle::current();
    let models = PaidModels::new(
        key(),
        config(server),
        handle.clone(),
        progress.clone(),
        Anthropic::default(),
        false,
    );
    let api = BatchApi::new(
        AnthropicClient::new(key(), &config(server), handle, progress.clone()).unwrap(),
        None,
    );
    let cards = env.cards.clone();
    let forge = ForgeHints::open(&env.root.path().join("cardsfolder")).unwrap();
    let translations = env.translations.clone();
    let names = env.names();
    let out = env.out();
    let task = tokio::task::spawn_blocking(move || {
        let library = mtg_cards::builtin::library().unwrap();
        let known = BTreeSet::new();
        let caps = mtg_engine::caps();
        let ctx = LineCtx {
            cards: &cards,
            library,
            names: &known,
            caps: &caps,
            forge: &forge,
            translations: &translations,
            model: MODEL,
            legal: None,
            pool: OnceLock::new(),
        };
        let options = LineOptions {
            samples,
            resume,
            meter: Meter { spend: Some(spend) },
        };
        match mode {
            Mode::Single => ctx.run(&models, &names, &out, options, &progress),
            Mode::Batch => {
                let settings = BatchSettings {
                    size: 100,
                    poll_every: Duration::from_millis(1),
                    schema: false,
                };
                ctx.run_batch(&api, &names, &out, options, &settings, &progress)
            }
        }
    });
    let result = tokio::time::timeout(Duration::from_secs(120), task)
        .await
        .unwrap_or_else(|_| panic!("the run hangs"))
        .unwrap();
    (result, captured)
}

fn all_text_under(dir: &Path) -> String {
    let mut text = String::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        for entry in fs::read_dir(&next).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                text.push_str(&fs::read_to_string(&path).unwrap_or_default());
            }
        }
    }
    text
}

fn ledger_labels(env: &Env) -> Vec<String> {
    fs::read_to_string(env.root.path().join("api-ledger.tsv"))
        .unwrap_or_default()
        .lines()
        .skip(1)
        .filter_map(|line| line.split('\t').nth(1).map(str::to_owned))
        .collect()
}

/// Each line answered: the trigger with a missing note, the draw with an ability.
fn plain() -> Arc<Answer> {
    Arc::new(|_, trigger, _| if trigger { TRIGGER } else { DRAW }.to_owned())
}

#[tokio::test(flavor = "multi_thread")]
async fn two_gapped_lines_go_out_as_two_cached_requests_and_come_back_as_one_card() {
    let env = Env::new(1);
    let mock = Mock::new();
    let server = mock.serve(plain()).await;
    let (result, captured) = run(
        &env,
        &server,
        env.spend(Usd(50_000_000), Billing::Standard),
        Mode::Single,
        1,
        false,
    )
    .await;
    let summary = result.unwrap();
    let sent = mock.messages();
    // Two lines, then two repair rounds of the mismatching card, both lines each time.
    assert_eq!(sent.len(), 6, "{summary}");
    assert_eq!(sent[0]["system"][0]["cache_control"]["type"], "ephemeral");
    assert!(
        sent[0]["system"][0]["text"]
            .as_str()
            .unwrap()
            .contains("triggered ability")
    );
    assert!(
        sent[1]["system"][0]["text"]
            .as_str()
            .unwrap()
            .contains("activated ability")
    );
    assert!(sent[0].get("output_config").is_none());
    let saved = all_text_under(&env.out());
    assert!(saved.contains("\"Activated\""), "the two parts are merged");
    let summary_tsv = fs::read_to_string(env.out().join("summary.tsv")).unwrap();
    assert!(
        summary_tsv.contains("Test Wyvern 01\tmismatch\t\t6\t"),
        "{summary_tsv}"
    );
    let labels = ledger_labels(&env);
    assert_eq!(labels, vec!["Test Wyvern 01".to_owned(); 6]);
    assert!(summary.contains("spent: $"), "{summary}");
    for text in [&saved, &captured.text(), &summary] {
        assert!(!text.contains(KEY));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_cap_stops_the_run_before_the_request_that_would_pass_it_and_resume_goes_on() {
    let env = Env::new(3);
    let mock = Mock::new();
    let server = mock.serve(Arc::new(|_, _, _| MISSING.to_owned())).await;
    // Each answer is billed about its worst case, and the cap has room for two and a half: the third request is refused.
    *mock.usage.lock().unwrap() = json!({"input_tokens": 0, "cache_creation_input_tokens": 3000, "cache_read_input_tokens": 0, "output_tokens": 4096});
    let probe = env.spend(Usd(u64::MAX), Billing::Standard);
    let cards = env.cards.clone();
    let forge = ForgeHints::open(&env.root.path().join("cardsfolder")).unwrap();
    let library = mtg_cards::builtin::library().unwrap();
    let known = BTreeSet::new();
    let caps = mtg_engine::caps();
    let ctx = LineCtx {
        cards: &cards,
        library,
        names: &known,
        caps: &caps,
        forge: &forge,
        translations: &env.translations,
        model: MODEL,
        legal: None,
        pool: OnceLock::new(),
    };
    let first = ctx.prepare("Test Wyvern 01").unwrap();
    let worst = probe
        .worst_case(&first.requests[0])
        .0
        .max(probe.worst_case(&first.requests[1]).0);
    drop(first);
    let cap = Usd(worst * 2 + worst / 2);
    let (result, _) = run(
        &env,
        &server,
        env.spend(cap, Billing::Standard),
        Mode::Single,
        1,
        false,
    )
    .await;
    let summary = result.unwrap();
    assert!(summary.contains("stopped: the spend cap"), "{summary}");
    assert_eq!(mock.messages().len(), 2, "{summary}");
    assert!(summary.contains("cannot express: 1"), "{summary}");
    let billed = env.ledger().total().unwrap();
    assert!(billed > 0 && u64::try_from(billed).unwrap() <= cap.0);

    let (result, _) = run(
        &env,
        &server,
        env.spend(Usd(50_000_000), Billing::Standard),
        Mode::Single,
        1,
        true,
    )
    .await;
    let summary = result.unwrap();
    assert_eq!(
        mock.messages().len(),
        6,
        "the two cards left are asked: {summary}"
    );
    assert!(summary.contains("cannot express: 3"), "{summary}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_line_batch_asks_samples_and_repairs_round_by_round_and_settles_each_batch_once() {
    let env = Env::new(2);
    let mock = Mock::new();
    // Card 01's trigger is no JSON at first and its draw is an ability, so the trigger alone is repaired; every other
    // answer says what is missing.
    let answer: Arc<Answer> = Arc::new(|card, trigger, repair| match (card, trigger, repair) {
        ("Test Wyvern 01", true, false) => "not json".to_owned(),
        ("Test Wyvern 01", false, _) => DRAW.to_owned(),
        _ => MISSING.to_owned(),
    });
    let server = mock.serve(answer).await;
    let (result, captured) = run(
        &env,
        &server,
        env.spend(Usd(50_000_000), Billing::Batch),
        Mode::Batch,
        2,
        false,
    )
    .await;
    let summary = result.unwrap();
    let batches = mock.batches();
    // Two cards, two lines, two samples; then card 01's trigger alone.
    assert_eq!(
        batches.iter().map(Vec::len).collect::<Vec<_>>(),
        [8, 1],
        "{summary}"
    );
    let ids: BTreeSet<&str> = batches[0]
        .iter()
        .filter_map(|r| r["custom_id"].as_str())
        .collect();
    assert!(
        ids.contains("w0-l0-s1") && ids.contains("w1-l1-s0"),
        "{ids:?}"
    );
    assert_eq!(
        batches[1][0]["params"]["messages"].as_array().map(Vec::len),
        Some(3)
    );
    assert!(summary.contains("cannot express: 2"), "{summary}");
    let summary_tsv = fs::read_to_string(env.out().join("summary.tsv")).unwrap();
    assert!(
        summary_tsv.contains("Test Wyvern 01\tinvalid"),
        "{summary_tsv}"
    );
    let lines: Vec<&str> = summary_tsv.lines().collect();
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("Test Wyvern 01") && l.ends_with("\tno")),
        "{summary_tsv}"
    );
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("Test Wyvern 02") && l.ends_with("\tyes")),
        "{summary_tsv}"
    );
    let labels = ledger_labels(&env);
    assert_eq!(labels.iter().filter(|l| l.contains("reserved")).count(), 2);
    assert_eq!(labels.iter().filter(|l| l.contains("settled")).count(), 2);
    // Nine answers at half price: 100 new tokens, 900 cached, 50 answered.
    let one = mtg_cli::propose::price::price_of(MODEL).unwrap().cost(
        &mtg_cli::propose::model::Usage {
            input_tokens: 1000,
            cache_write_tokens: 0,
            cache_read_tokens: 900,
            output_tokens: 50,
        },
        Billing::Batch,
    );
    assert_eq!(
        env.ledger().total().unwrap(),
        i64::try_from(one.0 * 9).unwrap()
    );
    assert!(!env.out().join(STATE_FILE).exists());
    assert!(!captured.text().contains(KEY));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_line_batch_that_is_out_is_collected_by_a_resumed_run_and_never_billed_twice() {
    let env = Env::new(2);
    let mock = Mock::new();
    mock.healthy.store(false, Ordering::SeqCst);
    let server = mock.serve(Arc::new(|_, _, _| MISSING.to_owned())).await;
    let (result, _) = run(
        &env,
        &server,
        env.spend(Usd(50_000_000), Billing::Batch),
        Mode::Batch,
        1,
        false,
    )
    .await;
    assert!(result.is_err(), "the status call fails");
    assert!(env.out().join(STATE_FILE).is_file());
    let held = env.ledger().total().unwrap();
    assert!(held > 0, "the batch's worst case is held while it is out");

    let (again, _) = run(
        &env,
        &server,
        env.spend(Usd(50_000_000), Billing::Batch),
        Mode::Batch,
        1,
        false,
    )
    .await;
    assert!(again.unwrap_err().to_string().contains("--resume"));

    mock.healthy.store(true, Ordering::SeqCst);
    let (result, _) = run(
        &env,
        &server,
        env.spend(Usd(50_000_000), Billing::Batch),
        Mode::Batch,
        1,
        true,
    )
    .await;
    let summary = result.unwrap();
    assert_eq!(
        mock.batches().len(),
        1,
        "no batch is asked again: {summary}"
    );
    assert!(summary.contains("cannot express: 2"), "{summary}");
    assert!(!env.out().join(STATE_FILE).exists());
    let labels = ledger_labels(&env);
    assert_eq!(labels.iter().filter(|l| l.contains("settled")).count(), 1);
    assert!(
        env.ledger().total().unwrap() < held,
        "the hold is given back"
    );
}
