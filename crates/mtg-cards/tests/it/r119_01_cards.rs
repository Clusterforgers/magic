//! Goal: every card the r119-01 pack made fully automatic reads back as its printed text clause by clause, and a near miss
//! of each (another number, player, zone or filter) does not.
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
#[case("All Good Things...")]
#[case("Assimilate the Wounded")]
#[case("Reg Barclay, Fidgety Fantasist")]
#[case("Rules of Acquisition")]
#[case("U.S.S. Farragut, Bold Explorer")]
#[case("B'Elanna Torres, Scrapper")]
#[case("Brazen Bluff")]
#[case("Cmdr. Troi, Outgoing Empath")]
#[case("Commander Spock")]
#[case("Guinan, Ancient Advisor")]
#[case("Itazura, Lingering Wick")]
#[case("Jean-Luc, Mon Capitaine")]
#[case("Kai Winn, Faithless Leader")]
#[case("Kuroki, Thief of Talents")]
#[case("Lwaxana Troi, Charming Guest")]
#[case("Marooned")]
#[case("Overflow Submatrix")]
#[case("Sarek, Federation Ambassador")]
#[case("The Everforger")]
#[case("Starfleet Communicator")]
#[case("Stray from History")]
#[case("Whtz, the Bibliophile")]
#[case("Zefram Cochrane, Warp Pioneer")]
fn r119_01_reads_back_whole(#[case] name: &str) {
    let report = report_of(name, &compact(name));
    assert_eq!(
        report.verdict(),
        Verdict::Accepted,
        "{name}: {:#?}",
        report.clauses
    );
}

#[rstest]
#[case::two_hundred("Whtz, the Bibliophile", r#""at_least":200"#, r#""at_least":100"#)]
#[case::three_cards("Itazura, Lingering Wick", r#""count":3"#, r#""count":2"#)]
#[case::five_types("Sarek, Federation Ambassador", r#""by":5"#, r#""by":4"#)]
fn r119_01_a_near_miss_does_not_read_back(
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
fn r119_01_probe_written() {
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
