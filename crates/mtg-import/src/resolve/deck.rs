//! Resolves the entries of a parsed deck to cards, reporting what could not be matched.

use thiserror::Error;

use super::index::{CardIndex, IndexedCard, Lookup, Suggestion};
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
    /// Several different cards share this name, or the name and the printing of the entry name different cards.
    #[error("{name:?} matches {} different cards", .candidates.len())]
    AmbiguousName {
        /// The name as written.
        name: String,
        /// The cards in question; the card of the printing comes first when there is one.
        candidates: Vec<Suggestion>,
    },
}

/// How a deck entry was tied to its card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    /// By set and collector number.
    Printing,
    /// By the name of the card, in full or by its front face.
    Name,
    /// By another name of a printing: a flavor name, or the name printed on a digital version.
    OtherName,
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
    /// What tied the entry to the card.
    pub matched: MatchKind,
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

fn candidate(card: &IndexedCard) -> Suggestion {
    Suggestion {
        oracle_id: card.oracle_id.clone(),
        name: card.name.clone(),
        distance: 0,
    }
}

/// What the name of an entry says by itself.
struct Named<'a> {
    found: Lookup<'a>,
    kind: MatchKind,
    /// Whether a trailing `(SET)` that the text parsers split off belongs to the name.
    set_was_part_of_name: bool,
}

/// Looks up an entry's name: card names first, retrying with a trailing `(SET)` as part of the name, then other names of printings.
fn lookup_name<'a>(index: &'a CardIndex, entry: &DeckEntry) -> Named<'a> {
    let named = |found, kind, set_was_part_of_name| Named {
        found,
        kind,
        set_was_part_of_name,
    };
    let direct = index.find(&entry.name);
    if direct != Lookup::Missing {
        return named(direct, MatchKind::Name, false);
    }
    // `Hazmat Suit (Used)` ends like a set code, so the text parsers split it off the name.
    if let (Some(set), None) = (&entry.set, &entry.collector_number) {
        let joined = index.find(&format!("{} ({set})", entry.name));
        if joined != Lookup::Missing {
            return named(joined, MatchKind::Name, true);
        }
    }
    named(index.find_alias(&entry.name), MatchKind::OtherName, false)
}

/// The card of the entry's printing, when the entry has a set and a collector number and exactly one card has them.
fn printed_card<'a>(index: &'a CardIndex, entry: &DeckEntry) -> Option<&'a IndexedCard> {
    let (set, number) = (entry.set.as_deref()?, entry.collector_number.as_deref()?);
    match index.find_printing(set, number) {
        Lookup::Found(card) => Some(card),
        Lookup::Ambiguous(_) | Lookup::Missing => None,
    }
}

/// Whether `name` is another name of a printing of `card`.
fn is_other_name_of(index: &CardIndex, name: &str, card: &IndexedCard) -> bool {
    match index.find_alias(name) {
        Lookup::Found(found) => found.oracle_id == card.oracle_id,
        Lookup::Ambiguous(found) => found.iter().any(|c| c.oracle_id == card.oracle_id),
        Lookup::Missing => false,
    }
}

/// What an entry came to.
enum Verdict<'a> {
    Card {
        card: &'a IndexedCard,
        matched: MatchKind,
        set_was_part_of_name: bool,
    },
    Ambiguous(Vec<&'a IndexedCard>),
    Missing,
}

/// The printing decides, unless the name names another card; then neither is chosen.
fn decide<'a>(index: &'a CardIndex, entry: &DeckEntry) -> Verdict<'a> {
    let named = lookup_name(index, entry);
    if let Some(printed) = printed_card(index, entry) {
        let by_printing = Verdict::Card {
            card: printed,
            matched: MatchKind::Printing,
            set_was_part_of_name: false,
        };
        return match named.found {
            Lookup::Missing => by_printing,
            Lookup::Found(card) if card.oracle_id == printed.oracle_id => by_printing,
            Lookup::Ambiguous(cards) if cards.iter().any(|c| c.oracle_id == printed.oracle_id) => {
                by_printing
            }
            _ if is_other_name_of(index, &entry.name, printed) => by_printing,
            Lookup::Found(card) => Verdict::Ambiguous(vec![printed, card]),
            Lookup::Ambiguous(cards) => {
                Verdict::Ambiguous(std::iter::once(printed).chain(cards).collect())
            }
        };
    }
    match named.found {
        Lookup::Found(card) => Verdict::Card {
            card,
            matched: named.kind,
            set_was_part_of_name: named.set_was_part_of_name,
        },
        Lookup::Ambiguous(cards) => Verdict::Ambiguous(cards),
        Lookup::Missing => Verdict::Missing,
    }
}

