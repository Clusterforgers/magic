//! r67-03 abilities whose wordings no other reader or writer joins: each entry of `r6703_wholes.json` is an ability with
//! the words the card prints for it (the card's name as `~`), read when a clause says exactly those words and written
//! back from the ability, so another count, step or event in the same ability never reads back.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R6703_WHOLES: &str = include_str!("r6703_wholes.json");

/// Pairs of abilities one sentence prints, as JSON, each with its wording (`r6703_pairs.json`: a list of
/// `[[first, second], wording]`).
const R6703_PAIRS: &str = include_str!("r6703_pairs.json");

/// The pairs parsed once.
static R6703_PAIR_SHAPES: LazyLock<Vec<([Ability; 2], String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<([serde_json::Value; 2], String)>>(R6703_PAIRS)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|([first, second], text)| {
            Some((
                [
                    serde_json::from_value(first).ok()?,
                    serde_json::from_value(second).ok()?,
                ],
                text,
            ))
        })
        .collect()
});

/// The wordings of two abilities one of the pairs matches in order.
pub(super) fn r6703_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    R6703_PAIR_SHAPES
        .iter()
        .filter(|([one, two], _)| one == first && two == second)
        .map(|(_, text)| text.clone())
        .collect()
}

/// The two abilities of the pair whose wording is the whole of the rest of the sentence.
pub(super) fn r6703_pair_read(r: &mut super::tokens::Reader<'_>) -> Option<Vec<Ability>> {
    R6703_PAIR_SHAPES
        .iter()
        .find_map(|([first, second], text)| {
            r.attempt(|r| {
                (r.r376_text(text) && r.end()).then(|| vec![first.clone(), second.clone()])
            })
        })
}

/// The entries parsed once.
static R6703_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6703_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose wording is the clause.
pub(super) fn r6703_whole_read(text: &str) -> Option<Ability> {
    R6703_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r6703_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R6703_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, and a near miss of its
    //! words (a number, a player or a duration changed) reads as nothing.
    use super::{
        R6703_PAIR_SHAPES, R6703_PAIRS, R6703_SHAPES, R6703_WHOLES, r6703_add_wholes,
        r6703_pair_texts, r6703_whole_read,
    };

    #[test]
    fn r6703_every_pair_parses_and_writes_its_wording_in_order() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6703_PAIRS).unwrap();
        assert_eq!(R6703_PAIR_SHAPES.len(), entries.len());
        for ([first, second], text) in R6703_PAIR_SHAPES.iter() {
            assert_eq!(r6703_pair_texts(first, second), std::slice::from_ref(text));
            assert!(
                r6703_pair_texts(second, first).is_empty(),
                "the order matters"
            );
        }
    }

    #[test]
    fn r6703_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6703_WHOLES).unwrap();
        assert_eq!(R6703_SHAPES.len(), entries.len());
        for (ability, text) in R6703_SHAPES.iter() {
            assert_eq!(r6703_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r6703_add_wholes(ability, &mut texts);
            assert!(texts.contains(text), "{text}");
        }
    }

    #[test]
    fn r6703_a_near_miss_reads_as_nothing() {
        let swaps = [
            ("waterbend, ", ""),
            ("or airbend", "or bend"),
            ("draw a card", "draw two cards"),
            ("all four", "all three"),
            ("this turn", "this game"),
            ("Whenever you", "Whenever an opponent"),
            ("transform ~", "untap ~"),
            ("pay {2}", "pay {3}"),
            ("Jaces you control", "Jaces"),
            ("each opponent's turn", "your turn"),
            ("creatures they control", "creatures you control"),
            ("artifact or creature spell", "creature spell"),
            ("under your control", "under its owner's control"),
            ("Counter target spell", "Counter target creature spell"),
            ("rounded down", "rounded up"),
            ("Add {C}{C}", "Add {C}"),
            ("with {X} in its mana cost", "with {X}"),
            (
                "noncombat damage to a creature",
                "combat damage to a creature",
            ),
            (
                "equal to that creature's toughness",
                "equal to that creature's power",
            ),
            ("plus X instead", "plus 1 instead"),
            ("a source you control would", "a source would"),
        ];
        for (_, text) in R6703_SHAPES.iter() {
            for (from, to) in swaps {
                if text.contains(from) {
                    let near = text.replacen(from, to, 1);
                    assert_eq!(r6703_whole_read(&near), None, "{near}");
                }
            }
        }
    }
}
