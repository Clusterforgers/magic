//! Resolves the entries of a parsed deck to cards, reporting what could not be matched.

use thiserror::Error;

use super::index::{CardIndex, Lookup, Suggestion};
use crate::decklist::{DeckEntry, ParsedDeck, Section};
use crate::ids::OracleId;

/// How many fuzzy suggestions accompany an unknown name.
pub const SUGGESTION_LIMIT: usize = 5;

/// Why a deck entry could not be tied to exactly one card.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ResolveError {
    /// No card has this name; `suggestions` lists close names but none was chosen.
    #[error("unknown card {name:?}")]
    UnknownCard {
        /// The name as written.
        name: String,
        /// Close matches, best first.
        suggestions: Vec<Suggestion>,
    },
    /// Several different cards share this name.
    #[error("{name:?} matches {} different cards", .candidates.len())]
    AmbiguousName {
        /// The name as written.
        name: String,
        /// The cards sharing the name.
        candidates: Vec<Suggestion>,
    },
}

/// A deck entry tied to exactly one card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEntry {
    /// Number of copies.
    pub count: u32,
    /// The card.
    pub oracle_id: OracleId,
    /// Canonical card name.
    pub name: String,
    /// Section of the deck.
    pub section: Section,
    /// Requested set code, passed through unchecked.
    pub set: Option<String>,
    /// Requested collector number, passed through unchecked.
    pub collector_number: Option<String>,
}

/// An entry that stayed unresolved, with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedEntry {
    /// Position of the entry in the parsed deck.
    pub index: usize,
    /// The entry as parsed.
    pub entry: DeckEntry,
    /// Why it was not resolved.
    pub error: ResolveError,
}

/// Result of resolving a whole deck; nothing is dropped silently.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resolution {
    /// Entries tied to a card, in deck order.
    pub resolved: Vec<ResolvedEntry>,
    /// Entries that need the user's attention, in deck order.
    pub unresolved: Vec<UnresolvedEntry>,
}

fn candidate(card: &super::index::IndexedCard) -> Suggestion {
    Suggestion {
        oracle_id: card.oracle_id.clone(),
        name: card.name.clone(),
        distance: 0,
    }
}

/// Looks up an entry's name, retrying with a trailing `(SET)` as part of the name.
fn lookup<'a>(index: &'a CardIndex, entry: &DeckEntry) -> (Lookup<'a>, bool) {
    let direct = index.find(&entry.name);
    // `Hazmat Suit (Used)` ends like a set code, so the text parsers split it off the name.
    if let (Lookup::Missing, Some(set), None) = (&direct, &entry.set, &entry.collector_number) {
        let joined = index.find(&format!("{} ({set})", entry.name));
        if joined != Lookup::Missing {
            return (joined, true);
        }
    }
    (direct, false)
}

/// Resolves every entry of `deck` against `index`; fuzzy matches are suggested, never applied.
pub fn resolve_deck(index: &CardIndex, deck: &ParsedDeck) -> Resolution {
    let mut resolution = Resolution::default();
    for (position, entry) in deck.entries.iter().enumerate() {
        let (found, set_was_part_of_name) = lookup(index, entry);
        match found {
            Lookup::Found(card) => resolution.resolved.push(ResolvedEntry {
                count: entry.count,
                oracle_id: card.oracle_id.clone(),
                name: card.name.clone(),
                section: entry.section,
                set: entry.set.clone().filter(|_| !set_was_part_of_name),
                collector_number: entry.collector_number.clone(),
            }),
            Lookup::Ambiguous(cards) => resolution.unresolved.push(UnresolvedEntry {
                index: position,
                entry: entry.clone(),
                error: ResolveError::AmbiguousName {
                    name: entry.name.clone(),
                    candidates: cards.into_iter().map(candidate).collect(),
                },
            }),
            Lookup::Missing => resolution.unresolved.push(UnresolvedEntry {
                index: position,
                entry: entry.clone(),
                error: ResolveError::UnknownCard {
                    name: entry.name.clone(),
                    suggestions: index.suggest(&entry.name, SUGGESTION_LIMIT),
                },
            }),
        }
    }
    resolution
}

#[cfg(test)]
mod tests {
    //! Goal: deck resolution accounts for every entry exactly once, resolves only unambiguous exact names, and explains each failure.
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    fn index() -> CardIndex {
        let mut index = CardIndex::new();
        for (n, name) in [
            "Lightning Bolt",
            "Fire // Ice",
            "Æther Vial",
            "Hazmat Suit (Used)",
            "Étude",
            "Etude",
            "Sol Ring",
        ]
        .into_iter()
        .enumerate()
        {
            index.insert(OracleId(format!("oracle-{n}")), name);
        }
        index
    }

    fn deck(entries: Vec<DeckEntry>) -> ParsedDeck {
        ParsedDeck {
            entries,
            errors: Vec::new(),
            ..ParsedDeck::default()
        }
    }

    fn entry(count: u32, name: &str, section: Section) -> DeckEntry {
        DeckEntry::new(count, name, section)
    }

