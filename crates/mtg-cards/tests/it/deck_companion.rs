//! Goal: the deck check judges a companion by the condition of its ability over the starting deck, the main deck and the commanders, in a constructed format and in Commander, names the card that breaks it in the companion zone, and leaves a companion whose card has no rules in the library, a second companion and a deck that fits to the other rules.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::deck::{CardFacts, DeckLine, DeckZone, IssueCode, Legality, deck_report};
use mtg_cards::load::{Library, Source};
use mtg_cards::support::{Caps, PrintedCard, PrintedFace};
use mtg_core::color::ColorSet;
use rstest::rstest;

const COMPANION: &str = "00000000-0000-4000-8000-000000000001";
const FOREST: &str = "00000000-0000-4000-8000-000000000002";
const BEAR: &str = "00000000-0000-4000-8000-000000000003";
const GIANT: &str = "00000000-0000-4000-8000-000000000004";
const LEADER: &str = "00000000-0000-4000-8000-000000000005";
const SECOND: &str = "00000000-0000-4000-8000-000000000006";

/// "Each permanent card in your starting deck has mana value 2 or less", on any card of the deck.
const RULES: &str = r#"{"Keyword":{"Companion":{"Each":{"has":[{"Compare":{"stat":"ManaValue","op":"Le","to":2}}]}}}}"#;

fn facts(id: &str, name: &str, cost: &str, type_line: &str, identity: &str) -> (String, CardFacts) {
    let legal: BTreeMap<String, Legality> = ["commander", "modern"]
        .iter()
        .map(|format| ((*format).to_owned(), Legality::Legal))
        .collect();
    (
        id.to_owned(),
        CardFacts {
            printed: PrintedCard {
                oracle_id: id.to_owned(),
                name: name.to_owned(),
                layout: String::new(),
                keywords: Vec::new(),
                faces: vec![PrintedFace {
                    name: name.to_owned(),
                    mana_cost: cost.to_owned(),
                    type_line: type_line.to_owned(),
                    ..PrintedFace::default()
                }],
            },
            color_identity: identity
                .chars()
                .filter_map(mtg_core::color::Color::from_symbol)
                .collect::<ColorSet>(),
            legalities: legal,
        },
    )
}

fn cards() -> BTreeMap<String, CardFacts> {
    [
        facts(
            COMPANION,
            "Small Companion",
            "{1}{G}",
            "Creature \u{2014} Cat",
            "G",
        ),
        facts(FOREST, "Forest", "", "Basic Land \u{2014} Forest", "G"),
        facts(BEAR, "Bear", "{1}{G}", "Creature \u{2014} Bear", "G"),
        facts(GIANT, "Giant", "{2}{G}", "Creature \u{2014} Giant", "G"),
        facts(
            LEADER,
            "Leader",
            "{2}{G}",
            "Legendary Creature \u{2014} Elf",
            "G",
        ),
        facts(
            SECOND,
            "Second Companion",
            "{1}{G}",
            "Creature \u{2014} Cat",
            "G",
        ),
    ]
    .into_iter()
    .collect()
}

/// A library holding the rules of the companions, or none.
fn library(with_rules: bool) -> Library {
    if !with_rules {
        return Library::from_sources(&[]).unwrap();
    }
    let source = |id: &str, name: &str| Source {
        path: format!("cards/{}.json", name.to_lowercase().replace(' ', "_")),
        text: format!(
            r#"{{"oracle_id":"{id}","name":"{name}","impl_status":"Full","faces":[{{"name":"{name}","oracle_fingerprint":"e3b0c44298fc1c149afbf4c8996fb924","abilities":[{RULES}]}}]}}"#
        ),
    };
    Library::from_sources(&[
        source(COMPANION, "Small Companion"),
        source(SECOND, "Second Companion"),
    ])
    .unwrap_or_else(|errors| panic!("{errors}"))
}

fn line(zone: DeckZone, id: &str, count: u32) -> DeckLine {
    DeckLine {
        zone,
        oracle_id: id.to_owned(),
        count,
    }
}

fn codes(format: &str, lines: &[DeckLine], with_rules: bool) -> Vec<IssueCode> {
    deck_report(format, lines, &cards(), &library(with_rules), &Caps::none())
        .legality
        .iter()
        .map(|issue| issue.code)
        .collect()
}

#[rstest]
#[case::cheap_permanents(vec![line(DeckZone::Main, FOREST, 56), line(DeckZone::Main, BEAR, 4)], true, false)]
#[case::one_big_permanent(vec![line(DeckZone::Main, FOREST, 59), line(DeckZone::Main, GIANT, 1)], true, true)]
#[case::big_permanents_in_the_sideboard_do_not_count(vec![line(DeckZone::Main, FOREST, 60), line(DeckZone::Sideboard, GIANT, 3)], true, false)]
#[case::no_rules_in_the_library_leave_it_unjudged(vec![line(DeckZone::Main, FOREST, 59), line(DeckZone::Main, GIANT, 1)], false, false)]
fn a_constructed_deck_is_judged_by_the_companion_condition(
    #[case] mut lines: Vec<DeckLine>,
    #[case] with_rules: bool,
    #[case] refused: bool,
) {
    lines.push(line(DeckZone::Companion, COMPANION, 1));
    let found = codes("modern", &lines, with_rules);
    assert_eq!(
        found.contains(&IssueCode::CompanionCondition),
        refused,
        "{found:?}"
    );
}

#[rstest]
#[case::a_cheap_commander(BEAR, false)]
#[case::the_commander_counts_toward_the_condition(LEADER, true)]
fn in_commander_the_commander_is_part_of_the_starting_deck(
    #[case] commander: &str,
    #[case] refused: bool,
) {
    let lines = vec![
        line(DeckZone::Commander, commander, 1),
        line(DeckZone::Main, FOREST, 99),
        line(DeckZone::Companion, COMPANION, 1),
    ];
    let found = codes("commander", &lines, true);
    assert_eq!(
        found.contains(&IssueCode::CompanionCondition),
        refused,
        "{found:?}"
    );
}

#[test]
fn the_issue_names_the_companion_and_the_card_that_breaks_the_condition() {
    let lines = vec![
        line(DeckZone::Main, FOREST, 59),
        line(DeckZone::Main, GIANT, 1),
        line(DeckZone::Companion, COMPANION, 1),
    ];
    let report = deck_report("modern", &lines, &cards(), &library(true), &Caps::none());
    let issue = report
        .legality
        .iter()
        .find(|issue| issue.code == IssueCode::CompanionCondition)
        .expect("the condition is broken");
    assert_eq!(issue.zone, Some(DeckZone::Companion));
    assert_eq!(issue.name.as_deref(), Some("Small Companion"));
    assert!(issue.message.contains("Giant"), "{}", issue.message);
}

#[test]
fn two_companions_are_a_count_issue_and_no_condition_is_judged() {
    let lines = vec![
        line(DeckZone::Main, FOREST, 59),
        line(DeckZone::Main, GIANT, 1),
        line(DeckZone::Companion, COMPANION, 1),
        line(DeckZone::Companion, SECOND, 1),
    ];
    let found = codes("modern", &lines, true);
    assert!(found.contains(&IssueCode::CompanionCount));
    assert!(!found.contains(&IssueCode::CompanionCondition));
}
