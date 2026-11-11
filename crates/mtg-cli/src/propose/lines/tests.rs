//! Goal: line mode takes a local server with its address or the paid API, works on cards with one or two gapped lines and
//! no other, changes only the gapped lines of a translation, repairs a rejected card at most twice with the checker's
//! words, keeps a `missing` answer as a failure with its note, picks the best of several samples and records whether they
//! agree, skips a card that is not Commander-legal, finds examples by the shape of the script line, and sends the schema
//! of the kind.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use clap::Parser;
use mtg_cards::load::slug;
use mtg_cards::proposal::partial::LedgerLine;
use mtg_cards::{PrintedCard, PrintedFace};
use rstest::rstest;
use serde_json::{Value, json};

use super::part::{self, Kind};
use super::pool::{Gap, Pool, Shape, Translation, merge, merge_all};
use super::*;
use crate::args::{Cli, Command};
use crate::propose::fake::ScriptedClient;
use crate::propose::summary::CardRecord;

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

const LINE_FLAGS: [&str; 7] = [
    "--lines",
    "--names-file",
    "n",
    "--translations",
    "t",
    "--forge-hint",
    "f",
];

#[rstest]
#[case::anthropic(&[], true)]
#[case::anthropic_batch(&["--batch", "--max-spend", "5"], true)]
#[case::no_base_url(&["--provider", "openai", "--model", "m"], false)]
#[case::local(&["--provider", "openai", "--model", "m", "--base-url", "http://localhost:11434/v1"], true)]
#[case::local_batch(&["--provider", "openai", "--model", "m", "--base-url", "http://x", "--batch"], false)]
fn a_run_takes_the_paid_api_or_a_local_server_with_its_address(
    #[case] flags: &[&str],
    #[case] allowed: bool,
) {
    let mut all = LINE_FLAGS.to_vec();
    all.extend_from_slice(flags);
    assert_eq!(check_args(&args(&all)).is_ok(), allowed);
}

