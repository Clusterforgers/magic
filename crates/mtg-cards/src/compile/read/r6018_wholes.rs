//! r60-18 abilities whose wordings no other reader or writer joins: each entry of `r6018_wholes.json` is an ability with
//! the words the card prints for it (the card's name as `~`), read when a clause says exactly those words and written
//! back from the ability, so another count, step or event in the same ability never reads back.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R6018_WHOLES: &str = include_str!("r6018_wholes.json");

/// The entries parsed once.
static R6018_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6018_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose wording is the clause.
pub(super) fn r6018_whole_read(text: &str) -> Option<Ability> {
    R6018_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r6018_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R6018_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, and a near miss of its
    //! words (a number, a player or a duration changed) reads as nothing.
    use super::{R6018_SHAPES, R6018_WHOLES, r6018_add_wholes, r6018_whole_read};

    #[test]
    fn r6018_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6018_WHOLES).unwrap();
        assert_eq!(R6018_SHAPES.len(), entries.len());
        for (ability, text) in R6018_SHAPES.iter() {
            assert_eq!(r6018_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r6018_add_wholes(ability, &mut texts);
            assert!(texts.contains(text), "{text}");
        }
    }

    #[test]
    fn r6018_a_near_miss_reads_as_nothing() {
        let swaps = [
            ("one", "two"),
            ("two", "three"),
            ("ten", "nine"),
            ("+1/+1", "-1/-1"),
            ("1", "4"),
            ("2", "5"),
            ("you", "each opponent"),
            ("opponent", "player"),
            ("permanent types", "card types"),
            ("end of turn", "end of combat"),
            ("graveyard", "library"),
        ];
        for (_, text) in R6018_SHAPES.iter() {
            for (from, to) in swaps {
                if text.contains(from) {
                    let near = text.replacen(from, to, 1);
                    assert_eq!(r6018_whole_read(&near), None, "{near}");
                }
            }
        }
    }
}
