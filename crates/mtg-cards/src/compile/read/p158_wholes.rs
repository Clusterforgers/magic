//! p158 activated and loyalty abilities whose wordings no other reader or writer joins: each entry of
//! `p158_wholes.json` is an ability with the words the card prints for it, read when a clause says exactly those words
//! and written back from the ability.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const P158_WHOLES: &str = include_str!("p158_wholes.json");

/// The entries parsed once; one that does not parse is dropped (a test holds that none is).
static P158_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(P158_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Reads a clause that is the wording of one ability of the table.
pub(super) fn p158_whole_read(text: &str) -> Option<Ability> {
    P158_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(shape, _)| shape.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn p158_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in P158_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, the same ability with
    //! one part changed is not written as that wording, and a near miss of the words reads as nothing.
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::compile::read::render::written_variants;

    #[test]
    fn p158_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(P158_WHOLES).unwrap();
        assert_eq!(P158_SHAPES.len(), entries.len());
        for (shape, text) in P158_SHAPES.iter() {
            assert_eq!(p158_whole_read(text).as_ref(), Some(shape), "{text}");
            assert!(
                written_variants(shape)
                    .iter()
                    .any(|written| super::super::tokens::same_written_text(written, text)),
                "{text} {:?}",
                written_variants(shape)
            );
        }
    }

    #[rstest::rstest]
    #[case("\"Skeleton\"", "\"Zombie\"")]
    #[case("\"LoseAbilities\"", "\"LoseAbilitiesExceptMana\"")]
    #[case("\"Eq\"", "\"Le\"")]
    #[case("\"ExiledWith\"", "\"NotedName\"")]
    #[case("\"Aura\"", "\"Equipment\"")]
    #[case("\"by\":2", "\"by\":3")]
    #[case("\"CreaturesHelpPay\":4", "\"CreaturesHelpPay\":3")]
    #[case("\"OptionsNotNoted\"", "\"Options\"")]
    #[case("\"Opponents\"", "\"EachPlayer\"")]
    fn p158_a_changed_part_is_not_written_whole(#[case] from: &str, #[case] to: &str) {
        for (shape, text) in P158_SHAPES.iter() {
            let json = serde_json::to_string(shape).unwrap();
            if !json.contains(from) {
                continue;
            }
            let changed = json.replacen(from, to, 1);
            if let Ok(near) = serde_json::from_str::<Ability>(&changed) {
                assert!(
                    !written_variants(&near)
                        .iter()
                        .any(|written| super::super::tokens::same_written_text(written, text)),
                    "{changed}"
                );
            }
        }
    }

    #[rstest::rstest]
    #[case("a Skeleton", "a Zombie")]
    #[case("has no abilities", "has flying")]
    #[case("a token that's a copy", "two tokens that are copies")]
    #[case("the mana value of", "the power of")]
    #[case("an untapped creature", "an untapped artifact")]
    #[case("an Aura spell", "a creature spell")]
    #[case("twice the number", "three times the number")]
    #[case("hasn't been chosen", "has been chosen")]
    #[case("an opponent of your choice", "a player of your choice")]
    fn p158_a_near_miss_reads_as_nothing(#[case] from: &str, #[case] to: &str) {
        for (_, text) in P158_SHAPES.iter() {
            if text.contains(from) {
                let near = text.replacen(from, to, 1);
                assert_eq!(p158_whole_read(&near), None, "{near}");
            }
        }
    }
}
