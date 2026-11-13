//! Goal: every card the r37-2 pack made fully automatic reads back as its printed text clause by clause, the compiler
//! reads its new wordings into the same rules, and a near miss (another count, another kind of counter, suspend gained
//! unconditionally where the card says "if it doesn't have suspend", a land found) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r372_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r372_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r372_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
    let cards = common::printed();
    let tokens = Library::from_sources(&common::token_sources()).unwrap();
    let card = cards
        .iter()
        .find(|card| card.name == name)
        .unwrap_or_else(|| panic!("the corpus has {name}"));
    check(
        text,
        card,
        &tokens,
        &r372_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R372_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r372_probe() {
    if let Ok(json) = std::env::var("R372_JSON") {
        for line in json.split('\n') {
            let ability: Ability = serde_json::from_str(line).unwrap();
            println!("-- {line}");
            for text in mtg_cards::compile::written_variants(&ability)
                .iter()
                .take(4)
            {
                println!("  written: {text}");
            }
        }
    }
    let Ok(list) = std::env::var("R372_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r372_report(name, &r372_file(name));
        println!("== {name}: {:?}", report.verdict());
        for problem in &report.problems {
            println!("  problem: {problem:?}");
        }
        for clause in &report.clauses {
            println!("  [{}] {} => {:?}", clause.face, clause.text, clause.state);
        }
        for stray in &report.strays {
            println!("  stray: {stray:?}");
        }
        let card: mtg_cards::card::Card = serde_json::from_str(&r372_file(name)).unwrap();
        for ability in &card.faces[0].abilities {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Epochrasite")]
#[case("The Tenth Doctor")]
#[case("The Parting of the Ways")]
#[case("The Eleventh Doctor")]
#[case("Rory Williams")]
#[case("Delay")]
#[case("Taigam, Master Opportunist")]
#[case("Amy Pond")]
fn r372_reads_back_whole(#[case] name: &str) {
    let report = r372_report(name, &r372_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::another_count("Epochrasite", r#""count": 3"#, r#""count": 4"#)]
#[case::another_counter("Epochrasite", r#"{ "Other": "time" }"#, r#"{ "Other": "fade" }"#)]
#[case::gains_it_unconditionally("The Tenth Doctor", r#", "unless_has": true"#, "")]
#[case::gains_it_conditionally(
    "Epochrasite",
    r#"{ "GainSuspend": { "what": { "Bound": "exiled" } } }"#,
    r#"{ "GainSuspend": { "what": { "Bound": "exiled" }, "unless_has": true } }"#
)]
#[case::a_land_found(
    "The Tenth Doctor",
    r#"{ "Not": { "Type": "Land" } }"#,
    r#"{ "Type": "Land" }"#
)]
#[case::four_cards("The Parting of the Ways", r#""count": 5"#, r#""count": 4"#)]
#[case::by_power(
    "The Parting of the Ways",
    r#""stat": "ManaValue""#,
    r#""stat": "Power""#
)]
#[case::by_toughness(
    "The Eleventh Doctor",
    r#""stat": "ManaValue""#,
    r#""stat": "Toughness""#
)]
#[case::always_gained("The Eleventh Doctor", r#", "unless_has": true"#, "")]
#[case::not_from_hand(
    "Rory Williams",
    r#"{ "WasCast": { "from": "Exile" } }"#,
    r#"{ "WasCast": { "from": "Hand" } }"#
)]
#[case::from_anywhere(
    "Rory Williams",
    r#", { "Not": { "WasCast": { "from": "Exile" } } }"#,
    ""
)]
#[case::two_counters("Delay", r#""count": 3"#, r#""count": 2"#)]
#[case::suspend_regardless("Delay", r#", "unless_has": true"#, "")]
#[case::third_spell("Taigam, Master Opportunist", r#""nth": 2"#, r#""nth": 3"#)]
#[case::five_counters("Taigam, Master Opportunist", r#""count": 4"#, r#""count": 5"#)]
#[case::any_suspended_card("Amy Pond", r#"{ "OwnedBy": "You" }, "#, "")]
#[case::one_counter("Amy Pond", r#""count": "Triggering""#, r#""count": 1"#)]
fn r372_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r372_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r372_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}

/// The compiler reads a wording alone into the steps the card's file holds for it (the head "Whenever you attack" is
/// written from the data only, so the steps of The Tenth Doctor are read under another head).
#[rstest]
#[case::dies_into_suspend(
    "Epochrasite",
    "When this creature dies, exile it with three time counters on it and it gains suspend.",
    1
)]
#[case::found_and_suspended(
    "The Tenth Doctor",
    "When this creature enters, exile cards from the top of your library until you exile a nonland card. Put three time counters on it. If it doesn't have suspend, it gains suspend.",
    0
)]
#[case::countered_into_suspend(
    "Delay",
    "Counter target spell. If the spell is countered this way, exile it with three time counters on it instead of putting it into its owner's graveyard. If it doesn't have suspend, it gains suspend.",
    0
)]
#[case::copied_then_suspended(
    "Taigam, Master Opportunist",
    "Flurry \u{2014} Whenever you cast your second spell each turn, copy it, then exile the spell you cast with four time counters on it. If it doesn't have suspend, it gains suspend.",
    0
)]
fn r372_compiler_reads_the_wording(#[case] name: &str, #[case] text: &str, #[case] ability: usize) {
    let mut card = common::printed()
        .into_iter()
        .find(|card| card.name == name)
        .unwrap();
    card.faces[0].oracle_text = text.to_owned();
    let compiled = mtg_cards::compile::compile(&card);
    let read: Vec<Ability> = compiled
        .card
        .map(|card| card.faces[0].abilities.clone())
        .unwrap_or_default();
    let file: mtg_cards::card::Card = serde_json::from_str(&r372_file(name)).unwrap();
    let [read] = read.as_slice() else {
        panic!("{name}: {read:?}");
    };
    assert_eq!(
        read.effects(),
        file.faces[0].abilities[ability].effects(),
        "{name}"
    );
}
