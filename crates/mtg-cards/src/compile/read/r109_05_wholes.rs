//! r109-05 rules of combat damage, added combat phases and redirected attackers whose wordings no other reader or writer
//! joins: each entry of `r109_05_wholes.json` is an ability with the words the card prints for it (the card's name as
//! `~`), read when a clause says exactly those words and written back from the ability, so another recipient, assigner,
//! creature or duration never reads back.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R109_05_WHOLES: &str = include_str!("r109_05_wholes.json");

/// The entries parsed once.
static R109_05_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R109_05_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose wording is the clause.
pub(super) fn r109_05_whole_read(text: &str) -> Option<Ability> {
    R109_05_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r109_05_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R109_05_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, and a near miss of its
    //! words (another recipient, assigner, blocker or source count) reads as nothing.
    use super::{R109_05_SHAPES, R109_05_WHOLES, r109_05_add_wholes, r109_05_whole_read};

    #[test]
    fn r109_05_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R109_05_WHOLES).unwrap();
        assert_eq!(R109_05_SHAPES.len(), entries.len());
        for (ability, text) in R109_05_SHAPES.iter() {
            assert_eq!(r109_05_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r109_05_add_wholes(ability, &mut texts);
            assert!(texts.contains(text), "{text}");
        }
    }

    #[rstest::rstest]
    #[case("defending player and/or", "defending player or")]
    #[case("any number of creatures", "one creature")]
    #[case("creatures they control", "creatures you control")]
    #[case("is unblocked", "is blocked")]
    #[case("to a creature defending player controls", "to a creature you control")]
    #[case("a single source", "two sources")]
    #[case("unless lethal damage", "unless damage")]
    #[case("you assign", "they assign")]
    #[case("attacking you.", "attacking.")]
    #[case("blocking it.", "blocking you.")]
    #[case("Only land creatures", "Only creatures")]
    #[case("their opponents control", "they control")]
    #[case("any number of target", "one target")]
    #[case("now attacking that player", "now attacking you")]
    #[case("untap all lands", "untap all creatures")]
    #[case("during that combat phase", "this turn")]
    #[case("attacks that combat if able", "attacks each combat if able")]
    #[case("can't attack you or planeswalkers", "can't attack planeswalkers")]
    #[case("Remove all attacking creatures", "Remove all blocking creatures")]
    fn r109_05_a_near_miss_reads_as_nothing(#[case] from: &str, #[case] to: &str) {
        let mut hit = false;
        for (_, text) in R109_05_SHAPES.iter() {
            if text.contains(from) {
                hit = true;
                let near = text.replacen(from, to, 1);
                assert_eq!(r109_05_whole_read(&near), None, "{near}");
            }
        }
        assert!(hit, "{from}");
    }
}
