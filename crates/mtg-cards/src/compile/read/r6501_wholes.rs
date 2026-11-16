//! r65-01 abilities whose wordings no other reader or writer joins: each entry of `r6501_wholes.json` is an ability with
//! the words the card prints for it (the card's name as `~`), read when a clause says exactly those words and written
//! back from the ability, so another count, step or event in the same ability never reads back.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R6501_WHOLES: &str = include_str!("r6501_wholes.json");

/// The entries parsed once.
static R6501_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6501_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose wording is the clause.
pub(super) fn r6501_whole_read(text: &str) -> Option<Ability> {
    R6501_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r6501_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R6501_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, and a near miss of its
    //! words (a number, a player or a duration changed) reads as nothing.
    use super::{R6501_SHAPES, R6501_WHOLES, r6501_add_wholes, r6501_whole_read};

    #[test]
    fn r6501_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6501_WHOLES).unwrap();
        assert_eq!(R6501_SHAPES.len(), entries.len());
        for (ability, text) in R6501_SHAPES.iter() {
            assert_eq!(r6501_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r6501_add_wholes(ability, &mut texts);
            assert!(texts.contains(text), "{text}");
        }
    }

    #[test]
    fn r6501_a_near_miss_reads_as_nothing() {
        let swaps = [
            ("white enchantment", "enchantment"),
            ("you control that", "an opponent controls that"),
            ("doesn't have", "has"),
            ("{1}", "{2}"),
            ("one color word", "one creature type"),
            ("attacking you or", "attacking an opponent or"),
            ("blocking that creature", "attacking"),
            ("at end of combat", "at end of turn"),
            ("Exile those tokens", "Sacrifice those tokens"),
            ("For each creature", "For each artifact"),
            ("Double the amount", "Triple the amount"),
            ("each type of", "one type of"),
            ("mana you have", "mana an opponent has"),
            ("{3}, {T}", "{2}, {T}"),
            ("any number of players'", "two players'"),
            ("life totals", "hands"),
        ];
        for (_, text) in R6501_SHAPES.iter() {
            for (from, to) in swaps {
                if text.contains(from) {
                    let near = text.replacen(from, to, 1);
                    assert_eq!(r6501_whole_read(&near), None, "{near}");
                }
            }
        }
    }
}