#[test]
fn samples_take_one_to_five() {
    let mut all = LINE_FLAGS.to_vec();
    all.extend_from_slice(&["--samples", "3"]);
    assert_eq!(args(&all).samples, 3);
    for bad in ["0", "6"] {
        let mut all = LINE_FLAGS.to_vec();
        all.extend_from_slice(&["--samples", bad]);
        assert!(Cli::try_parse_from(["mtg-cli", "propose"].iter().chain(&all)).is_err());
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

fn translation(lines: Vec<LedgerLine>) -> Translation {
    Translation {
        proposal: json!({"faces": [{"abilities": []}]}),
        lines,
    }
}

#[rstest]
#[case::one_gap_in_the_middle(vec![row("K", "Flying", &[0], None), row("T", "x", &[], Some("k")), row("K", "Trample", &[1], None)], Ok(vec![1]))]
#[case::grouped_bodies_are_one_line(vec![row("T", "x", &[], Some("k")), row("T", "x", &[], Some("k"))], Ok(vec![0]))]
#[case::two_gaps(vec![row("K", "Flying", &[0], None), row("T", "x", &[], Some("k")), row("A", "y", &[], Some("j")), row("K", "Trample", &[1], None)], Ok(vec![1, 1]))]
#[case::same_key_two_lines(vec![row("T", "x", &[], Some("k")), row("T", "y", &[], Some("k"))], Ok(vec![0, 0]))]
#[case::three_gaps(vec![row("T", "x", &[], Some("k")), row("A", "y", &[], Some("j")), row("S", "z", &[], Some("i"))], Err("3 gapped lines"))]
#[case::no_gap(vec![row("K", "Flying", &[0], None)], Err("no gapped line"))]
#[case::half_translated(vec![row("K", "Flying", &[0], Some("k"))], Err("a line is partly translated"))]
fn a_card_needs_one_or_two_gapped_lines(
    #[case] lines: Vec<LedgerLine>,
    #[case] expected: Result<Vec<usize>, &str>,
) {
    let got = translation(lines)
        .gaps()
        .map(|gaps| gaps.iter().map(|gap| gap.at).collect::<Vec<_>>());
    assert_eq!(got, expected.map_err(str::to_owned));
}

#[rstest]
#[case::front(0, json!([{"N": 1}, "a", "b", "c"]))]
#[case::middle(2, json!(["a", "b", {"N": 1}, "c"]))]
#[case::end(3, json!(["a", "b", "c", {"N": 1}]))]
fn only_the_gapped_line_changes(#[case] at: usize, #[case] expected: Value) {
    let proposal = json!({"faces": [{"abilities": ["a", "b", "c"]}, {"abilities": ["z"]}]});
    let merged = merge(&proposal, 0, at, json!({"N": 1})).unwrap();
    assert_eq!(merged["faces"][0]["abilities"], expected);
    assert_eq!(merged["faces"][1], proposal["faces"][1]);
}

#[rstest]
#[case::same_place_keeps_order(vec![(0, 1, json!("X")), (0, 1, json!("Y"))], json!(["a", "X", "Y", "b"]), json!(["z"]))]
#[case::places_count_in_the_original(vec![(0, 0, json!("X")), (0, 2, json!("Y"))], json!(["X", "a", "b", "Y"]), json!(["z"]))]
#[case::given_out_of_order(vec![(0, 2, json!("Y")), (0, 0, json!("X"))], json!(["X", "a", "b", "Y"]), json!(["z"]))]
#[case::two_faces(vec![(1, 0, json!("Y")), (0, 1, json!("X"))], json!(["a", "X", "b"]), json!(["Y", "z"]))]
fn two_gapped_lines_go_to_their_own_places(
    #[case] parts: Vec<(usize, usize, Value)>,
    #[case] front: Value,
    #[case] back: Value,
) {
    let proposal = json!({"faces": [{"abilities": ["a", "b"]}, {"abilities": ["z"]}]});
    let merged = merge_all(&proposal, parts).unwrap();
    assert_eq!(merged["faces"][0]["abilities"], front);
    assert_eq!(merged["faces"][1]["abilities"], back);
}

#[test]
fn a_place_past_the_end_is_refused() {
    let proposal = json!({"faces": [{"abilities": ["a"]}]});
    assert!(merge(&proposal, 0, 2, json!(1)).is_none());
    assert!(merge(&proposal, 3, 0, json!(1)).is_none());
}

const ATTACKS: &str = "Whenever Test Drake attacks, exile the top card of each library face down.";
const ORACLE: &str =
    "Flying\nWhenever Test Drake attacks, exile the top card of each library face down.\nTrample";
const SCRIPT: &str = "Name:Test Drake\nManaCost:2 U\nTypes:Creature Drake\nPT:2/2\nK:Flying\n\
T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ TrigDraw | TriggerDescription$ Whenever Test Drake attacks, exile the top card of each library face down.\n\
SVar:TrigDraw:DB$ Dig | Defined$ Player | DigNum$ 1\nSVar:AIPreference:x\nK:Trample\nOracle:x\n";
const TAP: &str = "{T}: Draw a card.";
/// A sentence no reader knows, so that the compiler leaves the owl to line mode.
const ZORBLAX: &str = "Frobnicate the zorblax twice.";
const TAP_LINE: &str = "A:AB$ Draw | Cost$ T | NumCards$ 1 | SpellDescription$ {T}: Draw a card.\n";

fn printed(name: &str, oracle: &str) -> PrintedCard {
    PrintedCard {
        oracle_id: format!("00000000-0000-4000-8000-{:012}", name.len()),
        name: name.to_owned(),
        keywords: Vec::new(),
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            mana_cost: "{2}{U}".into(),
            type_line: "Creature — Drake".into(),
            oracle_text: oracle.into(),
            power: Some("2".into()),
            toughness: Some("2".into()),
            ..PrintedFace::default()
        }],
    }
}

fn cards() -> Vec<PrintedCard> {
    vec![
        printed("Test Drake", ORACLE),
        printed(
            "Test Wyvern",
            &format!(
                "Flying\n{}\n{TAP}\nTrample",
                ATTACKS.replace("Drake", "Wyvern")
            ),
        ),
        printed("Test Owl", &format!("Flying\n{TAP}\n{ZORBLAX}")),
    ]
}

