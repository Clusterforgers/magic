//! r66-02 abilities whose wordings no other reader or writer joins: each entry of `r6602_wholes.json` is an ability with
//! the words the card prints for it (the card's name as `~`), read when a clause says exactly those words and written
//! back from the ability, so another count, step or event in the same ability never reads back.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R6602_WHOLES: &str = include_str!("r6602_wholes.json");

/// The entries parsed once.
static R6602_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6602_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose wording is the clause.
pub(super) fn r6602_whole_read(text: &str) -> Option<Ability> {
    R6602_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r6602_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R6602_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, and a near miss of its
    //! words (a token, a player, a duration or an event changed) reads as nothing.
    use super::{R6602_SHAPES, R6602_WHOLES, r6602_add_wholes, r6602_whole_read};

    #[test]
    fn r6602_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6602_WHOLES).unwrap();
        assert_eq!(R6602_SHAPES.len(), entries.len());
        for (ability, text) in R6602_SHAPES.iter() {
            assert_eq!(r6602_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r6602_add_wholes(ability, &mut texts);
            assert!(texts.contains(text), "{text}");
        }
    }

    #[test]
    fn r6602_a_near_miss_reads_as_nothing() {
        let swaps = [
            ("Wicked Role", "Cursed Role"),
            ("tapped under", "under"),
            ("you control gains", "an opponent controls gains"),
            ("its owner's control", "your control"),
            ("Until end of turn, target", "Target"),
            ("dies", "attacks"),
            ("starting with you, ", ""),
            ("you don't control", "you control"),
            ("may choose an artifact", "may choose a creature"),
            ("Each opponent may copy", "Each player may copy"),
            ("plus an additional time", "plus two additional times"),
            ("Discard a card with mana value X", "Discard a card"),
            ("Counter target spell", "Counter target creature spell"),
            ("fewer than seven", "fewer than six"),
        ];
        for (_, text) in R6602_SHAPES.iter() {
            for (from, to) in swaps {
                if text.contains(from) {
                    let near = text.replacen(from, to, 1);
                    assert_eq!(r6602_whole_read(&near), None, "{near}");
                }
            }
        }
    }
}
