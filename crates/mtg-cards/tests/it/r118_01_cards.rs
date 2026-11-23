//! Goal: every card the r118-01 pack made fully automatic reads back as its printed text clause by clause, and a near miss
//! of each (another number, player, duration or filter) does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeSet;

use crate::common;

use mtg_cards::Caps;
use mtg_cards::load::{Library, slug};
use mtg_cards::proposal::{Verdict, check};
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

/// The rules file of a corpus card as compact JSON.
fn compact(name: &str) -> String {
    let path = common::cards_dir().join(format!("{}.json", slug(name)));
    let text = std::fs::read_to_string(&path).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    serde_json::to_string(&value).unwrap()
}

#[rstest]
#[case("70,000 Light-Years from Home")]
#[case("Captain Jean-Luc Picard")]
#[case("Captain Kirk, Boldly Going")]
#[case("Deep Space 9")]
#[case("Deep Space Scan")]
#[case("Ensign Pavel Chekov")]
#[case("Gabriel Lorca, Mirror Menace")]
#[case("H.M.S. Bounty, Unlikely Timeship")]
#[case("Jett Reno, Problem Solver")]
#[case("Jean-Luc Picard, Luminary")]
#[case("Julian Bashir, Frontier Doctor")]
#[case("Kirk, Man of Action")]
#[case("K't'inga-Class Battle Cruiser")]
#[case("Lieutenant Nyota Uhura")]
#[case("Maular, the Next Evolution")]
#[case("Peace in Our Time")]
#[case("Pure Energy")]
#[case("Queen's Diamond")]
#[case("Rite of Ascension")]
#[case("Species 8472")]
#[case("S.S. Botany Bay")]
#[case("Starship Enterprise")]
#[case("Stray from History")]
#[case("Sulu, Space Swashbuckler")]
#[case("Tell Me What You See")]
#[case("The Borg Queen")]
#[case("The Caretaker")]
#[case("The City on the Edge of Forever")]
#[case("The Destruction of Praxis")]
#[case("Transwarp Hub")]
#[case("Unification")]
#[case("U.S.S. Defiant, Mirror Migrant")]
#[case("U.S.S. Enterprise-C, Time-Tossed")]
#[case("U.S.S. Prometheus, Prototype")]
#[case("U.S.S. Stargazer, Here and There")]
#[case("U.S.S. Voyager, Lone Wanderer")]
#[case("Vic Fontaine, One Night Only")]
#[case("Zagorka, Mother of Sanctum")]
fn r118_01_reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &compact(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::attacks_anyone("Zagorka, Mother of Sanctum", r#""you":true"#, r#""you":false"#)]
#[case::their_turn(
    "Zagorka, Mother of Sanctum",
    r#""TheirNextTurn""#,
    r#""UntilEndOfTurn""#
)]
#[case::only_you_out_of_reach(
    "Peace in Our Time",
    r#""R118Attack":{}"#,
    r#""R118Attack":{"you":true}"#
)]
#[case::eleven_counters("Peace in Our Time", r#""right":12"#, r#""right":11"#)]
#[case::extra_turns_begun(
    "H.M.S. Bounty, Unlikely Timeship",
    r#"{"Not":{"R118BegunExtraTurn":"You"}}"#,
    r#"{"R118BegunExtraTurn":"You"}"#
)]
#[case::rest_on_top(
    "Lieutenant Nyota Uhura",
    r#""place":"Bottom""#,
    r#""place":"Top""#
)]
#[case::creature_spell(
    "Lieutenant Nyota Uhura",
    r#"{"Type":"Sorcery"}"#,
    r#"{"Type":"Creature"}"#
)]
#[case::any_card(
    "Ensign Pavel Chekov",
    r#"{"Type":"Land"},{"OwnedBy""#,
    r#"{"OwnedBy""#
)]
#[case::always_shuffles(
    "Captain Kirk, Boldly Going",
    r#"{"If":{"condition":{"Searched":{"zone":"Library"}},"then":[{"Shuffle":{}}]}}"#,
    r#"{"Shuffle":{}}"#
)]
#[case::deathtouch(
    "Vic Fontaine, One Night Only",
    r#""Lifelink""#,
    r#""Deathtouch""#
)]
#[case::three_lands("U.S.S. Voyager, Lone Wanderer", r#""count":2"#, r#""count":3"#)]
#[case::their_power(
    "Maular, the Next Evolution",
    r#""of":"Triggering","stat":"Power""#,
    r#""of":"This","stat":"Power""#
)]
#[case::seven_or_more("The Destruction of Praxis", r#""right":6"#, r#""right":7"#)]
#[case::six_counters("Captain Jean-Luc Picard", r#"["charge",7]"#, r#"["charge",6]"#)]
#[case::if_they_do("Tell Me What You See", r#"{"Not":"Performed"}"#, r#""Performed""#)]
fn r118_01_a_near_miss_does_not_read_back(
    #[case] name: &str,
    #[case] from: &str,
    #[case] to: &str,
) {
    let text = compact(name);
    assert!(text.contains(from), "{name} holds {from}");
    let report = report_of(name, &text.replacen(from, to, 1));
    assert_ne!(report.verdict(), Verdict::Accepted, "{name}: {to}");
}

/// Prints every wording the writer gives each ability of `MTG_PROBE_ABILITIES`, one JSON ability a line (a tool for
/// writing rules, run by hand).
#[test]
#[ignore = "a probe run by hand"]
fn r118_01_probe_written() {
    let lines = std::fs::read_to_string(std::env::var("MTG_PROBE_ABILITIES").unwrap()).unwrap();
    for line in lines
        .lines()
        .filter(|line| line.trim_start().starts_with('{'))
    {
        let ability: mtg_cards::ability::Ability = match serde_json::from_str(line.trim()) {
            Ok(ability) => ability,
            Err(error) => {
                println!("=== bad json: {error}");
                continue;
            }
        };
        println!("=== {}", &line[..line.len().min(160)]);
        for text in mtg_cards::compile::written_variants(&ability) {
            println!("  {text}");
        }
    }
}
