//! p164 abilities whose wordings no other reader or writer joins: each entry of `p164_wholes.json` is an ability with
//! the words the card prints for it, read when a clause says exactly those words and written back from the ability.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const P164_WHOLES: &str = include_str!("p164_wholes.json");

/// The entries parsed once; one that does not parse is dropped (a test holds that none is).
static P164_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(P164_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Reads a clause that is the wording of one ability of the table.
pub(super) fn p164_whole_read(text: &str) -> Option<Ability> {
    P164_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(shape, _)| shape.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn p164_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in P164_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, and a near miss of the
    //! words reads as nothing.
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::compile::read::render::written_variants;

    #[test]
    fn p164_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(P164_WHOLES).unwrap();
        assert_eq!(P164_SHAPES.len(), entries.len());
        for (shape, text) in P164_SHAPES.iter() {
            assert_eq!(p164_whole_read(text).as_ref(), Some(shape), "{text}");
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
    #[case("any amount of mana", "any amount of life")]
    #[case("creature spells", "artifact spells")]
    #[case("less than or equal to", "greater than or equal to")]
    #[case("destroy a land", "destroy a creature")]
    fn p164_a_near_miss_reads_as_nothing(#[case] from: &str, #[case] to: &str) {
        for (_, text) in P164_SHAPES.iter() {
            if text.contains(from) {
                let near = text.replacen(from, to, 1);
                assert_eq!(p164_whole_read(&near), None, "{near}");
            }
        }
    }
}