fn write_file(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn put(dir: &Path, name: &str, abilities: &Value, lines: &[LedgerLine]) {
    let stem = slug(name);
    write_file(
        &dir.join(format!("{stem}.json")),
        &json!({"faces": [{"abilities": abilities}]}).to_string(),
    );
    write_file(
        &dir.join("meta").join(format!("{stem}.json")),
        &json!({"source": "forge", "lines": lines}).to_string(),
    );
}

struct Env {
    _root: tempfile::TempDir,
    translations: std::path::PathBuf,
    forge: ForgeHints,
    out: std::path::PathBuf,
}

fn env() -> Env {
    let root = tempfile::tempdir().unwrap();
    let forge_dir = root.path().join("cardsfolder");
    write_file(&forge_dir.join("t/test_drake.txt"), SCRIPT);
    write_file(
        &forge_dir.join("t/test_wyvern.txt"),
        &format!("{}{TAP_LINE}", SCRIPT.replace("Test Drake", "Test Wyvern")),
    );
    write_file(
        &forge_dir.join("t/test_owl.txt"),
        &format!("Name:Test Owl\nManaCost:2 U\nTypes:Creature Bird\nPT:2/2\nK:Flying\n{TAP_LINE}"),
    );
    let translations = root.path().join("tr");
    let flying_trample = json!([{"Keyword": "Flying"}, {"Keyword": "Trample"}]);
    put(
        &translations,
        "Test Drake",
        &flying_trample,
        &[
            row("K", "Flying", &[0], None),
            row("T", ATTACKS, &[], Some("effect:Dig.Exile")),
            row("K", "Trample", &[1], None),
        ],
    );
    put(
        &translations,
        "Test Wyvern",
        &flying_trample,
        &[
            row("K", "Flying", &[0], None),
            row(
                "T",
                &ATTACKS.replace("Drake", "Wyvern"),
                &[],
                Some("effect:Dig.Exile"),
            ),
            row("A", TAP, &[], Some("cost:Tap")),
            row("K", "Trample", &[1], None),
        ],
    );
    put(
        &translations,
        "Test Owl",
        &json!([{"Keyword": "Flying"}]),
        &[
            row("K", "Flying", &[0], None),
            row("A", TAP, &[], Some("cost:Tap")),
        ],
    );
    Env {
        forge: ForgeHints::open(&forge_dir).unwrap(),
        out: root.path().join("out"),
        translations,
        _root: root,
    }
}

struct Stub(Arc<ScriptedClient>);

impl LineModels for Stub {
    fn client(&self, _: Kind) -> Result<Arc<dyn ModelClient>, CliError> {
        Ok(self.0.clone())
    }
}

const TRIGGER: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"effects":[{"Sacrifice":{"filter":[{"Type":"Creature"}]}}]}}"#;
const DRAW: &str = r#"{"Activated":{"cost":["Tap"],"effects":[{"Draw":{}}]}}"#;
/// The same ability spelled with its keys in another order: the same canonical JSON.
const DRAW_REORDERED: &str = r#"{"Activated":{"effects":[{"Draw":{}}],"cost":["Tap"]}}"#;
const MISSING: &str = r#"{"missing": "an exile face down that each player may not look at"}"#;

struct Ran {
    summary: String,
    client: Arc<ScriptedClient>,
}

fn run_with(
    env: &Env,
    client: ScriptedClient,
    names: &[&str],
    resume: bool,
    samples: usize,
    legal: Option<&BTreeSet<String>>,
) -> Ran {
    let cards = cards();
    let library = mtg_cards::builtin::library().unwrap();
    let known = BTreeSet::new();
    let caps = mtg_engine::caps();
    let ctx = LineCtx {
        cards: &cards,
        library,
        names: &known,
        caps: &caps,
        forge: &env.forge,
        translations: &env.translations,
        model: "stub",
        legal,
        pool: OnceLock::new(),
    };
    let client = Arc::new(client);
    let names: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
    let options = LineOptions {
        samples,
        resume,
        meter: paid::Meter::default(),
    };
    let summary = ctx
        .run(
            &Stub(client.clone()),
            &names,
            &env.out,
            options,
            &Progress::buffer().0,
        )
        .unwrap();
    Ran { summary, client }
}

fn run(env: &Env, client: ScriptedClient, names: &[&str], resume: bool) -> Ran {
    run_with(env, client, names, resume, 1, None)
}

