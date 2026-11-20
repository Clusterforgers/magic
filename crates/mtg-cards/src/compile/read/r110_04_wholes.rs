//! r110-04 abilities whose wordings no other reader or writer joins (counters removed again for each put this way,
//! counted prevention, divided damage of a counted X): each entry of `r110_04_wholes.json` is an ability with the words
//! the card prints for it (the card's name as `~`), read when a clause says exactly those words and written back from
//! the ability.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R110_04_WHOLES: &str = include_str!("r110_04_wholes.json");

/// The entries parsed once.
static R110_04_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R110_04_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose wording is the clause.
pub(super) fn r110_04_whole_read(text: &str) -> Option<Ability> {
    R110_04_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r110_04_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R110_04_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, and a near miss of its
    //! words (a kind, a step, a number or a recipient changed) reads as nothing.
    use super::{R110_04_SHAPES, R110_04_WHOLES, r110_04_add_wholes, r110_04_whole_read};

    #[test]
    fn r110_04_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R110_04_WHOLES).unwrap();
        assert_eq!(R110_04_SHAPES.len(), entries.len());
        for (ability, text) in R110_04_SHAPES.iter() {
            assert_eq!(r110_04_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r110_04_add_wholes(ability, &mut texts);
            assert!(texts.contains(text), "{text}");
        }
    }

    #[test]
    fn r110_04_a_near_miss_reads_as_nothing() {
        let swaps = [
            ("three +1/+1", "two +1/+1"),
            ("three -1/-1", "four -1/-1"),
            ("next cleanup step", "next end step"),
            ("next end step", "next upkeep"),
            (
                "one, two, or three target creatures",
                "any number of target creatures",
            ),
            ("remove a +1/+1 counter", "remove a -1/-1 counter"),
            ("remove a -1/-1 counter", "put a -1/-1 counter"),
            ("greatest amount of damage", "total amount of damage"),
            (
                "by a source to a permanent or player",
                "by a source you control",
            ),
            (
                "any number of target creatures, where",
                "any number of targets, where",
            ),
            ("you may prevent X", "prevent X"),
            (
                "that much damage to any target",
                "that much damage to target creature",
            ),
            (
                "a +1/+1 counter on it, prevent",
                "a -1/-1 counter on it, prevent",
            ),
            ("number of Clerics", "number of Wizards"),
            (
                "After the first combat phase",
                "After the second combat phase",
            ),
            (
                "At the beginning of that combat",
                "At the beginning of each combat",
            ),
            ("creatures that attacked this turn", "creatures you control"),
            ("a counter of each kind not on", "a counter of each kind on"),
            ("one or more counters", "a counter"),
            ("If you do, draw a card", "Draw a card"),
            ("permanent you control onto", "creature you control onto"),
            ("damage to a player", "damage to a creature"),
        ];
        for (_, text) in R110_04_SHAPES.iter() {
            for (from, to) in swaps {
                if text.contains(from) {
                    let near = text.replacen(from, to, 1);
                    assert_eq!(r110_04_whole_read(&near), None, "{near}");
                }
            }
        }
    }
}
