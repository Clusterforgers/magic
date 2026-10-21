//! Goal: every export of the corpus resolves to the cards its site lists for the deck: by printing where the text has a set and a number, by the card's name or by another name of a printing (flavor name, name of a digital version) where it has none.
//!
//! The pool of the plain test is what a Scryfall dump of all printings would hold for these decks (every printing the sites list, plus the Scryfall records of the printings that carry another name); the ignored one resolves against the real Oracle Cards file instead.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::{BTreeMap, BTreeSet};

use mtg_import::decklist::parse_as;
use mtg_import::ids::OracleId;
use mtg_import::resolve::{CardIndex, MatchKind, resolve_deck};
use mtg_import::scryfall::{CardRecord, cards};

use crate::corpus::{Deck, Row, canonical, shape};
use crate::real_data;

/// Scryfall records of the printings the corpus and the real exports name differently from their card.
pub const OTHER_NAME_PRINTINGS: &str = include_str!("../fixtures/scryfall_alias_printings.json");

/// The records of [`OTHER_NAME_PRINTINGS`].
pub fn other_name_records() -> Vec<CardRecord> {
    cards(OTHER_NAME_PRINTINGS.as_bytes())
        .map(|record| record.expect("fixture record"))
        .collect()
}

fn rows(deck: &Deck) -> impl Iterator<Item = &Row> {
    let boards = &deck.truth.boards;
    [
        &boards.commanders,
        &boards.signature_spells,
        &boards.mainboard,
        &boards.sideboard,
        &boards.maybeboard,
        &boards.stickers,
    ]
    .into_iter()
    .flatten()
}

/// Card names the sites list for `deck`.
fn names(deck: &Deck) -> BTreeSet<String> {
    rows(deck).map(|row| canonical(&row.name)).collect()
}

/// An index of what the sites list (a card is its own oracle id) and the records that carry other names.
fn listed_printings(decks: &[Deck]) -> CardIndex {
    let mut index = CardIndex::new();
    for row in decks.iter().flat_map(rows) {
        let name = canonical(&row.name);
        let id = OracleId(name.clone());
        index.insert(id.clone(), &name);
        index.add_printing(&id, &row.set, &row.cn);
    }
    for mut record in other_name_records() {
        record.oracle_id = OracleId(record.name.clone());
        index.insert_record(&record);
    }
    index
}

/// How the entries of the exports were matched.
#[derive(Debug, Default)]
struct Tally {
    exports: usize,
    printing: usize,
    name: usize,
    other_name: usize,
}

/// Resolves every export of `decks` against `index`; returns one line per entry that is unresolved or resolved to a card the site does not list.
fn problems(decks: &[Deck], index: &CardIndex) -> (Tally, Vec<String>) {
    let mut tally = Tally::default();
    let mut found = Vec::new();
    for deck in decks {
        let listed = names(deck);
        let by_printing: BTreeMap<_, _> = rows(deck)
            .map(|row| {
                (
                    (row.set.clone(), row.cn.to_lowercase()),
                    canonical(&row.name),
                )
            })
            .collect();
        for variant in deck.truth.variants.keys() {
            tally.exports += 1;
            let label = format!("{}/{variant}", deck.label());
            let parsed = parse_as(shape(deck.site, variant).format, &deck.text(variant));
            let resolution = resolve_deck(index, &parsed);
            for item in resolution.unresolved {
                let entry = item.entry;
                found.push(format!(
                    "{label}: {:?} ({:?} {:?}): {}",
                    entry.name, entry.set, entry.collector_number, item.error
                ));
            }
            for item in resolution.resolved {
                match item.matched {
                    MatchKind::Printing => tally.printing += 1,
                    MatchKind::Name => tally.name += 1,
                    MatchKind::OtherName => tally.other_name += 1,
                }
                let printing = item
                    .set
                    .as_deref()
                    .zip(item.collector_number.as_deref())
                    .map(|(set, number)| (set.to_lowercase(), number.to_lowercase()));
                let expected = printing.as_ref().and_then(|key| by_printing.get(key));
                let right = match expected {
                    Some(name) => *name == item.name,
                    None => listed.contains(&item.name),
                };
                if !right {
                    found.push(format!(
                        "{label}: {:?} resolved to {:?}, which the deck does not list",
                        item.name, expected
                    ));
                }
            }
        }
    }
    (tally, found)
}

#[test]
fn every_export_resolves_to_the_cards_of_its_deck() {
    let decks = Deck::all();
    let (tally, found) = problems(&decks, &listed_printings(&decks));
    assert!(found.is_empty(), "\n{}", found.join("\n"));
    assert_eq!(tally.exports, 183);
    // The lines with a set and a number, the lines with a card name only, and the lines named by a flavor or digital name.
    assert_eq!(
        (tally.printing, tally.name, tally.other_name),
        (11_924, 6_043, 24),
        "{tally:?}"
    );
}

/// Needs the Oracle Cards dump described in `real_data.rs`.
#[test]
#[ignore = "needs MTG_REAL_DATA_DIR"]
fn every_export_resolves_against_the_real_scryfall_cards() {
    let mut records = real_data::load_cards();
    records.extend(other_name_records());
    let decks = Deck::all();
    let (tally, found) = problems(&decks, &CardIndex::from_records(&records));
    assert!(found.is_empty(), "\n{}", found.join("\n"));
    assert_eq!(tally.exports, 183);
    // Oracle Cards holds one printing per card, so only some lines match by printing; the other names come from the fixture.
    assert_eq!(tally.other_name, 24, "{tally:?}");
    assert!(tally.printing + tally.name > 17_000, "{tally:?}");
}