/// Resolves every entry of `deck` against `index`: the printing (set and collector number) first, then the name, then the other names of printings.
///
/// A printing and a name that point at different cards are reported, not chosen between; fuzzy matches are suggested, never applied.
pub fn resolve_deck(index: &CardIndex, deck: &ParsedDeck) -> Resolution {
    let mut resolution = Resolution::default();
    for (position, entry) in deck.entries.iter().enumerate() {
        match decide(index, entry) {
            Verdict::Card {
                card,
                matched,
                set_was_part_of_name,
            } => resolution.resolved.push(ResolvedEntry {
                count: entry.count,
                oracle_id: card.oracle_id.clone(),
                name: card.name.clone(),
                section: entry.section,
                set: entry.set.clone().filter(|_| !set_was_part_of_name),
                collector_number: entry.collector_number.clone(),
                matched,
            }),
            Verdict::Ambiguous(cards) => resolution.unresolved.push(UnresolvedEntry {
                index: position,
                entry: entry.clone(),
                error: ResolveError::AmbiguousName {
                    name: entry.name.clone(),
                    candidates: cards.into_iter().map(candidate).collect(),
                },
            }),
            Verdict::Missing => resolution.unresolved.push(UnresolvedEntry {
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

    /// Cards with printings and other names:
    /// the flavor name of a Secret Lair printing, the name printed on a digital version, a transform card with a flavor name of its own,
    /// and a printing whose flavor name is the name of another card.
    fn printed_index() -> CardIndex {
        let mut index = index();
        let id = |n: &str| OracleId(n.to_owned());
        for (oracle, name) in [
            ("stone", "Fellwar Stone"),
            ("camera", "Peter Parker's Camera"),
            ("delver", "Delver of Secrets // Insectile Aberration"),
            ("study", "Rhystic Study"),
        ] {
            index.insert(id(oracle), name);
        }
        for (oracle, set, number) in [
            ("stone", "SLD", "1040"),
            ("stone", "ema", "211"),
            ("camera", "SPM", "171"),
            ("camera", "OM1", "171"),
            ("delver", "ISD", "51"),
            ("study", "SLD", "478"),
            ("oracle-0", "M10", "146"),
            ("oracle-4", "ETU", "1"),
            ("oracle-0", "XYZ", "1"),
            ("oracle-1", "XYZ", "1"),
        ] {
            index.add_printing(&id(oracle), set, number);
        }
        for (oracle, alias) in [
            ("stone", "The Ball"),
            ("camera", "Phenomena Recorder"),
            ("delver", "Bug Catcher // Big Bug"),
            ("study", "Sol Ring"),
        ] {
            index.add_alias(&id(oracle), alias);
        }
        index
    }

    fn resolved_as(entry: DeckEntry) -> Option<(String, MatchKind)> {
        let result = resolve_deck(&printed_index(), &deck(vec![entry]));
        result.resolved.first().map(|r| (r.name.clone(), r.matched))
    }

    #[rstest]
    #[case::full_name("Lightning Bolt", "Lightning Bolt", MatchKind::Name)]
    #[case::case_and_accents("aether VIAL", "Æther Vial", MatchKind::Name)]
    #[case::split_full("Fire // Ice", "Fire // Ice", MatchKind::Name)]
    #[case::split_single_slash("Fire/Ice", "Fire // Ice", MatchKind::Name)]
    #[case::split_spaced_slash("Fire / Ice", "Fire // Ice", MatchKind::Name)]
    #[case::split_arena("Fire /// Ice", "Fire // Ice", MatchKind::Name)]
    #[case::split_front_only("Fire", "Fire // Ice", MatchKind::Name)]
    #[case::transform_front_only(
        "Delver of Secrets",
        "Delver of Secrets // Insectile Aberration",
        MatchKind::Name
    )]
    #[case::transform_moxfield(
        "Delver of Secrets / Insectile Aberration",
        "Delver of Secrets // Insectile Aberration",
        MatchKind::Name
    )]
    #[case::flavor_name("The Ball", "Fellwar Stone", MatchKind::OtherName)]
    #[case::flavor_name_lowercase("the ball", "Fellwar Stone", MatchKind::OtherName)]
    #[case::digital_name("Phenomena Recorder", "Peter Parker's Camera", MatchKind::OtherName)]
    #[case::flavor_name_of_a_transform_card(
        "Bug Catcher // Big Bug",
        "Delver of Secrets // Insectile Aberration",
        MatchKind::OtherName
    )]
    #[case::flavor_front_of_a_transform_card(
        "Bug Catcher",
        "Delver of Secrets // Insectile Aberration",
        MatchKind::OtherName
    )]
    #[case::card_name_beats_the_flavor_name_of_another_card(
        "Sol Ring",
        "Sol Ring",
        MatchKind::Name
    )]
    fn resolves_each_form_of_a_name_with_no_printing(
        #[case] written: &str,
        #[case] card: &str,
        #[case] matched: MatchKind,
    ) {
        let found = resolved_as(entry(1, written, Section::Main));
        assert_eq!(found, Some((card.to_owned(), matched)));
    }

    #[rstest]
    #[case::flavor_name_with_its_printing("The Ball", "SLD", "1040", "Fellwar Stone")]
    #[case::card_name_with_its_printing("Fellwar Stone", "SLD", "1040", "Fellwar Stone")]
    #[case::lowercase_set("Fellwar Stone", "sld", "1040", "Fellwar Stone")]
    #[case::another_printing_of_the_card("The Ball", "EMA", "211", "Fellwar Stone")]
    #[case::unknown_name_with_a_known_printing("The Bell", "SLD", "1040", "Fellwar Stone")]
    #[case::digital_printing("Phenomena Recorder", "OM1", "171", "Peter Parker's Camera")]
    #[case::paper_printing_under_the_digital_name(
        "Phenomena Recorder",
        "SPM",
        "171",
        "Peter Parker's Camera"
    )]
    #[case::front_face_with_printing(
        "Delver of Secrets",
        "ISD",
        "51",
        "Delver of Secrets // Insectile Aberration"
    )]
    #[case::printing_wins_over_a_card_name_that_is_its_flavor_name(
        "Sol Ring",
        "SLD",
        "478",
        "Rhystic Study"
    )]
    #[case::printing_settles_an_ambiguous_name("Etude", "ETU", "1", "Étude")]
    fn the_printing_decides_before_the_name(
        #[case] written: &str,
        #[case] set: &str,
        #[case] number: &str,
        #[case] card: &str,
    ) {
        let found = resolved_as(entry(1, written, Section::Main).with_printing(set, Some(number)));
        assert_eq!(found, Some((card.to_owned(), MatchKind::Printing)));
    }

    #[rstest]
    #[case::unknown_number("Lightning Bolt", "M10", Some("999"), MatchKind::Name)]
    #[case::unknown_set("Lightning Bolt", "ZZZ", Some("146"), MatchKind::Name)]
    #[case::set_without_number("Lightning Bolt", "M10", None, MatchKind::Name)]
    #[case::flavor_name_of_an_unknown_printing("The Ball", "ZZZ", Some("1"), MatchKind::OtherName)]
    #[case::printing_shared_by_two_cards_is_not_decisive(
        "Sol Ring",
        "XYZ",
        Some("1"),
        MatchKind::Name
    )]
    fn without_a_known_printing_the_name_decides(
        #[case] written: &str,
        #[case] set: &str,
        #[case] number: Option<&str>,
        #[case] matched: MatchKind,
    ) {
        let found = resolved_as(entry(1, written, Section::Main).with_printing(set, number));
        assert_eq!(found.map(|f| f.1), Some(matched));
    }

    #[rstest]
    #[case::card_name_of_another_card("Lightning Bolt", "SLD", "1040", "Fellwar Stone")]
    #[case::split_card_name("Fire // Ice", "SLD", "1040", "Fellwar Stone")]
    #[case::ambiguous_name_without_the_printing_card("Etude", "SLD", "1040", "Fellwar Stone")]
    fn a_name_and_a_printing_of_different_cards_are_both_reported(
        #[case] written: &str,
        #[case] set: &str,
        #[case] number: &str,
        #[case] printed: &str,
    ) {
        let entry = entry(1, written, Section::Main).with_printing(set, Some(number));
        let result = resolve_deck(&printed_index(), &deck(vec![entry]));
        assert_eq!(result.resolved, Vec::new());
        match &result.unresolved[0].error {
            ResolveError::AmbiguousName { name, candidates } => {
                assert_eq!(name, written);
                assert_eq!(
                    candidates[0].name, printed,
                    "the printing's card comes first"
                );
                assert!(candidates.len() >= 2);
            }
            other @ ResolveError::UnknownCard { .. } => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn other_names_of_unknown_cards_get_suggestions_under_the_card_name() {
        let result = resolve_deck(
            &printed_index(),
            &deck(vec![entry(1, "Phenomena Recorer", Section::Main)]),
        );
        match &result.unresolved[0].error {
            ResolveError::UnknownCard { suggestions, .. } => {
                assert_eq!(suggestions[0].name, "Peter Parker's Camera");
            }
            other @ ResolveError::AmbiguousName { .. } => panic!("unexpected {other:?}"),
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
        fn a_known_printing_is_never_overruled_by_a_name(
            name in prop_oneof![
                Just("The Ball".to_owned()),
                Just("Fellwar Stone".to_owned()),
                Just("Sol Ring".to_owned()),
                Just("Fire/Ice".to_owned()),
                Just("etude".to_owned()),
                "[A-Za-z ,'/]{0,20}",
            ]
        ) {
            let entry = entry(1, &name, Section::Main).with_printing("sld", Some("1040"));
            let result = resolve_deck(&printed_index(), &deck(vec![entry]));
            match (result.resolved.as_slice(), result.unresolved.as_slice()) {
                ([only], []) => prop_assert_eq!(&only.name, "Fellwar Stone"),
                ([], [only]) => match &only.error {
                    ResolveError::AmbiguousName { candidates, .. } => {
                        prop_assert_eq!(&candidates[0].name, "Fellwar Stone");
                    }
                    other @ ResolveError::UnknownCard { .. } => {
                        prop_assert!(false, "unexpected {other:?}");
                    }
                },
                other => prop_assert!(false, "unexpected {other:?}"),
            }
        }

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
