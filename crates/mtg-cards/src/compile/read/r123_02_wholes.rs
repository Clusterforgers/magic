//! r123-02 abilities whose wordings no other reader or writer joins: each entry of `r123_02_wholes.json` is an ability
//! with the words the card prints for it, read when a clause says exactly those words and written back from the ability.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R123_02_WHOLES: &str = include_str!("r123_02_wholes.json");

/// The entries parsed once; one that does not parse is dropped (a test holds that none is).
static R123_02_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R123_02_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Reads a clause that is the wording of one ability of the table.
pub(super) fn r123_02_whole_read(text: &str) -> Option<Ability> {
    R123_02_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(shape, _)| shape.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r123_02_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R123_02_SHAPES.iter() {
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
    fn r123_02_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R123_02_WHOLES).unwrap();
        assert_eq!(R123_02_SHAPES.len(), entries.len());
        for (shape, text) in R123_02_SHAPES.iter() {
            assert_eq!(r123_02_whole_read(text).as_ref(), Some(shape), "{text}");
            assert!(written_variants(shape).contains(text), "{text}");
        }
    }

    #[rstest::rstest]
    #[case("\"Le\"", "\"Ge\"")]
    #[case("\"DifferentControllers\"", "\"DifferentNames\"")]
    #[case("\"YourTurn\"", "\"ExtraTurn\"")]
    #[case("\"Asker\"", "\"You\"")]
    fn r123_02_a_changed_part_is_not_written_whole(#[case] from: &str, #[case] to: &str) {
        for (shape, text) in R123_02_SHAPES.iter() {
            let json = serde_json::to_string(shape).unwrap();
            if !json.contains(from) {
                continue;
            }
            let changed = json.replacen(from, to, 1);
            if let Ok(near) = serde_json::from_str::<Ability>(&changed) {
                assert!(!written_variants(&near).contains(text), "{changed}");
            }
        }
    }

    #[rstest::rstest]
    #[case("power 2 or less", "power 3 or less")]
    #[case("during their turn", "during your turn")]
    #[case("attacked you", "blocked you")]
    fn r123_02_a_near_miss_reads_as_nothing(#[case] from: &str, #[case] to: &str) {
        for (_, text) in R123_02_SHAPES.iter() {
            if text.contains(from) {
                let near = text.replacen(from, to, 1);
                assert_eq!(r123_02_whole_read(&near), None, "{near}");
            }
        }
    }
}
