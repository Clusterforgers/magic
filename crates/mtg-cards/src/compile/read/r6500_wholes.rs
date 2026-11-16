//! r65-00 abilities whose wordings no other reader or writer joins: each entry of `r6500_wholes.json` is an ability with
//! the words the card prints for it (the card's name as `~`), read when a clause says exactly those words and written
//! back from the ability, so another count, step or event in the same ability never reads back.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R6500_WHOLES: &str = include_str!("r6500_wholes.json");

/// The entries parsed once.
static R6500_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6500_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose wording is the clause.
pub(super) fn r6500_whole_read(text: &str) -> Option<Ability> {
    R6500_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r6500_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R6500_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, and a near miss of its
    //! words (a number, a player or a duration changed) reads as nothing.
    use super::{R6500_SHAPES, R6500_WHOLES, r6500_add_wholes, r6500_whole_read};

    #[test]
    fn r6500_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6500_WHOLES).unwrap();
        assert_eq!(R6500_SHAPES.len(), entries.len());
        for (ability, text) in R6500_SHAPES.iter() {
            assert_eq!(r6500_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r6500_add_wholes(ability, &mut texts);
            assert!(texts.contains(text), "{text}");
        }
    }

    #[test]
    fn r6500_a_near_miss_reads_as_nothing() {
        let swaps = [
            (
                "toughness rather than its power",
                "power rather than its toughness",
            ),
            ("toughness greater than", "toughness less than"),
            ("seven", "six"),
            ("dream counters", "charge counters"),
            ("you control", "an opponent controls"),
            ("untapped", "tapped"),
            ("0 loyalty", "1 loyalty"),
            ("Planeswalkers", "Creatures"),
        ];
        for (_, text) in R6500_SHAPES.iter() {
            for (from, to) in swaps {
                if text.contains(from) {
                    let near = text.replacen(from, to, 1);
                    assert_eq!(r6500_whole_read(&near), None, "{near}");
                }
            }
        }
    }
}
