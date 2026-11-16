//! r66-00 abilities whose wordings no other reader or writer joins: each entry of `r6600_wholes.json` is an ability with
//! the words the card prints for it (the card's name as `~`), read when a clause says exactly those words and written
//! back from the ability.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R6600_WHOLES: &str = include_str!("r6600_wholes.json");

/// The entries parsed once.
static R6600_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6600_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose wording is the clause.
pub(super) fn r6600_whole_read(text: &str) -> Option<Ability> {
    R6600_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r6600_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R6600_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, and a near miss of its
    //! words (a type, a zone, a player, a duration or a condition changed) reads as nothing.
    use super::{R6600_SHAPES, R6600_WHOLES, r6600_add_wholes, r6600_whole_read};

    #[test]
    fn r6600_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6600_WHOLES).unwrap();
        assert_eq!(R6600_SHAPES.len(), entries.len());
        for (ability, text) in R6600_SHAPES.iter() {
            assert_eq!(r6600_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r6600_add_wholes(ability, &mut texts);
            assert!(texts.contains(text), "{text}");
        }
    }

    #[test]
    fn r6600_a_near_miss_reads_as_nothing() {
        let swaps = [
            ("nonland permanent", "permanent"),
            ("your graveyard", "a graveyard"),
            ("haven't cast", "have cast"),
            ("this turn", "until your next turn"),
            ("you may cast", "you may play"),
            ("as a sorcery", "as an instant"),
            ("If you do, ", ""),
            ("{T}: ", "{1}: "),
            ("an opponent", "a player"),
            ("one or more", "a"),
            ("that much damage", "1 damage"),
        ];
        for (_, text) in R6600_SHAPES.iter() {
            for (from, to) in swaps {
                if text.contains(from) {
                    let near = text.replacen(from, to, 1);
                    assert_eq!(r6600_whole_read(&near), None, "{near}");
                }
            }
        }
    }
}
