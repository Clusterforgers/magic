//! Goal: line mode refuses a run without a local server, skips a card that does not have exactly one gap, changes only the
//! gapped line of a translation, repairs a bad answer once and then calls it invalid, and sends the schema of the kind.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;

use clap::Parser;
use mtg_cards::proposal::partial::LedgerLine;
use mtg_cards::{PrintedCard, PrintedFace};
use rstest::rstest;
use serde_json::{Value, json};

use super::part::{self, Kind};
use super::pool::{Gap, Pool, Translation, merge};
use super::*;
use crate::args::{Cli, Command};
use crate::propose::fake::ScriptedClient;

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

#[rstest]
#[case::anthropic(&["--lines", "--names-file", "n", "--translations", "t", "--forge-hint", "f"], false)]
#[case::no_base_url(&["--lines", "--names-file", "n", "--translations", "t", "--forge-hint", "f", "--provider", "openai", "--model", "m"], false)]
#[case::local(&["--lines", "--names-file", "n", "--translations", "t", "--forge-hint", "f", "--provider", "openai", "--model", "m", "--base-url", "http://localhost:11434/v1"], true)]
fn a_run_needs_a_local_server(#[case] flags: &[&str], #[case] allowed: bool) {
    assert_eq!(check_args(&args(flags)).is_ok(), allowed);
}

