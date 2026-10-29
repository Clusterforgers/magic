//! Which cards a run asks about: the ones named, or a seeded sample of the stored cards nothing covers yet.

use std::collections::BTreeSet;
use std::str::FromStr;

use mtg_cards::compile::compile;
use mtg_cards::load::{Library, slug};
use mtg_cards::support::{Support, support};
use mtg_cards::{Caps, PrintedCard};

use crate::audit::rank;
use crate::error::CliError;

/// The width of a length bucket: a card is preferred over a longer one only when it is a whole bucket shorter.
const BUCKET: usize = 100;

/// The first letters a sample takes cards from: `A-L`, `A-F,M`, `m`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Letters(Vec<(char, char)>);

impl FromStr for Letters {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let mut ranges = Vec::new();
        for part in text.split(',') {
            let chars: Vec<char> = part
                .trim()
                .chars()
                .map(|c| c.to_ascii_lowercase())
                .collect();
            let range = match chars.as_slice() {
                [one] => Some((*one, *one)),
                [from, '-', to] => Some((*from, *to)),
                _ => None,
            };
            match range {
                Some((from, to))
                    if from.is_ascii_lowercase() && from <= to && to.is_ascii_lowercase() =>
                {
                    ranges.push((from, to));
                }
                _ => {
                    return Err(format!(
                        "`{part}` is not a letter or a range of letters such as A-L"
                    ));
                }
            }
        }
        Ok(Letters(ranges))
    }
}

impl Letters {
    /// Whether the name begins with one of the letters; accents and ligatures fold as in file names (`Æther` is `a`).
    pub fn admit(&self, name: &str) -> bool {
        slug(name).chars().next().is_some_and(|first| {
            self.0
                .iter()
                .any(|(from, to)| (*from..=*to).contains(&first))
        })
    }
}

/// The positions in `cards` of the named cards, in the order named, each once. A name selects the card of that name or
/// that has a face of that name, in any case.
pub fn by_names(cards: &[PrintedCard], names: &[String]) -> Result<Vec<usize>, CliError> {
    let mut found = Vec::new();
    let mut missing = Vec::new();
    for name in names {
        let lower = name.to_lowercase();
        let at = cards.iter().position(|card| {
            card.name.to_lowercase() == lower
                || card
                    .faces
                    .iter()
                    .any(|face| face.name.to_lowercase() == lower)
        });
        match at {
            Some(at) if !found.contains(&at) => found.push(at),
            Some(_) => {}
            None => missing.push(format!("`{name}`")),
        }
    }
    if missing.is_empty() {
        Ok(found)
    } else {
        Err(CliError::Config(format!(
            "no stored card is named {}",
            missing.join(", ")
        )))
    }
}

/// Whether the compiler's rules for the card are all the engine plays, so a proposal adds nothing.
fn is_verified(card: &PrintedCard, caps: &Caps) -> bool {
    let compiled = compile(card);
    support(compiled.card.as_ref(), card, caps).support == Support::Verified
}

