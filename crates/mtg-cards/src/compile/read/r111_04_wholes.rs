//! r111-04 abilities whose wordings no other reader or writer joins (abilities of other permanents as they are now,
//! keywords given to cards outside the battlefield): each entry of `r111_04_wholes.json` is an ability with the words
//! the card prints for it (the card's name as `~`), read when a clause says exactly those words and written back from
//! the ability.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R111_04_WHOLES: &str = include_str!("r111_04_wholes.json");

/// The entries parsed once.
static R111_04_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R111_04_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose wording is the clause.
pub(super) fn r111_04_whole_read(text: &str) -> Option<Ability> {
    R111_04_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r111_04_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R111_04_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, and a near miss of its
    //! words (a kind, a step, a number or a recipient changed) reads as nothing.
    use super::{R111_04_SHAPES, R111_04_WHOLES, r111_04_add_wholes, r111_04_whole_read};

    #[test]
    fn r111_04_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R111_04_WHOLES).unwrap();
        assert_eq!(R111_04_SHAPES.len(), entries.len());
        for (ability, text) in R111_04_SHAPES.iter() {
            assert_eq!(r111_04_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r111_04_add_wholes(ability, &mut texts);
            assert!(texts.contains(text), "{text}");
        }
    }

    #[test]
    fn r111_04_a_near_miss_reads_as_nothing() {
        let swaps = [
            (
                "all lands on the battlefield",
                "all creatures on the battlefield",
            ),
            ("lands your opponents control", "lands you control"),
            (" except mana abilities", ""),
            ("nonland cards", "creature cards"),
            ("has plot", "has flash"),
            ("equal to its mana cost", "equal to {2}"),
            ("of target creature", "of target artifact"),
            ("until end of turn", "until your next turn"),
            ("red creature cards", "green creature cards"),
            ("in your hand have warp", "in your graveyard have warp"),
            ("sneak {3}{B}", "sneak {2}{B}"),
            ("their sneak abilities", "their warp abilities"),
            ("mana value 4 or greater", "mana value 3 or greater"),
            ("pay cost {1} less", "pay cost {2} less"),
            ("three time counters", "two time counters"),
            (
                "your next end step, sacrifice those",
                "the next end step, sacrifice those",
            ),
            ("during an opponent's turn", "during your turn"),
        ];
        for (_, text) in R111_04_SHAPES.iter() {
            for (from, to) in swaps {
                if text.contains(from) {
                    let near = text.replacen(from, to, 1);
                    assert_eq!(r111_04_whole_read(&near), None, "{near}");
                }
            }
        }
    }
}
