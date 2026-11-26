//! p160w abilities whose wordings no other reader or writer joins: each entry of `p160w_wholes.json` is an ability with
//! the words the card prints for it, read when a clause says exactly those words and written back from the ability.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const P160W_WHOLES: &str = include_str!("p160w_wholes.json");

/// The entries parsed once; one that does not parse is dropped (a test holds that none is).
static P160W_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(P160W_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Reads a clause that is the wording of one ability of the table.
pub(super) fn p160w_whole_read(text: &str) -> Option<Ability> {
    P160W_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(shape, _)| shape.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn p160w_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in P160W_SHAPES.iter() {
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
    fn p160w_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(P160W_WHOLES).unwrap();
        assert_eq!(P160W_SHAPES.len(), entries.len());
        for (shape, text) in P160W_SHAPES.iter() {
            assert_eq!(p160w_whole_read(text).as_ref(), Some(shape), "{text}");
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
    #[case("\"R6022Granter\"", "\"Controller\"")]
    #[case("\"most\":1", "\"most\":2")]
    #[case("\"R702185Warped\"", "\"Tapped\"")]
    #[case("\"BeforeAttackers\"", "\"AfterBlockers\"")]
    #[case("\"R1012ControlKept\"", "\"CantBlock\"")]
    #[case("\"R3034CantBeEnchanted\"", "\"CantAttack\"")]
    #[case("\"Untapped\"", "\"Tapped\"")]
    #[case("{\"Color\":\"Red\"}", "{\"Color\":\"Blue\"}")]
    fn p160w_a_changed_part_is_not_written_whole(#[case] from: &str, #[case] to: &str) {
        for (shape, text) in P160W_SHAPES.iter() {
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
    #[case("Only you may", "Any player may")]
    #[case("a warped creature card", "a creature card")]
    #[case("the chosen creature or card", "the chosen creature")]
    #[case("before attackers", "after attackers")]
    #[case("can't gain control", "can't gain life")]
    #[case("Black and/or red", "Black and/or green")]
    #[case("is untapped", "is tapped")]
    fn p160w_a_near_miss_reads_as_nothing(#[case] from: &str, #[case] to: &str) {
        for (_, text) in P160W_SHAPES.iter() {
            if text.contains(from) {
                let near = text.replacen(from, to, 1);
                assert_eq!(p160w_whole_read(&near), None, "{near}");
            }
        }
    }
}
