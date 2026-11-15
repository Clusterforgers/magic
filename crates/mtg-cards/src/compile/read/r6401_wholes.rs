//! r64-01 abilities whose wordings no other reader or writer joins: each entry of `r6401_wholes.json` is an ability with
//! the words the card prints for it (the card's name as `~`), read when a clause says exactly those words and written
//! back from the ability, so another count, step or event in the same ability never reads back.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R6401_WHOLES: &str = include_str!("r6401_wholes.json");

/// The entries parsed once.
static R6401_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6401_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose wording is the clause.
pub(super) fn r6401_whole_read(text: &str) -> Option<Ability> {
    R6401_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r6401_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R6401_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, and a near miss of its
    //! words (a number, a player or a duration changed) reads as nothing.
    use super::{R6401_SHAPES, R6401_WHOLES, r6401_add_wholes, r6401_whole_read};

    #[test]
    fn r6401_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6401_WHOLES).unwrap();
        assert_eq!(R6401_SHAPES.len(), entries.len());
        for (ability, text) in R6401_SHAPES.iter() {
            assert_eq!(r6401_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r6401_add_wholes(ability, &mut texts);
            assert!(texts.contains(text), "{text}");
        }
    }

    #[test]
    fn r6401_a_near_miss_reads_as_nothing() {
        let swaps = [
            ("an opponent", "a player"),
            ("left or right", "left"),
            ("the next player", "the player"),
            ("to their right", "to their left"),
            ("nonland permanent", "permanent"),
            ("three or more", "two or more"),
            ("rolls a 1", "rolls a 2"),
            ("until end of turn, or", "until"),
            ("a Plains", "a Forest"),
            ("2 damage", "3 damage"),
            ("tapped a land", "tapped a creature"),
            ("shield counter", "+1/+1 counter"),
            ("equipped creature", "enchanted creature"),
            ("lose the flip", "win the flip"),
            ("another player", "an opponent"),
            ("Otherwise", "Then"),
            ("each player's upkeep", "your upkeep"),
            ("you own", "you control"),
        ];
        for (_, text) in R6401_SHAPES.iter() {
            for (from, to) in swaps {
                if text.contains(from) {
                    let near = text.replacen(from, to, 1);
                    assert_eq!(r6401_whole_read(&near), None, "{near}");
                }
            }
        }
    }
}
