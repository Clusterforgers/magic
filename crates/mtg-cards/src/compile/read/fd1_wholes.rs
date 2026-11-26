//! fd1 abilities (face-down permanents an effect sets up) whose wordings no other reader or writer joins: each entry of `fd1_wholes.json` is an ability with
//! the words the card prints for it, read when a clause says exactly those words and written back from the ability.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const FD1_WHOLES: &str = include_str!("fd1_wholes.json");

/// The entries parsed once; one that does not parse is dropped (a test holds that none is).
static FD1_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(FD1_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Reads a clause that is the wording of one ability of the table.
pub(super) fn fd1_whole_read(text: &str) -> Option<Ability> {
    FD1_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(shape, _)| shape.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn fd1_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in FD1_SHAPES.iter() {
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
    fn fd1_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(FD1_WHOLES).unwrap();
        assert_eq!(FD1_SHAPES.len(), entries.len());
        for (shape, text) in FD1_SHAPES.iter() {
            assert_eq!(fd1_whole_read(text).as_ref(), Some(shape), "{text}");
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
    #[case("\"power\":3", "\"power\":2")]
    #[case("\"note_name\":true", "\"note_name\":false")]
    #[case("\"MoveTo\":\"Exile\"", "\"MoveTo\":\"Hand\"")]
    #[case("\"R6008FaceDown\"", "\"Madness\"")]
    #[case("\"timing\":\"Sorcery\"", "\"timing\":\"Instant\"")]
    #[case("{\"In\":\"Hand\"}", "{\"In\":\"Graveyard\"}")]
    fn fd1_a_changed_part_is_not_written_whole(#[case] from: &str, #[case] to: &str) {
        let mut hit = false;
        for (shape, text) in FD1_SHAPES.iter() {
            let json = serde_json::to_string(shape).unwrap();
            if !json.contains(from) {
                continue;
            }
            hit = true;
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
        assert!(hit, "no entry has {from}");
    }

    #[rstest::rstest]
    #[case("It's a 3/3 creature", "It's a 2/2 creature")]
    #[case("with the noted name", "with the chosen name")]
    #[case("exile it instead", "return it to its owner's hand instead")]
    #[case("as a 2/2 creature spell", "as a 3/3 creature spell")]
    #[case("creature card in your hand", "creature card in your graveyard")]
    #[case("Activate only as a sorcery.", "Activate only once each turn.")]
    fn fd1_a_near_miss_reads_as_nothing(#[case] from: &str, #[case] to: &str) {
        assert!(FD1_SHAPES.iter().any(|(_, text)| text.contains(from)));
        for (_, text) in FD1_SHAPES.iter() {
            if text.contains(from) {
                let near = text.replacen(from, to, 1);
                assert_eq!(fd1_whole_read(&near), None, "{near}");
            }
        }
    }
}
