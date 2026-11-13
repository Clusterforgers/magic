//! Goal: every card the r55-18 pack made fully automatic reads back as its printed text clause by clause, and a near
//! miss (another zone, seat, die row or shared quality) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r5518_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r5518_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r5518_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r5518_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R5518_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r5518_probe() {
    if let Ok(json) = std::env::var("R5518_JSON") {
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
    if let Ok(list) = std::env::var("R5518_COMPILE") {
        for name in list.split('|') {
            let card = common::printed()
                .into_iter()
                .find(|card| card.name == name)
                .unwrap();
            let compiled = mtg_cards::compile::compile(&card);
            println!("== {name}");
            for clause in &compiled.clauses {
                println!("  clause read={} {}", clause.read, clause.text);
            }
            if let Some(card) = compiled.card {
                for face in &card.faces {
                    for ability in &face.abilities {
                        println!("  {}", serde_json::to_string(ability).unwrap());
                    }
                }
            }
        }
    }
    let Ok(list) = std::env::var("R5518_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r5518_report(name, &r5518_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r5518_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
            for (at, row) in mtg_cards::compile::die_row_texts(ability)
                .iter()
                .enumerate()
            {
                println!("  row {at}: {:?}", row.iter().take(3).collect::<Vec<_>>());
            }
        }
    }
}

#[rstest]
#[case("Niambi, Faithful Healer")]
#[case("Teferi, Timebender")]
#[case("Mathise, Surge Channeler")]
#[case("Druid of the Emerald Grove")]
#[case("Iron Mastiff")]
#[case("Druid Class")]
#[case("Adagia, Windswept Bastion")]
#[case("Spirit-Sister's Call")]
#[case("Sphinx of the Chimes")]
#[case("Illuminated Folio")]
fn r5518_reads_back_whole(#[case] name: &str) {
    let report = r5518_report(name, &r5518_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::niambi_library_only("Niambi, Faithful Healer", r#""also": ["Graveyard"],"#, "")]
#[case::niambi_another_name(
    "Niambi, Faithful Healer",
    "Teferi, Timebender",
    "Teferi, Hero of Dominaria"
)]
#[case::mathise_you_draw(
    "Mathise, Surge Channeler",
    r#"{ "Draw": { "who": "EachPlayer" } }"#,
    r#"{ "Draw": {} }"#
)]
#[case::mathise_rows_shifted("Mathise, Surge Channeler", r#""to": 9"#, r#""to": 8"#)]
#[case::druid_untapped(
    "Druid of the Emerald Grove",
    r#"{ "Battlefield": { "tapped": true } }"#,
    r#""Battlefield""#
)]
#[case::druid_or_less_from_two("Druid of the Emerald Grove", r#""from": 1"#, r#""from": 2"#)]
#[case::mastiff_one_die("Iron Mastiff", r#""highest": { "Players": "Attacked" },"#, "")]
#[case::mastiff_defending_not_you("Iron Mastiff", r#""to": "You""#, r#""to": "Defending""#)]
#[case::druid_class_without_haste("Druid Class", r#"{ "AddKeyword": "Haste" },"#, "")]
#[case::druid_class_lands_of_anyone(
    "Druid Class",
    r#"{ "Type": "Land" }, { "ControlledBy": "You" }]"#,
    r#"{ "Type": "Land" }]"#
)]
#[case::adagia_eleven("Adagia, Windswept Bastion", r#""right": 12"#, r#""right": 11"#)]
#[case::adagia_not_legendary(
    "Adagia, Windswept Bastion",
    r#""except": [{ "AddSupertype": "Legendary" }]"#,
    r#""except": []"#
)]
#[case::call_any_permanent(
    "Spirit-Sister's Call",
    r#"{ "SharesType": { "Target": 0 } }"#,
    r#"{ "Type": "Creature" }"#
)]
#[case::call_not_exiled(
    "Spirit-Sister's Call",
    r#"{ "MoveTo": "Exile" }"#,
    r#"{ "MoveTo": "Hand" }"#
)]
#[case::sphinx_share_color(
    "Sphinx of the Chimes",
    r#""alike": "SameName""#,
    r#""alike": "ShareColor""#
)]
#[case::sphinx_lands_too(
    "Sphinx of the Chimes",
    r#""filter": [{ "Not": { "Type": "Land" } }],"#,
    ""
)]
#[case::folio_three("Illuminated Folio", r#""count": 2"#, r#""count": 3"#)]
#[case::folio_discarded(
    "Illuminated Folio",
    r#"{ "AlikeFromHand": { "count""#,
    r#"{ "AlikeFromHand": { "discard": true, "count""#
)]
fn r5518_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r5518_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r5518_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}

#[rstest]
#[case::same_name("Discard two nonland cards with the same name: Draw four cards.", true)]
#[case::share_color(
    "Reveal two cards from your hand that share a color: Draw a card.",
    true
)]
#[case::reveal_same_name(
    "Reveal two cards from your hand with the same name: Draw a card.",
    false
)]
#[case::one_card("Discard a nonland card with the same name: Draw four cards.", false)]
fn r5518_the_compiler_reads_cards_alike_as_a_cost(#[case] line: &str, #[case] read: bool) {
    let cards = common::printed();
    let mut card = cards
        .into_iter()
        .find(|card| card.name == "Sphinx of the Chimes")
        .unwrap();
    if let Some(face) = card.faces.first_mut() {
        face.oracle_text = line.to_owned();
    }
    let compiled = mtg_cards::compile::compile(&card);
    let found = compiled.clauses.iter().any(|clause| clause.read)
        && serde_json::to_string(&compiled.card)
            .unwrap()
            .contains("AlikeFromHand");
    assert_eq!(found, read, "{line}: {:?}", compiled.clauses);
}
