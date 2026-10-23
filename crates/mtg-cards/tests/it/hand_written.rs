//! Goal: every hand-written rules file is valid as a file of its own (structure, then the printed card) against its fixture record and, when an Oracle Cards download is at hand, against the current record of the same card, so an erratum or a changed keyword list fails the card it touched and a check that finds nothing is shown to find something.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};

use mtg_cards::card::Card;
use mtg_cards::load::Library;
use mtg_cards::support::PrintedCard;
use rstest::rstest;

use crate::common::{self, Corpus, UPDATE};

/// The lowercase names of the cards and their faces: what a rule may name.
fn names_of(cards: &[PrintedCard]) -> BTreeSet<String> {
    cards
        .iter()
        .flat_map(|card| {
            std::iter::once(&card.name).chain(card.faces.iter().map(|face| &face.name))
        })
        .map(|name| name.to_lowercase())
        .collect()
}

/// The problems of the file of `card` against `shown`, each with the card's name; none when it is valid.
fn problems_of_card(
    library: &Library,
    card: &Card,
    shown: Option<&PrintedCard>,
    names: &BTreeSet<String>,
) -> Vec<String> {
    match shown {
        None => vec![format!(
            "{}: no record has oracle id {}",
            card.name, card.oracle_id
        )],
        Some(shown) => library
            .check_generated(card, shown, names)
            .iter()
            .map(|problem| format!("{}: {problem}", card.name))
            .collect(),
    }
}

/// The problems of every file of the library against the records.
fn problems(library: &Library, records: &[PrintedCard]) -> Vec<String> {
    let names = names_of(records);
    let by_id: BTreeMap<&str, &PrintedCard> = records
        .iter()
        .map(|record| (record.oracle_id.as_str(), record))
        .collect();
    library
        .cards()
        .flat_map(|card| {
            let shown = by_id.get(card.oracle_id.as_str()).copied();
            problems_of_card(library, card, shown, &names)
        })
        .collect()
}

/// Where the records a file is checked against come from.
#[derive(Debug, Clone, Copy)]
enum Records {
    /// The fixture, the corpus cards as they were imported.
    Fixture,
    /// The newest Oracle Cards download of `MTG_REAL_DATA_DIR` or the `mtg-cli` cache.
    Download,
}

impl Records {
    /// What the records are called in a failure, and the records; `None` when there is no download.
    fn load(self) -> Option<(String, Vec<PrintedCard>)> {
        match self {
            Records::Fixture => Some(("the fixture".into(), Corpus::repo().printed())),
            Records::Download => common::find_oracle_cards().map(|file| {
                (
                    file.display().to_string(),
                    common::printed_of_download(&file),
                )
            }),
        }
    }
}

#[rstest]
#[case::its_fixture_record(Records::Fixture)]
#[case::the_current_oracle_card(Records::Download)]
fn every_hand_written_file_is_valid_against(#[case] records: Records) {
    let Some((from, records)) = records.load() else {
        eprintln!(
            "skipped: no oracle-cards-*.jsonl.gz in MTG_REAL_DATA_DIR or the mtg-cli cache; \
             `mtg-cli fetch scryfall` downloads it"
        );
        return;
    };
    let found = problems(&Corpus::repo().library(), &records);
    assert!(
        found.is_empty(),
        "{} problems against {from}; fix the rules for an erratum, or refresh the fingerprint \
         (`{UPDATE}`) when the rules still fit:\n{}",
        found.len(),
        found.join("\n")
    );
}

/// The card of the corpus the cases below change.
const BOLT: &str = "Lightning Bolt";

fn bolt(records: &mut [PrintedCard]) -> &mut PrintedCard {
    records
        .iter_mut()
        .find(|record| record.name == BOLT)
        .expect("Lightning Bolt is in the corpus")
}

#[rstest]
#[case::an_erratum_in_the_text(
    |records: &mut [PrintedCard]| bolt(records).faces[0].oracle_text.push_str(" Draw a card."),
    "oracle_fingerprint"
)]
#[case::a_new_keyword(
    |records: &mut [PrintedCard]| bolt(records).keywords.push("Flying".into()),
    "keyword `Flying`"
)]
#[case::a_new_type(
    |records: &mut [PrintedCard]| bolt(records).faces[0].type_line = "Creature — Elemental".into(),
    "Spell"
)]
#[case::a_new_name(
    |records: &mut [PrintedCard]| bolt(records).name = "Lightning Strike".into(),
    "imported name"
)]
#[case::a_second_face(
    |records: &mut [PrintedCard]| {
        let face = bolt(records).faces[0].clone();
        bolt(records).faces.push(face);
    },
    "face(s)"
)]
fn a_change_in_the_record_fails_the_card_it_touches(
    #[case] change: fn(&mut [PrintedCard]),
    #[case] named: &str,
) {
    let (library, mut records) = (Corpus::repo().library(), Corpus::repo().printed());
    change(&mut records);
    let card = library
        .card_by_name(BOLT)
        .expect("Lightning Bolt has a file");
    let shown = records
        .iter()
        .find(|record| record.oracle_id == card.oracle_id);
    let found = problems_of_card(&library, card, shown, &names_of(&records));
    assert!(!found.is_empty(), "the change is not found");
    assert!(
        found.iter().any(|line| line.contains(named)),
        "no problem names `{named}`: {found:?}"
    );
}

#[test]
fn a_card_without_a_record_is_a_problem() {
    let (library, mut records) = (Corpus::repo().library(), Corpus::repo().printed());
    records.retain(|record| record.name != BOLT);
    let found = problems(&library, &records);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].starts_with(BOLT) && found[0].contains("no record"),
        "{found:?}"
    );
}
