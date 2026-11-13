//! Goal: every card the r55-05 pack made automatic reads back as its printed text clause by clause, a near miss of
//! each new piece does not, and the validator refuses each new node outside the shapes the engine runs.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::compile::{ClauseKind, w11d_room_clause};
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{ClauseState, Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn report_of(name: &str, text: &str) -> mtg_cards::proposal::Report {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    check(text, card, &tokens, &names(&cards), &Caps::everything())
}

fn file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

/// A rules file of the corpus with one piece of its compact JSON replaced.
fn altered(name: &str, from: &str, to: &str) -> String {
    let compact: serde_json::Value = serde_json::from_str(&file(name)).unwrap();
    let compact = serde_json::to_string(&compact).unwrap();
    assert!(compact.contains(from), "{name} holds {from}");
    compact.replacen(from, to, 1)
}

const FLAMEWAR: &str = "Flamewar, Brash Veteran // Flamewar, Streetwise Operative";
const JETFIRE: &str = "Jetfire, Ingenious Scientist // Jetfire, Air Guardian";

#[rstest]
#[case(FLAMEWAR)]
#[case(JETFIRE)]
#[case("Cruel Entertainment")]
#[case("The Apprentice's Folly")]
fn reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::two_counters_before_converting(
    FLAMEWAR,
    r#"{"PutCounters":{"counter":"+1/+1","on":"This"}}"#,
    r#"{"PutCounters":{"count":2,"counter":"+1/+1","on":"This"}}"#
)]
#[case::no_convert(
    FLAMEWAR,
    r#",{"Transform":{"what":"This"}}],"timing""#,
    r#"],"timing""#
)]
#[case::noncreature_mana(
    JETFIRE,
    r#"{"Spell":{"types":["Artifact"]}}"#,
    r#"{"Spell":{"types":["Creature"]}}"#
)]
#[case::mana_for_spells_alone(JETFIRE, r#",{"Ability":{}},"OtherCosts""#, "")]
#[case::adapt_without_converting(
    JETFIRE,
    r#"{"Transform":{"what":"This"}},{"Adapt""#,
    r#"{"Adapt""#
)]
#[case::one_way_control(
    "Cruel Entertainment",
    r#"{"ControlPlayer":{"by":{"Target":1},"who":{"Target":0}}}"#,
    r#"{"ControlPlayer":{"who":{"Target":0}}}"#
)]
#[case::a_second_target_that_may_repeat(
    "Cruel Entertainment",
    r#"{"what":{"Another":{"earlier":[0],"what":"Player"}}}"#,
    r#"{"what":"Player"}"#
)]
#[case::any_name(
    "The Apprentice's Folly",
    r#",{"Not":{"SharesName":{"Each":["Token",{"ControlledBy":"You"}]}}}"#,
    ""
)]
#[case::no_haste("The Apprentice's Folly", r#",{"AddKeyword":"Haste"}"#, "")]
fn a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let report = report_of(name, &altered(name, from, to));
    assert_ne!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {to}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::convert_as_a_sorcery(
    "{2}: Convert ~. Activate only as a sorcery.",
    ClauseKind::Activated,
    &[r#"{"Transform":{"what":"This"}}"#, r#""timing":"Sorcery""#]
)]
#[case::transform_on_attack(
    "Whenever ~ attacks, transform ~.",
    ClauseKind::Triggered,
    &[r#"{"Transform":{"what":"This"}}"#]
)]
#[case::convert_on_attack(
    "Whenever ~ attacks, convert ~.",
    ClauseKind::Triggered,
    &[r#"{"Transform":{"what":"This"}}"#]
)]
#[case::nonartifact_mana(
    "{T}: Add {C}. This mana can't be spent to cast nonartifact spells.",
    ClauseKind::Activated,
    &[r#"{"Spell":{"types":["Artifact"]}},{"Ability":{}},"OtherCosts""#]
)]
#[case::noncreature_mana(
    "{T}: Add {C}. This mana can't be spent to cast noncreature spells.",
    ClauseKind::Activated,
    &[r#"{"Spell":{"types":["Creature"]}},{"Ability":{}},"OtherCosts""#]
)]
fn the_compiler_reads_convert_and_type_bound_mana(
    #[case] text: &str,
    #[case] kind: ClauseKind,
    #[case] parts: &[&str],
) {
    let ability = w11d_room_clause(text, kind).unwrap_or_else(|| panic!("{text} is read"));
    let json = serde_json::to_string(&ability).unwrap();
    for part in parts {
        assert!(json.contains(part), "{text}: {json} holds {part}");
    }
}

#[rstest]
#[case("This mana can't be spent to cast nonsnow spells.")]
#[case("This mana can't be spent to cast nonblue spells.")]
fn a_spend_sentence_of_no_card_type_is_not_read(#[case] sentence: &str) {
    let text = format!("{{T}}: Add {{C}}. {sentence}");
    assert!(
        w11d_room_clause(&text, ClauseKind::Activated).is_none(),
        "{text}"
    );
}

/// Prints how each clause of the cards named in `R5505_PROBE` (`;`-separated) reads back.
#[test]
#[ignore = "a probe run by hand"]
fn probe() {
    let Ok(list) = std::env::var("R5505_PROBE") else {
        return;
    };
    let scratch = std::env::var("R5505_DIR").unwrap_or_default();
    for name in list.split(';') {
        let candidate = std::path::Path::new(&scratch).join(format!("{}.json", slug(name)));
        let text = if let Ok(text) = std::fs::read_to_string(&candidate) {
            text
        } else if common::cards_dir()
            .join(format!("{}.json", slug(name)))
            .exists()
        {
            file(name)
        } else {
            let card = common::printed()
                .into_iter()
                .find(|card| card.name == name)
                .unwrap();
            let compiled = mtg_cards::compile::compile(&card).card.unwrap();
            serde_json::to_string(&compiled).unwrap()
        };
        if std::env::var("R5505_JSON").is_ok_and(|v| !v.is_empty()) {
            let value: serde_json::Value = serde_json::from_str(&text).unwrap();
            for face in value["faces"].as_array().into_iter().flatten() {
                for ability in face["abilities"].as_array().into_iter().flatten() {
                    println!("   {ability}");
                }
            }
        }
        let report = report_of(name, &text);
        println!(
            "== {name}: {:?} problems {:?}",
            report.verdict(),
            report.problems
        );
        for clause in &report.clauses {
            match &clause.state {
                ClauseState::Reads { .. } => println!("  ok   {}", clause.text),
                ClauseState::Differs { written, .. } => {
                    println!("  DIFF {}\n       {written}", clause.text);
                }
                other => println!("  --   {} :: {other:?}", clause.text),
            }
        }
    }
}
