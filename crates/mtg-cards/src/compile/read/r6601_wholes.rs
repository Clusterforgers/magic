//! r66-01 abilities whose wordings no other reader or writer joins: each entry of `r6601_wholes.json` is an ability with
//! the words the card prints for it (the card's name as `~`), read when a clause says exactly those words and written
//! back from the ability, so another count, step or event in the same ability never reads back.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R6601_WHOLES: &str = include_str!("r6601_wholes.json");

/// The entries parsed once.
static R6601_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6601_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose wording is the clause.
pub(super) fn r6601_whole_read(text: &str) -> Option<Ability> {
    R6601_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r6601_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R6601_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, and a near miss of its
    //! words (a number, a player or a duration changed) reads as nothing.
    use super::{R6601_SHAPES, R6601_WHOLES, r6601_add_wholes, r6601_whole_read};

    #[test]
    fn r6601_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6601_WHOLES).unwrap();
        assert_eq!(R6601_SHAPES.len(), entries.len());
        for (ability, text) in R6601_SHAPES.iter() {
            assert_eq!(r6601_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r6601_add_wholes(ability, &mut texts);
            assert!(texts.contains(text), "{text}");
        }
    }

    #[test]
    fn r6601_a_near_miss_reads_as_nothing() {
        let swaps = [
            ("Each opponent may", "Each player may"),
            ("a land card", "a creature card"),
            ("Then each player who searched", "Then you"),
            ("target opponent", "target player"),
            ("rounded up", "rounded down"),
            ("greatest mana value", "least mana value"),
            ("in exile", "in your graveyard"),
            ("annihilator X", "toxic X"),
            ("+1/+1 counters", "-1/-1 counters"),
            ("your end step", "each end step"),
            ("greatest toughness", "greatest power"),
            ("each opponent and", "each player and"),
            ("blight X", "blight 2"),
            ("up to one target", "target"),
            ("or planeswalkers you control", ""),
            ("one or more counters", "a +1/+1 counter"),
        ];
        for (_, text) in R6601_SHAPES.iter() {
            for (from, to) in swaps {
                if text.contains(from) {
                    let near = text.replacen(from, to, 1);
                    assert_eq!(r6601_whole_read(&near), None, "{near}");
                }
            }
        }
    }
}