#[test]
fn a_run_without_a_base_url_says_so() {
    let flags = [
        "--lines",
        "--names-file",
        "n",
        "--translations",
        "t",
        "--forge-hint",
        "f",
        "--provider",
        "openai",
        "--model",
        "m",
    ];
    let error = check_args(&args(&flags)).unwrap_err().to_string();
    assert!(error.contains("--base-url"), "{error}");
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
#[case::one_gap_in_the_middle(vec![row("K", "Flying", &[0], None), row("T", "x", &[], Some("k")), row("K", "Trample", &[1], None)], Ok(1))]
#[case::grouped_bodies_are_one_line(vec![row("T", "x", &[], Some("k")), row("T", "x", &[], Some("k"))], Ok(0))]
#[case::no_gap(vec![row("K", "Flying", &[0], None)], Err("no gapped line"))]
#[case::two_gaps(vec![row("T", "x", &[], Some("k")), row("A", "y", &[], Some("j"))], Err("2 gapped lines"))]
#[case::same_gap_two_lines(vec![row("T", "x", &[], Some("k")), row("T", "y", &[], Some("k"))], Err("2 gapped lines"))]
#[case::half_translated(vec![row("K", "Flying", &[0], Some("k"))], Err("a line is partly translated"))]
fn a_card_needs_exactly_one_gapped_line(
    #[case] lines: Vec<LedgerLine>,
    #[case] expected: Result<usize, &str>,
) {
    let got = translation(lines).single_gap().map(|gap| gap.at);
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

#[test]
fn a_place_past_the_end_is_refused() {
    let proposal = json!({"faces": [{"abilities": ["a"]}]});
    assert!(merge(&proposal, 0, 2, json!(1)).is_none());
    assert!(merge(&proposal, 3, 0, json!(1)).is_none());
}

const ORACLE: &str =
    "Flying\nWhenever Test Drake attacks, exile the top card of each library face down.\nTrample";
const SCRIPT: &str = "Name:Test Drake\nManaCost:2 U\nTypes:Creature Drake\nPT:2/2\nK:Flying\n\
T:Mode$ Attacks | ValidCard$ Card.Self | Execute$ TrigDraw | TriggerDescription$ Whenever Test Drake attacks, exile the top card of each library face down.\n\
SVar:TrigDraw:DB$ Dig | Defined$ Player | DigNum$ 1\nSVar:AIPreference:x\nK:Trample\nOracle:x\n";

fn drake(name: &str) -> PrintedCard {
    PrintedCard {
        oracle_id: format!("00000000-0000-4000-8000-{:012}", name.len()),
        name: name.to_owned(),
        keywords: Vec::new(),
        layout: "normal".into(),
        faces: vec![PrintedFace {
            name: name.to_owned(),
            mana_cost: "{2}{U}".into(),
            type_line: "Creature — Drake".into(),
            oracle_text: ORACLE.into(),
            power: Some("2".into()),
            toughness: Some("2".into()),
            ..PrintedFace::default()
        }],
    }
}

fn write_file(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A translation of a drake whose attack trigger is the gap, with the extra gap rows `more` when given.
fn put_translation(dir: &Path, name: &str, more: Option<LedgerLine>) {
    let stem = slug(name);
    let abilities = json!([{"Keyword": "Flying"}, {"Keyword": "Trample"}]);
    write_file(
        &dir.join(format!("{stem}.json")),
        &json!({"faces": [{"abilities": abilities}]}).to_string(),
    );
    let mut lines = vec![
        row("K", "Flying", &[0], None),
        row(
            "T",
            "Whenever Test Drake attacks, exile the top card of each library face down.",
            &[],
            Some("effect:Dig.Exile"),
        ),
        row("K", "Trample", &[1], None),
    ];
    lines.extend(more);
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
        &SCRIPT.replace("Test Drake", "Test Wyvern"),
    );
    let translations = root.path().join("tr");
    put_translation(&translations, "Test Drake", None);
    put_translation(
        &translations,
        "Test Wyvern",
        Some(row("A", "Other.", &[], Some("zone:Other"))),
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

fn run(
    env: &Env,
    client: ScriptedClient,
    names: &[&str],
    resume: bool,
) -> (String, Arc<ScriptedClient>) {
    let cards = vec![drake("Test Drake"), drake("Test Wyvern")];
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
    };
    let client = Arc::new(client);
    let names: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
    let summary = ctx
        .run(
            &Stub(client.clone()),
            &names,
            &env.out,
            resume,
            &Progress::buffer().0,
        )
        .unwrap();
    (summary, client)
}

fn saved(out: &Path, stem: &str) -> Option<Value> {
    ["", "unchecked/", "rejected/"]
        .iter()
        .find_map(|dir| fs::read_to_string(out.join(format!("{dir}{stem}.json"))).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
}

#[test]
fn only_the_gapped_line_changes_in_a_run_and_the_part_is_marked_unread() {
    let env = env();
    let client = ScriptedClient::new()
        .answer("Test Drake", TRIGGER)
        .answer("Test Drake", TRIGGER);
    let (summary, client) = run(&env, client, &["Test Drake"], false);
    assert_eq!(client.requests().len(), 2, "one repair for the mismatch");
    let saved = saved(&env.out, "test_drake").unwrap();
    let abilities = saved["faces"][0]["abilities"].as_array().unwrap();
    assert_eq!(abilities.len(), 3, "{saved}");
    assert_eq!(abilities[0], json!({"Keyword": "Flying"}));
    assert_eq!(
        abilities[1]["Triggered"]["effects"][0],
        json!({"Sacrifice": {"filter": [{"Type": "Creature"}]}})
    );
    assert_eq!(abilities[2], json!({"Keyword": "Trample"}));
    assert!(summary.contains("asked: 1 cards"), "{summary}");
    assert!(summary.ends_with(SAFETY), "{summary}");
    let rows = read_results(&env.out.join(RESULTS));
    assert_eq!(rows.len(), 1);
    // Whatever the checker says, a kept file carries the unread source.
    let metas: Vec<Value> = ["meta", "unchecked/meta"]
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
fn bad_json_gets_one_repair_and_is_then_invalid() {
    let env = env();
    let client = ScriptedClient::new()
        .answer("Test Drake", "not json at all")
        .answer("Test Drake", r#"{"Triggered": 3}"#)
        .answer("Test Drake", TRIGGER);
    let (summary, client) = run(&env, client, &["Test Drake"], false);
    let requests = client.requests();
    assert_eq!(requests.len(), 2, "one repair and no more");
    assert_eq!(requests[1].messages.len(), 3);
    assert!(
        requests[1].messages[2]
            .text
            .contains("Reply with the corrected JSON")
    );
    assert!(summary.contains("invalid: 1"), "{summary}");
    assert!(env.out.join("rejected/test_drake.json").is_file());
}

#[test]
fn a_repair_turns_bad_json_into_a_judged_card() {
    let env = env();
    let client = ScriptedClient::new()
        .answer("Test Drake", "oops")
        .answer("Test Drake", TRIGGER);
    let (summary, client) = run(&env, client, &["Test Drake"], false);
    assert_eq!(client.requests().len(), 2);
    assert!(
        summary.contains("invalid: 0") && summary.contains("mismatch: 1"),
        "{summary}"
    );
    assert!(saved(&env.out, "test_drake").is_some());
}

#[test]
fn a_card_with_several_gaps_is_skipped_without_a_request() {
    let env = env();
    let client = ScriptedClient::new().answer("Test Wyvern", TRIGGER);
    let (summary, client) = run(&env, client, &["Test Wyvern", "Nobody"], false);
    assert_eq!(client.requests().len(), 0);
    assert!(summary.contains("skipped: 2"), "{summary}");
    let rows = read_results(&env.out.join(RESULTS));
    assert_eq!(rows[0].note, "2 gapped lines");
    assert_eq!(rows[1].note, "not in the database");
}

#[test]
fn a_resumed_run_skips_the_names_it_has_and_keeps_the_order() {
    let env = env();
    let first = ScriptedClient::new()
        .answer("Test Drake", TRIGGER)
        .answer("Test Drake", TRIGGER);
    run(&env, first, &["Test Drake"], false);
    let second = ScriptedClient::new().answer("Test Drake", TRIGGER);
    let (summary, client) = run(&env, second, &["Test Drake", "Nobody"], true);
    assert_eq!(client.requests().len(), 0);
    let names: Vec<String> = read_results(&env.out.join(RESULTS))
        .into_iter()
        .map(|row| row.name)
        .collect();
    assert_eq!(names, ["Test Drake", "Nobody"]);
    assert!(summary.contains("asked: 1 cards"), "{summary}");
}

#[test]
fn a_failed_request_is_a_card_without_an_answer() {
    let env = env();
    let client = ScriptedClient::new().failure("Test Drake", 500);
    let (summary, _) = run(&env, client, &["Test Drake"], false);
    assert!(summary.contains("no answer: 1"), "{summary}");
}

#[test]
fn the_prompt_is_small_and_holds_the_gap_the_script_and_the_shape() {
    let env = env();
    let client = ScriptedClient::new()
        .answer("Test Drake", TRIGGER)
        .answer("Test Drake", TRIGGER);
    let (_, client) = run(&env, client, &["Test Drake"], false);
    let request = &client.requests()[0];
    let text = &request.messages[0].text;
    assert!(
        request.estimated_tokens() < 3_000,
        "{}",
        request.estimated_tokens()
    );
    assert!(text.contains("Oracle line to write: Whenever Test Drake attacks"));
    assert!(text.contains("T:Mode$ Attacks"));
    assert!(text.contains("SVar:TrigDraw:DB$ Dig"));
    assert!(!text.contains("AIPreference"));
    assert!(text.contains("effect:Dig.Exile"));
    assert!(text.contains("\"Triggered\""));
    assert!(
        !text.contains("Trample"),
        "the rest of the card is not sent"
    );
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

#[test]
fn examples_are_translated_lines_of_the_same_kind_sharing_words() {
    let dir = tempfile::tempdir().unwrap();
    let mut table: BTreeMap<&str, (&str, &str, Value)> = BTreeMap::new();
    table.insert(
        "a",
        (
            "T",
            "Whenever this attacks, exile the top card of each library face down.",
            json!({"Triggered": {"when": "x", "effects": []}}),
        ),
    );
    table.insert(
        "b",
        (
            "T",
            "Whenever this attacks, exile a card.",
            json!({"Triggered": {"when": "y", "effects": []}}),
        ),
    );
    table.insert(
        "c",
        (
            "A",
            "Draw a card on attack.",
            json!({"Spell": {"effects": []}}),
        ),
    );
    table.insert(
        "d",
        (
            "T",
            "Gain life.",
            json!({"Triggered": {"when": "z", "effects": []}}),
        ),
    );
    for (stem, (tag, text, ability)) in &table {
        write_file(
            &dir.path().join(format!("{stem}.json")),
            &json!({"faces": [{"abilities": [ability]}]}).to_string(),
        );
        write_file(
            &dir.path().join("meta").join(format!("{stem}.json")),
            &json!({"lines": [row(tag, text, &[0], None)]}).to_string(),
        );
    }
    let pool = Pool::load(dir.path());
    assert_eq!(pool.len(), 4);
    let shown = pool.nearest(
        Kind::Triggered,
        "Whenever this attacks, exile the top card",
        3,
    );
    let texts: Vec<&str> = shown.iter().map(|one| one.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "Whenever this attacks, exile the top card of each library face down.",
            "Whenever this attacks, exile a card.",
        ]
    );
    let gap = Gap {
        oracle: "o".into(),
        forge: "f".into(),
        key: "k".into(),
        kind: Kind::Triggered,
    };
    let request = part::request("m", "Card", &gap, Kind::Triggered, &shown);
    assert!(
        request.messages[0]
            .text
            .contains("- Whenever this attacks, exile a card. => {\"Triggered\"")
    );
}
