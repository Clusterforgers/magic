//! The two hashes the Verified marker is derived from: one over what the deck is made of, one over
//! how far the game plays those cards. Each hashes a versioned, canonical text, so a change of
//! encoding is a change of version and equal decks always give equal hashes.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use super::{DeckLine, DeckZone, merged};
use crate::support::hex;

/// Heads the text `deck_hash` hashes.
const DECK_VERSION: &str = "mtg-deck-check/deck/v1";
/// Heads the text `coverage_hash` hashes.
const COVERAGE_VERSION: &str = "mtg-deck-check/coverage/v1";

fn sha256_hex(text: &str) -> String {
    hex(&Sha256::digest(text.as_bytes()))
}

/// The hash of a deck's zones: SHA-256, as 64 lower-case hex digits, of the text `mtg-deck-check/deck/v1`
/// and then one line `<zone> <oracle id> <count>` per card and zone, ordered by zone name and oracle
/// id, with copies of one card in one zone summed and ids in lower case. Printings, the deck's name,
/// notes and format are not in it, so swapping art keeps it and any change of the cards changes it.
pub fn deck_hash(lines: &[DeckLine]) -> String {
    deck_hash_of(&merged(lines))
}

pub(super) fn deck_hash_of(deck: &BTreeMap<(DeckZone, String), u32>) -> String {
    let mut ordered: Vec<(&str, &str, u32)> = deck
        .iter()
        .map(|((zone, id), count)| (zone.keyword(), id.as_str(), *count))
        .collect();
    ordered.sort_unstable();
    let mut text = format!("{DECK_VERSION}\n");
    for (zone, id, count) in ordered {
        let _ = writeln!(text, "{zone} {id} {count}");
    }
    sha256_hex(&text)
}

/// The hash of how far the game plays a deck: SHA-256, as 64 lower-case hex digits, of the text
/// `mtg-deck-check/coverage/v1` and then one line `<oracle id> <support fingerprint>` per distinct
/// card, ordered by oracle id. Counts and zones are not in it. A change in a card's rules, its imported
/// text or what the engine runs changes the card's fingerprint and so the hash.
pub fn coverage_hash(cards: &[(String, String)]) -> String {
    let distinct: BTreeSet<(String, &str)> = cards
        .iter()
        .map(|(id, fingerprint)| (id.to_ascii_lowercase(), fingerprint.as_str()))
        .collect();
    let mut text = format!("{COVERAGE_VERSION}\n");
    for (id, fingerprint) in distinct {
        let _ = writeln!(text, "{id} {fingerprint}");
    }
    sha256_hex(&text)
}

#[cfg(test)]
mod tests {
    //! Goal: both hashes are pinned to values computed outside this crate, do not depend on the order or the splitting of the input, and change with every input they are made of and with nothing else.
    use rstest::rstest;

    use super::*;

    const A: &str = "00000000-0000-4000-8000-00000000000a";
    const B: &str = "00000000-0000-4000-8000-00000000000b";

    fn line(zone: DeckZone, id: &str, count: u32) -> DeckLine {
        DeckLine {
            zone,
            oracle_id: id.to_owned(),
            count,
        }
    }

    fn base() -> Vec<DeckLine> {
        vec![
            line(DeckZone::Main, A, 4),
            line(DeckZone::Sideboard, B, 2),
            line(DeckZone::Commander, A, 1),
        ]
    }

    #[test]
    fn the_deck_hash_is_pinned() {
        // printf 'mtg-deck-check/deck/v1\ncommander 00000000-0000-4000-8000-00000000000a 1\nmain 00000000-0000-4000-8000-00000000000a 4\nsideboard 00000000-0000-4000-8000-00000000000b 2\n' | sha256sum
        assert_eq!(
            deck_hash(&base()),
            "0ec94d92a9b2786bfde4c9ad07b35429412e26c1bd7adefafd2be73684a8c96d"
        );
        // printf 'mtg-deck-check/deck/v1\n' | sha256sum
        assert_eq!(
            deck_hash(&[]),
            "3cf60796a040d6db9357121b8fb6956b52e7b8dbad33b4c1cb1e07935cecc157"
        );
    }

    #[test]
    fn the_coverage_hash_is_pinned() {
        let cards = vec![
            (A.to_owned(), "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned()),
            (B.to_owned(), "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned()),
        ];
        // printf 'mtg-deck-check/coverage/v1\n00000000-0000-4000-8000-00000000000a aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n00000000-0000-4000-8000-00000000000b bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n' | sha256sum
        assert_eq!(
            coverage_hash(&cards),
            "32b7b84b57f6b483e5c70af382b555366292e423d789875c6102351cd1dd8d24"
        );
    }

    #[test]
    fn order_splitting_case_and_empty_lines_do_not_matter() {
        let expected = deck_hash(&base());
        let mut shuffled = base();
        shuffled.reverse();
        assert_eq!(deck_hash(&shuffled), expected);

        let split = vec![
            line(DeckZone::Main, A, 1),
            line(DeckZone::Sideboard, B, 2),
            line(DeckZone::Main, &A.to_uppercase(), 3),
            line(DeckZone::Commander, A, 1),
            line(DeckZone::Companion, B, 0),
        ];
        assert_eq!(deck_hash(&split), expected);
    }

    #[rstest]
    #[case::another_zone(vec![line(DeckZone::Sideboard, A, 4), line(DeckZone::Sideboard, B, 2), line(DeckZone::Commander, A, 1)])]
    #[case::another_card(vec![line(DeckZone::Main, B, 4), line(DeckZone::Sideboard, B, 2), line(DeckZone::Commander, A, 1)])]
    #[case::another_count(vec![line(DeckZone::Main, A, 5), line(DeckZone::Sideboard, B, 2), line(DeckZone::Commander, A, 1)])]
    #[case::a_card_fewer(vec![line(DeckZone::Main, A, 4), line(DeckZone::Sideboard, B, 2)])]
    #[case::a_card_more(vec![line(DeckZone::Main, A, 4), line(DeckZone::Sideboard, B, 2), line(DeckZone::Commander, A, 1), line(DeckZone::Companion, B, 1)])]
    fn every_input_of_the_deck_hash_changes_it(#[case] other: Vec<DeckLine>) {
        assert_ne!(deck_hash(&other), deck_hash(&base()));
    }

    #[test]
    fn the_coverage_hash_follows_the_fingerprints_and_the_card_set_only() {
        let fingerprint = |c: char| c.to_string().repeat(32);
        let cards = |pairs: &[(&str, char)]| -> Vec<(String, String)> {
            pairs
                .iter()
                .map(|(id, c)| ((*id).to_owned(), fingerprint(*c)))
                .collect()
        };
        let expected = coverage_hash(&cards(&[(A, 'a'), (B, 'b')]));
        assert_eq!(coverage_hash(&cards(&[(B, 'b'), (A, 'a')])), expected);
        assert_eq!(
            coverage_hash(&cards(&[(A, 'a'), (B, 'b'), (A, 'a')])),
            expected,
            "a card in two zones is one card"
        );
        assert_ne!(coverage_hash(&cards(&[(A, 'a'), (B, 'c')])), expected);
        assert_ne!(coverage_hash(&cards(&[(A, 'a')])), expected);
        assert_ne!(
            coverage_hash(&cards(&[(A, 'a'), (B, 'b'), ("c", 'c')])),
            expected
        );
        assert_ne!(coverage_hash(&[]), expected);
    }
}
