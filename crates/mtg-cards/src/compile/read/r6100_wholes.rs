//! r61-00 abilities whose wordings no other reader or writer joins: each entry of `r6100_wholes.json` is an ability with
//! the words the card prints for it (the card's name as `~`), read when a clause says exactly those words and written
//! back from the ability, so another count, step or event in the same ability never reads back.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R6100_WHOLES: &str = include_str!("r6100_wholes.json");

/// The entries parsed once.
static R6100_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6100_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose wording is the clause.
pub(super) fn r6100_whole_read(text: &str) -> Option<Ability> {
    R6100_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r6100_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R6100_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, and a near miss of its
    //! words (a number, a player or a duration changed) reads as nothing.
    use super::{R6100_SHAPES, R6100_WHOLES, r6100_add_wholes, r6100_whole_read};

    #[test]
    fn r6100_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6100_WHOLES).unwrap();
        assert_eq!(R6100_SHAPES.len(), entries.len());
        for (ability, text) in R6100_SHAPES.iter() {
            assert_eq!(r6100_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r6100_add_wholes(ability, &mut texts);
            assert!(texts.contains(text), "{text}");
        }
    }

    #[test]
    fn r6100_a_near_miss_reads_as_nothing() {
        let swaps = [
            ("creature card", "card"),
            ("your graveyard", "a graveyard"),
            ("another Phoenix", "a Phoenix"),
            ("each end step", "your end step"),
            ("same mana value", "same power"),
            ("you don't control", "you control"),
            ("1/1", "2/2"),
            ("black", "red"),
            ("other Rat", "Rat"),
            ("your opponents'", "your"),
            ("this turn", "last turn"),
            ("any target", "target creature"),
            ("noncombat", "combat"),
            ("less than", "greater than"),
            ("red source", "green source"),
            ("tapped and transformed", "transformed"),
            ("4 or more", "5 or more"),
            ("three or more", "two or more"),
            ("bore counter", "oil counter"),
            ("plus one", "plus two"),
            ("planeswalker", "creature"),
            ("your upkeep", "each upkeep"),
        ];
        for (_, text) in R6100_SHAPES.iter() {
            for (from, to) in swaps {
                if text.contains(from) {
                    let near = text.replacen(from, to, 1);
                    assert_eq!(r6100_whole_read(&near), None, "{near}");
                }
            }
        }
    }
}
