//! Goal: every name a card or printing is known by (card, front face, flavor, digital and meld name) ties a line to the card, a flavor or digital name also selects the one printing that carries it, and no such name takes a card name from its card.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_import::decklist::{DeckFormat, parse_as};
use mtg_import::resolve::{CardIndex, MatchKind, Resolution, resolve_deck};
use mtg_import::scryfall::{CardRecord, cards};
use rstest::rstest;

const CARDS: &str = include_str!("../fixtures/scryfall_oracle_cards.json");
const OTHER_NAMES: &str = include_str!("../fixtures/scryfall_alias_printings.json");
const MELD_PAIR: &str = include_str!("../fixtures/scryfall_meld_pair.json");

/// The fixture cards, the printings with another name and the meld pair, as one all-printings file would hold them.
fn records() -> Vec<CardRecord> {
    [CARDS, OTHER_NAMES, MELD_PAIR]
        .into_iter()
        .flat_map(|json| cards(json.as_bytes()).filter_map(Result::ok))
        .collect()
}

fn resolve(records: &[CardRecord], line: &str) -> Resolution {
    let deck = parse_as(DeckFormat::PlainText, line);
    resolve_deck(&CardIndex::from_records(records), &deck)
}

/// The card, how the line was matched, the set and the number of the one entry a line resolved to.
type Outcome = (String, MatchKind, Option<String>, Option<String>);

fn outcome(resolution: &Resolution) -> Outcome {
    let [entry] = resolution.resolved.as_slice() else {
        panic!("not resolved: {resolution:?}");
    };
    (
        entry.name.clone(),
        entry.matched,
        entry.set.clone(),
        entry.collector_number.clone(),
    )
}

fn expected(card: &str, matched: MatchKind, printing: Option<(&str, &str)>) -> Outcome {
    let (set, number) = printing.unzip();
    (
        card.to_owned(),
        matched,
        set.map(str::to_owned),
        number.map(str::to_owned),
    )
}

#[rstest]
#[case::card_name("1 Fellwar Stone", "Fellwar Stone", MatchKind::Name, None)]
#[case::flavor_name("1 The Ball", "Fellwar Stone", MatchKind::OtherName, Some(("sld", "1040")))]
#[case::flavor_name_in_capitals("1 THE BALL", "Fellwar Stone", MatchKind::OtherName, Some(("sld", "1040")))]
#[case::digital_name("1 Skittering Kitten", "Masked Meower", MatchKind::OtherName, Some(("om1", "96")))]
#[case::digital_name_again("1 Phenomena Recorder", "Peter Parker's Camera", MatchKind::OtherName, Some(("om1", "171")))]
#[case::flavor_name_of_three_printings(
    "1 Helm's Deep",
    "Shinka, the Bloodsoaked Keep",
    MatchKind::OtherName,
    None
)]
#[case::flavor_name_beside_its_printing("1 The Ball (SLD) 1040", "Fellwar Stone", MatchKind::Printing, Some(("SLD", "1040")))]
#[case::flavor_name_beside_a_printing_the_index_lacks("1 The Ball (C21) 259", "Fellwar Stone", MatchKind::OtherName, Some(("C21", "259")))]
#[case::front_face(
    "1 Delver of Secrets",
    "Delver of Secrets // Insectile Aberration",
    MatchKind::Name,
    None
)]
#[case::split_card("1 Fire // Ice", "Fire // Ice", MatchKind::Name, None)]
#[case::split_card_front("1 Fire", "Fire // Ice", MatchKind::Name, None)]
#[case::meld_piece(
    "1 Gisela, the Broken Blade",
    "Gisela, the Broken Blade",
    MatchKind::Name,
    None
)]
#[case::meld_result(
    "1 Brisela, Voice of Nightmares",
    "Brisela, Voice of Nightmares",
    MatchKind::Name,
    None
)]
#[case::meld_name(
    "1 Gisela, the Broken Blade // Brisela, Voice of Nightmares",
    "Gisela, the Broken Blade",
    MatchKind::OtherName,
    None
)]
#[case::meld_name_of_the_other_piece(
    "1 Bruna, the Fading Light // Brisela, Voice of Nightmares",
    "Bruna, the Fading Light",
    MatchKind::OtherName,
    None
)]
fn a_line_ties_to_its_card_and_a_name_of_a_printing_selects_it(
    #[case] line: &str,
    #[case] card: &str,
    #[case] matched: MatchKind,
    #[case] printing: Option<(&str, &str)>,
) {
    let found = outcome(&resolve(&records(), line));
    assert_eq!(found, expected(card, matched, printing), "{line}");
}

#[test]
fn a_set_alone_stays_as_written_beside_a_name_of_a_printing() {
    let found = outcome(&resolve(&records(), "1 The Ball (SLD)"));
    assert_eq!(
        found,
        (
            "Fellwar Stone".to_owned(),
            MatchKind::OtherName,
            Some("SLD".to_owned()),
            None
        )
    );
}

#[rstest]
#[case::half_a_name_of_the_card("1 Brisela")]
#[case::the_halves_in_the_wrong_order("1 Brisela, Voice of Nightmares // Gisela, the Broken Blade")]
#[case::a_back_face_alone("1 Insectile Aberration")]
#[case::the_name_of_a_foreign_printing("1 Peter Parkers Kamera")]
fn names_that_no_card_or_printing_carries_stay_unresolved(#[case] line: &str) {
    let resolution = resolve(&records(), line);
    assert!(resolution.resolved.is_empty(), "{resolution:?}");
    assert_eq!(resolution.unresolved.len(), 1);
}

#[test]
fn a_name_of_a_printing_never_takes_a_card_name_from_its_card() {
    let mut records = records();
    // A card whose own name is the flavor name another card's printing carries.
    let mut card = records
        .iter()
        .find(|record| record.name == "Sol Ring")
        .cloned()
        .unwrap();
    card.name = "The Ball".to_owned();
    card.front.name = "The Ball".to_owned();
    card.oracle_id.0 = "the-ball-oracle".to_owned();
    card.printing_id.0 = "the-ball-printing".to_owned();
    card.set = "xxx".to_owned();
    records.push(card);

    let found = outcome(&resolve(&records, "1 The Ball"));

    assert_eq!(found, expected("The Ball", MatchKind::Name, None));
}
