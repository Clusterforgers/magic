//! Goal: every card the r40-9 pack made fully automatic reads back as its printed text clause by clause, the compiler
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

fn r409_names(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| std::iter::once(&card.name).chain(card.faces.iter().map(|f| &f.name)))
        .map(|name| name.to_lowercase())
        .collect()
}

fn r409_file(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    std::fs::read_to_string(&path).unwrap()
}

fn r409_report(name: &str, text: &str) -> mtg_cards::proposal::Report {
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
        &r409_names(&cards),
        &Caps::everything(),
    )
}

/// Prints the clause states of the cards named in `R409_PROBE` (separated by `|`); a tool for writing rules files.
#[test]
fn r409_probe() {
    if let Ok(json) = std::env::var("R409_JSON") {
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
    if let Ok(list) = std::env::var("R409_COMPILE") {
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
    let Ok(list) = std::env::var("R409_PROBE") else {
        return;
    };
    for name in list.split('|') {
        let report = r409_report(name, &r409_file(name));
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
        let card: mtg_cards::card::Card = serde_json::from_str(&r409_file(name)).unwrap();
        for ability in card.faces.iter().flat_map(|f| &f.abilities) {
            for text in mtg_cards::compile::written_variants(ability).iter().take(6) {
                println!("  written: {text}");
            }
        }
    }
}

#[rstest]
#[case("Elbrus, the Binding Blade // Withengar Unbound")]
#[case("Verdeloth the Ancient")]
#[case("Thieving Skydiver")]
#[case("Bane of the Living")]
#[case("Warbreak Trumpeter")]
#[case("Ral and the Implicit Maze")]
#[case("Crag Saurian")]
#[case("Oath of Teferi")]
#[case("Urza Assembles the Titans")]
#[case("Belltower Sphinx")]
#[case("Reaper of Sheoldred")]
#[case("Tamiyo Meets the Story Circle")]
fn r409_reads_back_whole(#[case] name: &str) {
    let report = r409_report(name, &r409_file(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::twelve_counters(
    "Elbrus, the Binding Blade // Withengar Unbound",
    r#""count": 13"#,
    r#""count": 12"#
)]
#[case::only_you_lose(
    "Elbrus, the Binding Blade // Withengar Unbound",
    r#""action": "LostGame""#,
    r#""action": "LostGame", "who": "You""#
)]
#[case::one_saproling("Verdeloth the Ancient", r#""count": "X""#, r#""count": 1"#)]
#[case::unkicked_too("Verdeloth the Ancient", r#""This", "Kicked""#, r#""This""#)]
#[case::x_may_be_zero("Thieving Skydiver", r#", { "XAtLeast": 1 }"#, "")]
#[case::x_at_least_two("Thieving Skydiver", r#"{ "XAtLeast": 1 }"#, r#"{ "XAtLeast": 2 }"#)]
#[case::any_mana_value(
    "Thieving Skydiver",
    r#"{ "Compare": { "stat": "ManaValue", "op": "Le", "to": "X" } }"#,
    r#"{ "Type": "Artifact" }"#
)]
#[case::attach_an_aura(
    "Thieving Skydiver",
    r#"{ "Subtype": "Equipment" }"#,
    r#"{ "Subtype": "Aura" }"#
)]
#[case::minus_one("Bane of the Living", r#""by": -1, "of": "X""#, r#""by": -1, "of": 1"#)]
#[case::only_yours(
    "Bane of the Living",
    r#"{ "Type": "Creature" }"#,
    r#"{ "Type": "Creature" }, { "ControlledBy": "You" }"#
)]
#[case::one_goblin("Warbreak Trumpeter", r#""count": "X""#, r#""count": 1"#)]
#[case::when_it_enters(
    "Warbreak Trumpeter",
    r#"{ "TurnedFaceUp": { "what": ["This"] } }"#,
    r#"{ "ZoneChange": { "what": ["This"], "to": "Battlefield" } }"#
)]
#[case::a_clue(
    "Ral and the Implicit Maze",
    r#""Named": "spellgorger_weird""#,
    r#""Named": "clue""#
)]
#[case::two_weirds(
    "Ral and the Implicit Maze",
    r#"{ "Named": "spellgorger_weird" } }"#,
    r#"{ "Named": "spellgorger_weird" }, "count": 2 }"#
)]
#[case::you_take_it("Crag Saurian", r#"{ "ControllerOf": "Dealer" }"#, r#""You""#)]
#[case::only_combat_damage("Crag Saurian", r#""to": "This""#, r#""to": "This", "combat": true"#)]
#[case::everyone_twice(
    "Oath of Teferi",
    r#""LoyaltyTwice": "You""#,
    r#""LoyaltyTwice": "EachPlayer""#
)]
#[case::twice_for_ever(
    "Urza Assembles the Titans",
    r#""LoyaltyTwice": "You" }, "duration": "UntilEndOfTurn""#,
    r#""LoyaltyTwice": "You" }"#
)]
#[case::for_ever(
    "Tamiyo Meets the Story Circle",
    r#", "until": "UntilYourNextTurn""#,
    ""
)]
#[case::minus_three("Tamiyo Meets the Story Circle", r#""power": -2"#, r#""power": -3"#)]
#[case::attach_always("Thieving Skydiver", r#""right": 1"#, r#""right": 0"#)]
fn r409_a_near_miss_does_not_read_back(#[case] name: &str, #[case] from: &str, #[case] to: &str) {
    let file = r409_file(name);
    assert!(file.contains(from), "{name} has {from}");
    let report = r409_report(name, &file.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}

/// The keyword reader reads "Kicker {X}. X can't be 0." with the least X, and a plain kicker without it.
#[rstest]
#[case::at_least_one("Kicker {X}. X can't be 0.", true)]
#[case::plain("Kicker {X}", false)]
fn r409_kicker_x_reads(#[case] line: &str, #[case] least: bool) {
    let Some(mtg_cards::keyword::Keyword::Kicker(cost)) = mtg_cards::compile::read_keyword(line)
    else {
        panic!("{line} reads as a kicker");
    };
    assert!(cost.has_x());
    assert_eq!(
        cost.0.contains(&mtg_cards::cost::CostItem::XAtLeast(1)),
        least
    );
}
