//! Goal: a recorded MTGJSON precon deck file normalizes into a deck with commanders, boards and cross-references intact, ignoring everything we do not model.

use mtg_import::decklist::Section;
use mtg_import::ids::{OracleId, PrintingId};
use mtg_import::mtgjson::{PreconDeck, parse_deck};

const DECK_JSON: &str = include_str!("fixtures/mtgjson_deck_commander.json");

fn deck() -> PreconDeck {
    parse_deck(DECK_JSON.as_bytes()).unwrap_or_else(|e| panic!("fixture must parse: {e}"))
}

#[test]
fn deck_header_is_normalized() {
    let deck = deck();
    assert_eq!(deck.name, "Breed Lethality");
    assert_eq!(deck.code, "C16");
    assert_eq!(deck.deck_type.as_deref(), Some("Commander Deck"));
    assert_eq!(deck.release_date.as_deref(), Some("2016-11-11"));
}

#[test]
fn boards_hold_their_cards_in_file_order() {
    let deck = deck();
    let names = |cards: &[mtg_import::mtgjson::PreconCard]| -> Vec<String> {
        cards
            .iter()
            .map(|c| format!("{}x {}", c.count, c.name))
            .collect()
    };
    assert_eq!(names(&deck.commander), ["1x Atraxa, Praetors' Voice"]);
    assert_eq!(
        names(&deck.display_commander),
        ["1x Atraxa, Praetors' Voice"]
    );
    assert_eq!(
        names(&deck.main_board),
        [
            "1x Sol Ring",
            "1x Arcane Signet",
            "36x Forest",
            "1x Fire // Ice"
        ]
    );
    assert_eq!(deck.side_board, Vec::new());
}

#[test]
fn card_printing_and_identifiers_are_kept() {
    let deck = deck();
    let sol_ring = &deck.main_board[0];
    assert!(sol_ring.is_foil);
    assert_eq!(sol_ring.set_code.as_deref(), Some("C16"));
    assert_eq!(sol_ring.number.as_deref(), Some("266"));
    assert_eq!(sol_ring.uuid, "1c6cab14-1a3e-5b49-9b4b-4a2d1b5b2d02");
    assert_eq!(
        sol_ring.oracle_id.as_ref().map(OracleId::as_str),
        Some("bd10c9cd-d103-45d9-8d3a-07f5ceb67495")
    );
    assert_eq!(
        sol_ring.scryfall_id.as_ref().map(PrintingId::as_str),
        Some("7a0bc8ee-d092-431e-88fa-f60f951a23e5")
    );
    assert!(!deck.main_board[1].is_foil);

    let fire_ice = &deck.main_board[3];
    assert_eq!(fire_ice.oracle_id, None);
    assert_eq!(fire_ice.name, "Fire // Ice");
}

#[test]
fn conversion_to_a_deck_list_puts_commanders_first() {
    let list = deck().to_parsed_deck();
    assert_eq!(list.errors, Vec::new());
    assert_eq!(list.entries.len(), 5);
    assert_eq!(list.entries[0].section, Section::Commander);
    assert!(list.entries[1..].iter().all(|e| e.section == Section::Main));
    assert_eq!(list.card_count(Section::Commander), 1);
    assert_eq!(list.card_count(Section::Main), 39);
    assert_eq!(list.card_count(Section::Sideboard), 0);
    assert_eq!(list.entries[0].set.as_deref(), Some("C16"));
    assert_eq!(list.entries[0].collector_number.as_deref(), Some("28"));
}

#[test]
fn truncated_file_is_an_error_not_a_partial_deck() {
    for cut in (1..DECK_JSON.len()).step_by(17) {
        assert!(
            parse_deck(&DECK_JSON.as_bytes()[..cut]).is_err(),
            "cut at {cut}"
        );
    }
}
