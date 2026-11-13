//! Goal: every card the r40-8 pack made automatic reads back as its printed text clause by clause, and a near miss (another
//! zone, another player, another kind of land, a counter or zone changed) does not read back.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::ability::Ability;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
use mtg_cards::support::PrintedCard;
use rstest::rstest;

fn r408_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r408_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r408_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r408_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R408_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r408_probe() {
    if let Ok(json) = std::env::var("R408_JSON") {
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
    let Ok(list) = std::env::var("R408_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r408_report(name, &r408_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r408_file(name)).unwrap();
        for ability in &card.faces[0].abilities {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Firecat Blitz")]
#[case("Flash of Insight")]
#[case("Experimental Frenzy")]
#[case("Cornered Market")]
#[case("Discontinuity")]
#[case("Intervention Pact")]
#[case("Demonfire")]
#[case("Marina Vendrell's Grimoire")]
#[case("Garza's Assassin")]
#[case("Gor Muldrak, Amphinologist")]
#[case("Sekki, Seasons' Guide")]
#[case("Catalyst Stone")]
#[case("Day's Undoing")]
#[case("Mental Modulation")]
#[case("Murderous Betrayal")]
#[case("Pact of Negation")]
#[case("Pact of the Titan")]
#[case("Slaughter Pact")]
#[case("Sundial of the Infinite")]
#[case("Temur Battlecrier")]
#[case("Time Stop")]
#[case("Ultima")]
fn r408_reads_back_whole(#[case] name: &str) {
    let report = r408_report(name, &r408_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::lands_anywhere(
    "Experimental Frenzy",
    r#""PlayLands": [{ "In": "Hand" }]"#,
    r#""PlayLands": [{ "In": "Library" }]"#
)]
#[case::opponents_play(
    "Experimental Frenzy",
    r#""who": "You", "action": { "PlayLands""#,
    r#""who": "Opponents", "action": { "PlayLands""#
)]
#[case::spells_from_the_library(
    "Experimental Frenzy",
    r#""filter": [{ "In": "Hand" }]"#,
    r#""filter": [{ "In": "Library" }]"#
)]
#[case::basic_lands(
    "Cornered Market",
    r#"{ "Not": { "Supertype": "Basic" } }"#,
    r#"{ "Supertype": "Basic" }"#
)]
#[case::token_names(
    "Cornered Market",
    r#"{ "SharesName": { "Each": [{ "Not": "Token" }] } }"#,
    r#"{ "SharesName": { "Each": ["Token"] } }"#
)]
#[case::sacrifices_lands(
    "Firecat Blitz",
    r#"{ "Subtype": "Mountain" }"#,
    r#"{ "Subtype": "Forest" }"#
)]
#[case::other_turns(
    "Discontinuity",
    r#""condition": "YourTurn""#,
    r#""condition": { "Not": "YourTurn" }"#
)]
#[case::less_reduction("Discontinuity", "{2}{U}{U}", "{1}{U}{U}")]
#[case::pact_price("Intervention Pact", "{1}{W}{W}", "{2}{W}{W}")]
#[case::loses_life("Intervention Pact", "GainLife", "LoseLife")]
#[case::one_card_in_hand("Demonfire", r#""right": 0"#, r#""right": 1"#)]
#[case::draws_on_loss("Marina Vendrell's Grimoire", "Discard", "Draw")]
#[case::rounded_down("Garza's Assassin", r#""up": true"#, r#""up": false"#)]
#[case::most_creatures("Gor Muldrak, Amphinologist", r#""op": "Le""#, r#""op": "Ge""#)]
#[case::other_counters(
    "Sekki, Seasons' Guide",
    r#""counter": "+1/+1", "count": "Triggering""#,
    r#""counter": "charge", "count": "Triggering""#
)]
#[case::two_counters_each(
    "Sekki, Seasons' Guide",
    r#""count": "Triggering" }"#,
    r#""count": 2 }"#
)]
#[case::escape_costs(
    "Catalyst Stone",
    r#"{ "CastWith": "Flashback" }"#,
    r#"{ "CastWith": "Escape" }"#
)]
#[case::three_less("Catalyst Stone", r#""mana": "{2}""#, r#""mana": "{3}""#)]
#[case::red_cards("Flash of Insight", r#""Blue""#, r#""Red""#)]
fn r408_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r408_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r408_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}