/// The positions of at most `count` cards for a sample: cards with a text, no hand-written file and a support state other
/// than `Verified` (among the oracle ids of `within` when given), short ones first (by buckets of 100 characters), within
/// a bucket in the order of the seed. The same seed gives the same cards whatever order the cards come in.
pub fn sample(
    cards: &[PrintedCard],
    library: &Library,
    caps: &Caps,
    count: usize,
    seed: u64,
    letters: Option<&Letters>,
    within: Option<&BTreeSet<String>>,
) -> Vec<usize> {
    let length = |card: &PrintedCard| {
        card.faces
            .iter()
            .map(|face| face.oracle_text.chars().count())
            .sum::<usize>()
    };
    let mut pool: Vec<(usize, u64, usize)> = cards
        .iter()
        .enumerate()
        .filter(|(_, card)| letters.is_none_or(|letters| letters.admit(&card.name)))
        .filter(|(_, card)| within.is_none_or(|within| within.contains(&card.oracle_id)))
        .filter(|(_, card)| library.card(&card.oracle_id).is_none())
        .filter(|(_, card)| card.faces.iter().any(|face| !face.oracle_text.is_empty()))
        .map(|(at, card)| (length(card) / BUCKET, rank(seed, &card.oracle_id), at))
        .collect();
    pool.sort_by(|a, b| (a.0, a.1, &cards[a.2].oracle_id).cmp(&(b.0, b.1, &cards[b.2].oracle_id)));
    pool.into_iter()
        .map(|(_, _, at)| at)
        .filter(|at| !is_verified(&cards[*at], caps))
        .take(count)
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: letters select names by their first letter as file names fold it, names select cards in any case and report
    //! what is missing, and a sample is short first, seeded, leaves out cards a file or the compiler already covers, and keeps its order within a pool.
    use mtg_cards::PrintedFace;
    use rstest::rstest;

    use super::*;

    fn card(name: &str, text: &str) -> PrintedCard {
        PrintedCard {
            oracle_id: format!("id-{name}"),
            name: name.to_owned(),
            keywords: Vec::new(),
            layout: "normal".into(),
            faces: vec![PrintedFace {
                name: name.to_owned(),
                type_line: "Sorcery".into(),
                oracle_text: text.to_owned(),
                ..PrintedFace::default()
            }],
        }
    }

    #[rstest]
    #[case("A-L", "Abzan Charm", true)]
    #[case("A-L", "Lightning Bolt", true)]
    #[case("A-L", "Mind Stone", false)]
    #[case("a-l", "Æther Vial", true)]
    #[case("A-F,M", "Mind Stone", true)]
    #[case("A-F,M", "Guttersnipe", false)]
    #[case("m", "Mind Stone", true)]
    #[case("A-L", "\"Ach! Hans, Run!\"", true)]
    fn letters_select_names_by_their_first_letter(
        #[case] letters: &str,
        #[case] name: &str,
        #[case] admitted: bool,
    ) {
        assert_eq!(letters.parse::<Letters>().unwrap().admit(name), admitted);
    }

    #[rstest]
    #[case("")]
    #[case("AL")]
    #[case("L-A")]
    #[case("1-5")]
    #[case("A-")]
    #[case("A,,B")]
    fn bad_letters_are_refused(#[case] letters: &str) {
        assert!(letters.parse::<Letters>().is_err());
    }

    #[test]
    fn names_select_cards_in_any_case_and_each_once() {
        let cards = vec![card("Opt", "Scry 1."), card("Wind Drake", "Flying")];
        let names = [
            "wind drake".to_owned(),
            "OPT".to_owned(),
            "Wind Drake".to_owned(),
        ];
        assert_eq!(by_names(&cards, &names).unwrap(), vec![1, 0]);
    }

    #[test]
    fn a_name_no_card_has_is_reported() {
        let cards = vec![card("Opt", "Scry 1.")];
        let error = by_names(&cards, &["Opt".to_owned(), "Nope".to_owned()]).unwrap_err();
        assert_eq!(error.to_string(), "no stored card is named `Nope`");
    }

    #[test]
    fn a_sample_is_short_first_seeded_and_without_covered_cards() {
        let library = mtg_cards::builtin::library().unwrap();
        let caps = mtg_engine::caps();
        // Text no template reads, so the compiler never makes these cards `Verified`.
        let long = "Frobnicate the quux whenever a wibble gloops thrice. ".repeat(4);
        // A card a hand-written file covers is never taken.
        let covered = library.cards().next().unwrap();
        let mut hand_written = card(&covered.name, "Draw a card.");
        hand_written.oracle_id.clone_from(&covered.oracle_id);
        let cards = vec![
            card("Zed Long", &long),
            card("Alpha Short", "Frobnicate the quux."),
            card("Beta Short", "Gloop the wibble twice."),
            card("Gamma Short", "Wibble every quux."),
            hand_written,
        ];
        let first = sample(&cards, library, &caps, 2, 7, None, None);
        assert_eq!(
            first,
            sample(&cards, library, &caps, 2, 7, None, None),
            "same seed, same cards"
        );
        assert_eq!(first.len(), 2);
        assert!(
            first.iter().all(|at| *at != 0),
            "the long card comes after the short ones"
        );
        let all = sample(&cards, library, &caps, 10, 7, None, None);
        assert_eq!(&all[..2], &first[..], "a smaller count is a prefix");
        assert_eq!(all.len(), 4, "the covered card is left out");
        assert!(!all.contains(&4));
        let reversed: Vec<PrintedCard> = cards.iter().rev().cloned().collect();
        let names = |picked: &[usize], from: &[PrintedCard]| -> Vec<String> {
            picked.iter().map(|at| from[*at].name.clone()).collect()
        };
        assert_eq!(
            names(
                &sample(&reversed, library, &caps, 3, 7, None, None),
                &reversed
            ),
            names(&sample(&cards, library, &caps, 3, 7, None, None), &cards),
            "the order of the cards does not matter"
        );
    }

    #[test]
    fn a_sample_takes_only_the_letters_asked_for() {
        let library = mtg_cards::builtin::library().unwrap();
        let caps = mtg_engine::caps();
        let cards = vec![
            card("Alpha Short", "Frobnicate the quux."),
            card("Mu Short", "Gloop the wibble twice."),
        ];
        let letters: Letters = "M-Z".parse().unwrap();
        assert_eq!(
            sample(&cards, library, &caps, 5, 1, Some(&letters), None),
            vec![1]
        );
    }

    #[rstest]
    #[case::one_seed(1)]
    #[case::another_seed(42)]
    fn a_sample_within_a_pool_keeps_the_order_of_the_whole_sample(#[case] seed: u64) {
        let library = mtg_cards::builtin::library().unwrap();
        let caps = mtg_engine::caps();
        let long = "Frobnicate the quux whenever a wibble gloops thrice. ".repeat(4);
        let cards = vec![
            card("Zed Long", &long),
            card("Alpha Short", "Frobnicate the quux."),
            card("Beta Short", "Gloop the wibble twice."),
            card("Gamma Short", "Wibble every quux."),
        ];
        let within: BTreeSet<String> =
            ["id-Zed Long", "id-Gamma Short", "id-Alpha Short", "id-Gone"]
                .into_iter()
                .map(str::to_owned)
                .collect();
        let whole = sample(&cards, library, &caps, 10, seed, None, None);
        let kept: Vec<usize> = whole
            .into_iter()
            .filter(|at| within.contains(&cards[*at].oracle_id))
            .collect();
        let picked = sample(&cards, library, &caps, 10, seed, None, Some(&within));
        assert_eq!(picked, kept);
        assert_eq!(picked.len(), 3);
        assert_eq!(picked.last(), Some(&0), "the long card still comes last");
        assert_eq!(
            sample(&cards, library, &caps, 2, seed, None, Some(&within)),
            picked[..2],
            "a resumed run takes the same cards first"
        );
    }
}