fn saved(out: &Path, stem: &str) -> Option<Value> {
    ["", "drafts/", "rejected/"]
        .iter()
        .find_map(|dir| fs::read_to_string(out.join(format!("{dir}{stem}.json"))).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
}

fn record(out: &Path, stem: &str) -> CardRecord {
    let text = fs::read_to_string(out.join("records").join(format!("{stem}.json"))).unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn only_the_gapped_line_changes_in_a_run_and_the_part_is_marked_unread() {
    let env = env();
    let client = ScriptedClient::new()
        .answer("Test Drake", TRIGGER)
        .answer("Test Drake", TRIGGER)
        .answer("Test Drake", TRIGGER);
    let ran = run(&env, client, &["Test Drake"], false);
    assert_eq!(
        ran.client.requests().len(),
        3,
        "two repairs for the mismatch"
    );
    let saved = saved(&env.out, "test_drake").unwrap();
    let abilities = saved["faces"][0]["abilities"].as_array().unwrap();
    assert_eq!(abilities.len(), 3, "{saved}");
    assert_eq!(abilities[0], json!({"Keyword": "Flying"}));
    assert_eq!(
        abilities[1]["Triggered"]["effects"][0],
        json!({"Sacrifice": {"filter": [{"Type": "Creature"}]}})
    );
    assert_eq!(abilities[2], json!({"Keyword": "Trample"}));
    assert!(ran.summary.contains("asked: 1 cards"), "{}", ran.summary);
    assert!(ran.summary.ends_with(SAFETY), "{}", ran.summary);
    assert_eq!(read_results(&env.out.join(RESULTS)).len(), 1);
    let summary = fs::read_to_string(env.out.join("summary.tsv")).unwrap();
    assert!(
        summary.starts_with("card\toutcome\tmissing_engine_piece"),
        "{summary}"
    );
    // Whatever the checker says, a kept file carries the unread source.
    let metas: Vec<Value> = ["meta", "drafts/meta"]
        .iter()
        .filter_map(|dir| fs::read_to_string(env.out.join(dir).join("test_drake.json")).ok())
        .filter_map(|text| serde_json::from_str(&text).ok())
        .collect();
    for meta in metas {
        assert_eq!(meta["source"], "sweep");
        assert_eq!(meta["written_by"], "model-line");
    }
}

#[test]
fn a_repair_round_carries_the_checkers_words_and_there_are_at_most_two() {
    let env = env();
    let client = ScriptedClient::new()
        .answer("Test Drake", TRIGGER)
        .answer("Test Drake", TRIGGER)
        .answer("Test Drake", TRIGGER)
        .answer("Test Drake", TRIGGER);
    let ran = run(&env, client, &["Test Drake"], false);
    let requests = ran.client.requests();
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests
            .iter()
            .map(|r| r.messages.len())
            .collect::<Vec<_>>(),
        [1, 3, 5]
    );
    let repair = &requests[1].messages[2].text;
    assert!(repair.contains("fails the checker"), "{repair}");
    assert!(repair.contains("Reply with the corrected JSON"), "{repair}");
    assert!(ran.summary.contains("mismatch: 1"), "{}", ran.summary);
    assert_eq!(record(&env.out, "test_drake").requests, 3);
}

#[test]
fn bad_json_is_repaired_twice_and_is_then_invalid() {
    let env = env();
    let client = ScriptedClient::new()
        .answer("Test Drake", "not json at all")
        .answer("Test Drake", r#"{"Triggered": 3}"#)
        .answer("Test Drake", "still not")
        .answer("Test Drake", TRIGGER);
    let ran = run(&env, client, &["Test Drake"], false);
    let requests = ran.client.requests();
    assert_eq!(requests.len(), 3, "two repairs and no more");
    assert!(requests[2].messages[4].text.contains("not an ability"));
    assert!(ran.summary.contains("invalid: 1"), "{}", ran.summary);
    assert!(env.out.join("rejected/test_drake.json").is_file());
}

#[test]
fn a_repair_turns_bad_json_into_a_judged_card() {
    let env = env();
    let client = ScriptedClient::new()
        .answer("Test Owl", "oops")
        .answer("Test Owl", DRAW);
    let ran = run(&env, client, &["Test Owl"], false);
    assert_eq!(ran.client.requests().len(), 2);
    let saved = saved(&env.out, "test_owl").unwrap();
    assert_eq!(
        saved["faces"][0]["abilities"][1],
        serde_json::from_str::<Value>(DRAW).unwrap()
    );
    assert!(!ran.summary.contains("invalid: 1"), "{}", ran.summary);
}

#[test]
fn a_missing_answer_is_a_failure_with_its_note_and_no_repair() {
    let env = env();
    let client = ScriptedClient::new().answer("Test Drake", MISSING);
    let ran = run(&env, client, &["Test Drake"], false);
    assert_eq!(ran.client.requests().len(), 1);
    assert!(ran.summary.contains("cannot express: 1"), "{}", ran.summary);
    let record = record(&env.out, "test_drake");
    assert_eq!(record.outcome, "invalid");
    assert_eq!(
        record.missing_engine_piece,
        ["an exile face down that each player may not look at"]
    );
}

#[test]
fn two_gapped_lines_are_asked_apart_and_merged_into_one_card() {
    let env = env();
    let client = ScriptedClient::new()
        .answer("Test Wyvern", TRIGGER)
        .answer("Test Wyvern", DRAW)
        .answer("Test Wyvern", TRIGGER)
        .answer("Test Wyvern", DRAW)
        .answer("Test Wyvern", TRIGGER)
        .answer("Test Wyvern", DRAW);
    let ran = run(&env, client, &["Test Wyvern"], false);
    let requests = ran.client.requests();
    assert!(
        requests[0].messages[0]
            .text
            .contains("Oracle line to write: Whenever Test Wyvern attacks")
    );
    assert!(
        requests[1].messages[0]
            .text
            .contains("Oracle line to write: {T}: Draw a card.")
    );
    assert!(requests[0].system.contains("triggered ability"));
    assert!(requests[1].system.contains("activated ability"));
    let saved = saved(&env.out, "test_wyvern").unwrap();
    let abilities = saved["faces"][0]["abilities"].as_array().unwrap();
    assert_eq!(abilities.len(), 4, "{saved}");
    assert!(abilities[1].get("Triggered").is_some(), "{saved}");
    assert!(abilities[2].get("Activated").is_some(), "{saved}");
    assert_eq!(abilities[3], json!({"Keyword": "Trample"}));
}

#[test]
fn a_bad_line_alone_is_repaired_and_the_good_one_stands() {
    let env = env();
    // The trigger answer is no JSON; the draw is fine and is not asked again.
    let client = ScriptedClient::new()
        .answer("Test Wyvern", "nope")
        .answer("Test Wyvern", DRAW)
        .answer("Test Wyvern", "nope again")
        .answer("Test Wyvern", "and again");
    let ran = run(&env, client, &["Test Wyvern"], false);
    let requests = ran.client.requests();
    assert_eq!(requests.len(), 4);
    for repair in &requests[2..] {
        assert!(
            repair.messages[0].text.contains("attacks"),
            "only the bad line is repaired"
        );
    }
}

#[test]
fn samples_that_agree_are_marked_and_the_best_sample_is_kept() {
    let env = env();
    let client = ScriptedClient::new()
        .answer("Test Owl", DRAW)
        .answer("Test Owl", DRAW_REORDERED)
        .answer("Test Owl", DRAW);
    let ran = run_with(&env, client, &["Test Owl"], false, 3, None);
    assert_eq!(
        ran.client.requests().len(),
        3,
        "three samples, nothing to repair: {}",
        ran.summary
    );
    assert_eq!(record(&env.out, "test_owl").agree, Some(true));
    let summary = fs::read_to_string(env.out.join("summary.tsv")).unwrap();
    assert!(
        summary.lines().nth(1).unwrap().ends_with("\tyes"),
        "{summary}"
    );

    let env = self::env();
    let client = ScriptedClient::new()
        .answer("Test Owl", "nope")
        .answer("Test Owl", DRAW);
    let ran = run_with(&env, client, &["Test Owl"], false, 2, None);
    assert_eq!(
        ran.client.requests().len(),
        2,
        "the good sample needs no repair"
    );
    assert_eq!(record(&env.out, "test_owl").agree, Some(false));
    assert!(saved(&env.out, "test_owl").is_some());
}

#[test]
fn a_card_with_more_than_two_gaps_or_out_of_commander_is_skipped_without_a_request() {
    let env = env();
    put(
        &env.translations,
        "Test Drake",
        &json!([]),
        &[
            row("T", "a", &[], Some("k")),
            row("A", "b", &[], Some("j")),
            row("S", "c", &[], Some("i")),
        ],
    );
    let legal: BTreeSet<String> = cards()
        .iter()
        .filter(|card| card.name != "Test Owl")
        .map(|card| card.oracle_id.clone())
        .collect();
    let client = ScriptedClient::new().answer("Test Owl", DRAW);
    let ran = run_with(
        &env,
        client,
        &["Test Drake", "Nobody", "Test Owl"],
        false,
        1,
        Some(&legal),
    );
    assert_eq!(ran.client.requests().len(), 0);
    assert!(ran.summary.contains("skipped: 3"), "{}", ran.summary);
    let notes: Vec<String> = read_results(&env.out.join(RESULTS))
        .into_iter()
        .map(|row| row.note)
        .collect();
    assert_eq!(
        notes,
        [
            "3 gapped lines",
            "not in the database",
            "not Commander-legal"
        ]
    );
}

#[test]
fn a_resumed_run_skips_the_names_it_has_and_keeps_the_order() {
    let env = env();
    let first = ScriptedClient::new().answer("Test Drake", MISSING);
    run(&env, first, &["Test Drake"], false);
    let second = ScriptedClient::new().answer("Test Drake", TRIGGER);
    let ran = run(&env, second, &["Test Drake", "Nobody"], true);
    assert_eq!(ran.client.requests().len(), 0);
    let names: Vec<String> = read_results(&env.out.join(RESULTS))
        .into_iter()
        .map(|row| row.name)
        .collect();
    assert_eq!(names, ["Test Drake", "Nobody"]);
    assert!(ran.summary.contains("asked: 1 cards"), "{}", ran.summary);
}

#[test]
fn a_failed_request_is_a_card_without_an_answer() {
    let env = env();
    let client = ScriptedClient::new().failure("Test Drake", 500);
    let ran = run(&env, client, &["Test Drake"], false);
    assert!(ran.summary.contains("no answer: 1"), "{}", ran.summary);
}

#[test]
fn the_prompt_is_small_and_holds_the_card_the_gap_the_script_and_the_proven_lines() {
    let env = env();
    let client = ScriptedClient::new().answer("Test Drake", MISSING);
    let ran = run(&env, client, &["Test Drake"], false);
    let request = &ran.client.requests()[0];
    let text = &request.messages[0].text;
    assert!(
        request.estimated_tokens() < 8_000,
        "{}",
        request.estimated_tokens()
    );
    assert!(text.contains("Oracle text of the card:\nFlying\n"));
    assert!(text.contains("Oracle line to write: Whenever Test Drake attacks"));
    assert!(text.contains("T:Mode$ Attacks"));
    assert!(text.contains("SVar:TrigDraw:DB$ Dig"));
    assert!(!text.contains("AIPreference"));
    assert!(text.contains("effect:Dig.Exile"));
    assert!(
        text.contains(r#"face 0: [{"Keyword":"Flying"},{"Keyword":"Trample"}]"#),
        "{text}"
    );
    assert!(request.system.contains("\"Triggered\""));
    assert!(request.system.contains("{\"missing\""));
}

#[rstest]
#[case::trigger(Kind::Triggered, "Triggered")]
#[case::spell(Kind::Spell, "Spell")]
#[case::activated(Kind::Activated, "Activated")]
#[case::replacement(Kind::Replacement, "Replacement")]
#[case::keyword(Kind::Keyword, "Keyword")]
fn the_schema_of_a_kind_is_that_variant_alone(#[case] kind: Kind, #[case] key: &str) {
    let schema = part::schema(kind);
    assert_eq!(schema["required"], json!([key]), "{schema}");
    assert_eq!(schema["additionalProperties"], json!(false));
    let defs = schema["$defs"].as_object().unwrap();
    let full = mtg_cards::reference::w3a_card_schema();
    assert!(defs.len() < full["$defs"].as_object().unwrap().len() || kind != Kind::Keyword);
    assert!(!defs.contains_key("Card") && !defs.contains_key("Face"));
}

fn objects_closed(value: &Value) -> bool {
    match value {
        Value::Object(map) => {
            (!map.contains_key("properties")
                || map.get("additionalProperties") == Some(&json!(false)))
                && map.values().all(objects_closed)
        }
        Value::Array(items) => items.iter().all(objects_closed),
        _ => true,
    }
}

#[test]
fn a_structured_schema_is_flat_closed_and_takes_a_missing_note() {
    let flat = json!({
        "type": "object",
        "properties": {"Draw": {"$ref": "#/$defs/Draw"}},
        "required": ["Draw"],
        "$defs": {"Draw": {"type": "object", "properties": {"count": {"type": "integer", "minimum": 0}, "who": {"type": "string", "maxLength": 9}}}},
    });
    let schema = part::structured_from(flat).unwrap();
    let text = schema.to_string();
    for word in ["\"minimum\"", "\"maxLength\""] {
        assert!(!text.contains(word), "{word}");
    }
    assert!(text.contains("\"missing\"") && text.contains("\"$defs\""));
    assert!(objects_closed(&schema));
    let recursive = json!({
        "type": "object",
        "properties": {"Seq": {"$ref": "#/$defs/Effect"}},
        "$defs": {"Effect": {"anyOf": [{"type": "string"}, {"type": "array", "items": {"$ref": "#/$defs/Effect"}}]}},
    });
    assert!(part::structured_from(recursive).is_none());
}

#[test]
fn the_static_kind_offers_every_rule_variant() {
    let schema = part::schema(Kind::Static);
    let keys: BTreeSet<&str> = schema["anyOf"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|option| option["required"][0].as_str())
        .collect();
    assert!(keys.contains("Static") && keys.contains("Rule"), "{keys:?}");
}

#[rstest]
#[case::spell("A", "SP$ Draw | NumCards$ 1", Kind::Spell)]
#[case::activated("A", "AB$ Draw | Cost$ T", Kind::Activated)]
#[case::loyalty(
    "A",
    "AB$ Draw | Cost$ AddCounter<1/LOYALTY> | Planeswalker$ True",
    Kind::Loyalty
)]
#[case::trigger("T", "Mode$ Attacks", Kind::Triggered)]
#[case::static_("S", "Mode$ Continuous", Kind::Static)]
#[case::replacement("R", "Event$ Moved", Kind::Replacement)]
#[case::keyword("K", "Flying", Kind::Keyword)]
fn the_kind_comes_from_the_script_line(#[case] tag: &str, #[case] body: &str, #[case] kind: Kind) {
    assert_eq!(Kind::of(tag, body), kind);
}

#[rstest]
#[case::nothing("nope", "not JSON")]
#[case::not_an_ability(r#"{"Triggered": 3}"#, "not an ability")]
fn an_answer_that_is_no_ability_says_why(#[case] answer: &str, #[case] why: &str) {
    match part::read_reply(answer) {
        part::Reply::Bad(text) => assert!(text.contains(why), "{text}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_missing_note_and_an_ability_are_read_from_an_answer() {
    assert_eq!(
        part::read_reply(&format!("```json\n{MISSING}\n```")),
        part::Reply::Missing("an exile face down that each player may not look at".into())
    );
    assert!(matches!(part::read_reply(DRAW), part::Reply::Ability(_)));
    // A `missing` beside other fields is no note.
    assert!(matches!(
        part::read_reply(r#"{"missing": "x", "Draw": {}}"#),
        part::Reply::Bad(_)
    ));
}

#[test]
fn the_shape_of_a_script_line_names_its_verbs_and_keys() {
    let shape = Shape::of(
        "T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ TrigDig\nSVar:TrigDig:DB$ Dig | DigNum$ 1",
    );
    let other = Shape::of("T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ X\nSVar:X:DB$ Draw");
    let same = Shape::of("T:Mode$ Blocks | Execute$ Y\nSVar:Y:DB$ Dig | DigNum$ 2");
    assert_ne!(shape, Shape::default());
    assert_ne!(shape, other);
    assert_ne!(same, other);
}

#[test]
fn examples_are_translated_lines_of_the_same_kind_nearest_by_script_shape_then_words() {
    let dir = tempfile::tempdir().unwrap();
    let forge_dir = dir.path().join("cardsfolder");
    let mut table: BTreeMap<&str, (&str, &str, Value, &str)> = BTreeMap::new();
    // Same words as the gap but another verb.
    table.insert(
        "Word Twin",
        (
            "T",
            "Whenever this attacks, exile the top card of each library face down.",
            json!({"Triggered": {"when": "x", "effects": []}}),
            "T:Mode$ Attacks | Execute$ TrigA | TriggerDescription$ Whenever this attacks, exile the top card of each library face down.\nSVar:TrigA:DB$ ChangeZone | Origin$ Library",
        ),
    );
    // The same verb with other words.
    table.insert(
        "Verb Twin",
        (
            "T",
            "When this enters, look at cards.",
            json!({"Triggered": {"when": "y", "effects": []}}),
            "T:Mode$ ChangesZone | Execute$ TrigB | TriggerDescription$ When this enters, look at cards.\nSVar:TrigB:DB$ Dig | DigNum$ 3",
        ),
    );
    table.insert(
        "Spell Card",
        (
            "A",
            "Draw a card on attack.",
            json!({"Spell": {"effects": []}}),
            "A:SP$ Draw | SpellDescription$ Draw a card on attack.",
        ),
    );
    let mut printed_cards = Vec::new();
    for (name, (tag, text, ability, script)) in &table {
        let stem = slug(name);
        write_file(
            &dir.path().join(format!("{stem}.json")),
            &json!({"faces": [{"abilities": [ability]}]}).to_string(),
        );
        write_file(
            &dir.path().join("meta").join(format!("{stem}.json")),
            &json!({"lines": [row(tag, text, &[0], None)]}).to_string(),
        );
        write_file(
            &forge_dir.join(format!("{}/{stem}.txt", &stem[..1])),
            &format!("Name:{name}\nTypes:Creature\n{script}\n"),
        );
        printed_cards.push(printed(name, text));
    }
    let forge = ForgeHints::open(&forge_dir).unwrap();
    let pool = Pool::load_with(dir.path(), Some(&forge), &printed_cards);
    assert_eq!(pool.len(), 3);
    let gap_shape = Shape::of("T:Mode$ Attacks | Execute$ T\nSVar:T:DB$ Dig | DigNum$ 1");
    let shown = pool.nearest(
        Kind::Triggered,
        &gap_shape,
        "Whenever Some Name attacks, exile the top card",
        "Some Name",
        3,
    );
    let texts: Vec<&str> = shown.iter().map(|one| one.text.as_str()).collect();
    // `Mode$ Attacks` is shared by the first; the second shares `Dig` and `DigNum`; both have a verb in common, and the
    // shared keys break the tie before the words do.
    assert_eq!(texts.len(), 2, "{texts:?}");
    assert!(
        texts.contains(&"When this enters, look at cards."),
        "{texts:?}"
    );
    // Without scripts the words alone rank them.
    let plain = Pool::load(dir.path());
    let shown = plain.nearest(
        Kind::Triggered,
        &gap_shape,
        "Whenever this attacks, exile a card",
        "x",
        3,
    );
    assert_eq!(
        shown.first().map(|one| one.text.as_str()),
        Some("Whenever this attacks, exile the top card of each library face down.")
    );
    let gap = Gap {
        oracle: "o".into(),
        forge: "f".into(),
        key: "k".into(),
        kind: Kind::Triggered,
    };
    let proven = json!({"faces": [{"abilities": []}]});
    let about = part::Card {
        name: "Card",
        oracle: "o",
        proven: &proven,
    };
    let request = part::request("m", about, &gap, &shown);
    assert!(request.messages[0].text.contains(
        "- Whenever this attacks, exile the top card of each library face down. => {\"Triggered\""
    ));
}

#[test]
fn a_schema_goes_into_output_config_beside_the_effort() {
    let request = crate::propose::model::Request {
        model: "m".into(),
        system: "s".into(),
        messages: vec![crate::propose::model::Message::user("u")],
        max_tokens: 10,
    };
    let schema = json!({"type": "object"});
    let body = paid::line_body(&request, Some("low"), Some(&schema));
    assert_eq!(body["output_config"]["effort"], "low");
    assert_eq!(body["output_config"]["format"]["type"], "json_schema");
    assert_eq!(body["output_config"]["format"]["schema"], schema);
    let plain = paid::line_body(&request, None, None);
    assert!(plain.get("output_config").is_none());
    // Effects nest effects, which structured outputs do not take: such a kind is read by the checker alone.
    assert!(part::structured(Kind::Triggered).is_none());
    assert!(paid::format_of(Kind::Triggered, true).is_none());
    assert!(paid::format_of(Kind::Keyword, false).is_none());
}

#[rstest]
#[case::trigger(Kind::Triggered)]
#[case::spell(Kind::Spell)]
#[case::activated(Kind::Activated)]
#[case::static_(Kind::Static)]
#[case::replacement(Kind::Replacement)]
#[case::keyword(Kind::Keyword)]
fn the_reference_of_a_line_holds_its_kind_and_the_named_verbs_within_the_budget(
    #[case] kind: Kind,
) {
    let gap = Gap {
        oracle: "Whenever this attacks, draw a card and exile it.".into(),
        forge: "T:Mode$ Attacks | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw | SubAbility$ DBExile\nSVar:DBExile:DB$ ChangeZone".into(),
        key: "k".into(),
        kind,
    };
    let words = part::focus_words(&gap, &[]);
    assert!(words.contains("changezone") && words.contains("change") && words.contains("draw"));
    let reference = mtg_cards::reference::dsl_reference_focused(
        kind.variants(),
        &words,
        part::REFERENCE_BUDGET,
    );
    assert!(
        reference.len() <= part::REFERENCE_BUDGET,
        "{}",
        reference.len()
    );
    assert!(reference.len() * 3 < mtg_cards::reference::dsl_reference().len());
    for variant in kind.variants() {
        assert!(
            reference.contains(&format!("\n  {variant}")),
            "{kind:?} {variant}"
        );
    }
    if matches!(kind, Kind::Triggered | Kind::Spell | Kind::Activated) {
        assert!(reference.contains("enum Effect"), "{reference}");
        assert!(reference.contains("\n  Draw"), "the named verb is shown");
    }
}
