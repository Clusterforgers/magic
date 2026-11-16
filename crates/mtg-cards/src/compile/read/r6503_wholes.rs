//! r65-03 abilities whose wordings no other reader or writer joins: each entry of `r6503_wholes.json` is an ability with
//! the words the card prints for it (the card's name as `~`), read when a clause says exactly those words and written
//! back from the ability, so another count, step or event in the same ability never reads back.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R6503_WHOLES: &str = include_str!("r6503_wholes.json");

/// The entries parsed once.
static R6503_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6503_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose wording is the clause.
pub(super) fn r6503_whole_read(text: &str) -> Option<Ability> {
    R6503_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r6503_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R6503_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, and a near miss of its
    //! words (a counter, a player, a number or a zone changed) reads as nothing.
    use super::{R6503_SHAPES, R6503_WHOLES, r6503_add_wholes, r6503_whole_read};

    #[test]
    fn r6503_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6503_WHOLES).unwrap();
        assert_eq!(R6503_SHAPES.len(), entries.len());
        for (ability, text) in R6503_SHAPES.iter() {
            assert_eq!(r6503_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r6503_add_wholes(ability, &mut texts);
            assert!(texts.contains(text), "{text}");
        }
    }

    #[rstest::rstest]
    #[case("experience counters", "energy counters")]
    #[case("you have", "an opponent has")]
    #[case("ward {X}", "ward {2}")]
    #[case("your hand", "your graveyard")]
    #[case("instant or sorcery", "instant")]
    #[case("a dream counter", "a time counter")]
    #[case("each opponent", "target opponent")]
    #[case("twenty-five", "twenty")]
    #[case("rounded up", "rounded down")]
    #[case("in exile", "in your graveyard")]
    #[case("+1/+1 counters", "-1/-1 counters")]
    fn r6503_a_near_miss_reads_as_nothing(#[case] from: &str, #[case] to: &str) {
        for (_, text) in R6503_SHAPES.iter() {
            if text.contains(from) {
                let near = text.replacen(from, to, 1);
                assert_eq!(r6503_whole_read(&near), None, "{near}");
            }
        }
    }
}
