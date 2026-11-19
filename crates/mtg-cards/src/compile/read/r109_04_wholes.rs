//! r109-04 abilities whose wordings no other reader or writer joins ("the next time" shields and next-spell rules, draw
//! replacements): each entry of `r109_04_wholes.json` is an ability with the words the card prints for it (the card's
//! name as `~`), read when a clause says exactly those words and written back from the ability.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R109_04_WHOLES: &str = include_str!("r109_04_wholes.json");

/// The entries parsed once.
static R109_04_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R109_04_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose wording is the clause.
pub(super) fn r109_04_whole_read(text: &str) -> Option<Ability> {
    R109_04_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r109_04_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R109_04_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, and a near miss of its
    //! words (a zone, a duration, a kind or a number changed) reads as nothing.
    use super::{R109_04_SHAPES, R109_04_WHOLES, r109_04_add_wholes, r109_04_whole_read};

    #[test]
    fn r109_04_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R109_04_WHOLES).unwrap();
        assert_eq!(R109_04_SHAPES.len(), entries.len());
        for (ability, text) in R109_04_SHAPES.iter() {
            assert_eq!(r109_04_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r109_04_add_wholes(ability, &mut texts);
            assert!(texts.contains(text), "{text}");
        }
    }

    #[test]
    fn r109_04_a_near_miss_reads_as_nothing() {
        let swaps = [
            ("from your hand", "from exile"),
            ("this turn", "this game"),
            ("into your hand", "into exile"),
            ("instant or sorcery", "creature"),
            ("permanent you control", "creature you control"),
            ("next end step", "next upkeep"),
            ("two additional", "three additional"),
            ("enchantment creatures", "artifact creatures"),
            ("nonlegendary creature", "creature"),
            ("creatures or planeswalkers", "creatures"),
            ("target land", "target creature"),
            ("remove all damage marked on it", "regenerate it"),
            ("four cards", "three cards"),
            ("opponents' libraries", "libraries"),
            ("seven cards", "six cards"),
        ];
        for (_, text) in R109_04_SHAPES.iter() {
            for (from, to) in swaps {
                if text.contains(from) {
                    let near = text.replacen(from, to, 1);
                    assert_eq!(r109_04_whole_read(&near), None, "{near}");
                }
            }
        }
    }
}