    #[test]
    fn resolves_in_order_and_carries_entry_data() {
        let parsed = deck(vec![
            entry(4, "lightning bolt", Section::Main).with_printing("m10", Some("146")),
            entry(1, "Aether Vial", Section::Sideboard),
            entry(2, "Fire/Ice", Section::Commander),
        ]);
        let result = resolve_deck(&index(), &parsed);
        assert_eq!(result.unresolved, Vec::new());
        let got: Vec<_> = result
            .resolved
            .iter()
            .map(|r| (r.count, r.name.as_str(), r.section))
            .collect();
        assert_eq!(
            got,
            [
                (4, "Lightning Bolt", Section::Main),
                (1, "Æther Vial", Section::Sideboard),
                (2, "Fire // Ice", Section::Commander)
            ]
        );
        assert_eq!(result.resolved[0].set.as_deref(), Some("M10"));
        assert_eq!(result.resolved[0].collector_number.as_deref(), Some("146"));
        assert_eq!(result.resolved[0].oracle_id, OracleId("oracle-0".into()));
    }

    #[test]
    fn typos_become_errors_with_suggestions_and_are_never_applied() {
        let parsed = deck(vec![
            entry(1, "Lightnin Bolt", Section::Main),
            entry(1, "Sol Ring", Section::Main),
            entry(1, "Black Lotus", Section::Main),
        ]);
        let result = resolve_deck(&index(), &parsed);
        assert_eq!(result.resolved.len(), 1);
        assert_eq!(result.resolved[0].name, "Sol Ring");
        assert_eq!(result.unresolved.len(), 2);

        let typo = &result.unresolved[0];
        assert_eq!(typo.index, 0);
        assert_eq!(typo.entry.name, "Lightnin Bolt");
        match &typo.error {
            ResolveError::UnknownCard { name, suggestions } => {
                assert_eq!(name, "Lightnin Bolt");
                assert_eq!(suggestions[0].name, "Lightning Bolt");
            }
            other @ ResolveError::AmbiguousName { .. } => panic!("unexpected {other:?}"),
        }
        assert!(typo.error.to_string().contains("Lightnin Bolt"));

        let nonsense = &result.unresolved[1];
        assert_eq!(nonsense.index, 2);
        assert!(
            matches!(&nonsense.error, ResolveError::UnknownCard { suggestions, .. } if suggestions.is_empty())
        );
    }

    #[test]
    fn ambiguous_names_list_every_candidate() {
        let result = resolve_deck(&index(), &deck(vec![entry(1, "etude", Section::Main)]));
        assert_eq!(result.resolved, Vec::new());
        match &result.unresolved[0].error {
            ResolveError::AmbiguousName { candidates, .. } => {
                let mut names: Vec<_> = candidates.iter().map(|c| c.name.as_str()).collect();
                names.sort_unstable();
                assert_eq!(names, ["Etude", "Étude"]);
                assert!(candidates.iter().all(|c| c.distance == 0));
            }
            other @ ResolveError::UnknownCard { .. } => panic!("unexpected {other:?}"),
        }
        assert!(
            result.unresolved[0]
                .error
                .to_string()
                .contains("2 different cards")
        );
    }

    #[rstest]
    #[case::parenthetical_parsed_as_set(
        entry(1, "Hazmat Suit", Section::Main).with_printing("USED", None),
        Some(("Hazmat Suit (Used)", None))
    )]
    #[case::real_set_with_unknown_name(
        entry(1, "Hazmat Suits", Section::Main).with_printing("USED", None),
        None
    )]
    #[case::collector_number_means_it_really_was_a_set(
        entry(1, "Hazmat Suit", Section::Main).with_printing("USED", Some("163")),
        None
    )]
    #[case::plain_name_wins(
        entry(1, "Sol Ring", Section::Main).with_printing("C16", None),
        Some(("Sol Ring", Some("C16")))
    )]
    fn retries_a_trailing_parenthetical_as_part_of_the_name(
        #[case] input: DeckEntry,
        #[case] expected: Option<(&str, Option<&str>)>,
    ) {
        let result = resolve_deck(&index(), &deck(vec![input]));
        match expected {
            Some((name, set)) => {
                assert_eq!(result.resolved.len(), 1);
                assert_eq!(result.resolved[0].name, name);
                assert_eq!(result.resolved[0].set.as_deref(), set);
            }
            None => assert_eq!(result.unresolved.len(), 1),
        }
    }

    #[test]
    fn empty_deck_resolves_to_nothing() {
        assert_eq!(
            resolve_deck(&index(), &ParsedDeck::default()),
            Resolution::default()
        );
    }

    #[test]
    fn empty_index_resolves_nothing() {
        let result = resolve_deck(
            &CardIndex::new(),
            &deck(vec![entry(1, "Sol Ring", Section::Main)]),
        );
        assert_eq!(result.resolved, Vec::new());
        assert!(matches!(
            &result.unresolved[0].error,
            ResolveError::UnknownCard { suggestions, .. } if suggestions.is_empty()
        ));
    }

    proptest! {
        #[test]
        fn every_entry_lands_in_exactly_one_bucket(
            names in prop::collection::vec(
                prop_oneof![
                    Just("Lightning Bolt".to_owned()),
                    Just("etude".to_owned()),
                    Just("Sol Rign".to_owned()),
                    "[A-Za-z ,']{1,20}",
                ],
                0..40,
            )
        ) {
            let entries: Vec<_> = names.iter().map(|n| entry(1, n, Section::Main)).collect();
            let total = entries.len();
            let result = resolve_deck(&index(), &deck(entries));
            prop_assert_eq!(result.resolved.len() + result.unresolved.len(), total);
            let mut positions: Vec<usize> = result.unresolved.iter().map(|u| u.index).collect();
            positions.sort_unstable();
            positions.dedup();
            prop_assert_eq!(positions.len(), result.unresolved.len());
            prop_assert!(result.unresolved.iter().all(|u| u.index < total));
        }
    }
}
