//! cx8 abilities whose wordings no other reader or writer joins: each entry of `cx8_wholes.json` is an ability with
//! the words the card prints for it, read when a clause says exactly those words and written back from the ability.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const CX8_WHOLES: &str = include_str!("cx8_wholes.json");

/// The entries parsed once; one that does not parse is dropped (a test holds that none is).
static CX8_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(CX8_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Reads a clause that is the wording of one ability of the table.
pub(super) fn cx8_whole_read(text: &str) -> Option<Ability> {
    CX8_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(shape, _)| shape.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn cx8_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in CX8_SHAPES.iter() {
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
    fn cx8_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(CX8_WHOLES).unwrap();
        assert_eq!(CX8_SHAPES.len(), entries.len());
        for (shape, text) in CX8_SHAPES.iter() {
            assert_eq!(cx8_whole_read(text).as_ref(), Some(shape), "{text}");
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
    #[case("\"Opponents\"", "\"EachPlayer\"")]
    #[case("\"free\":true", "\"free\":false")]
    #[case("\"count\":2", "\"count\":3")]
    #[case("\"Forever\"", "\"UntilEndOfTurn\"")]
    fn cx8_a_changed_part_is_not_written_whole(#[case] from: &str, #[case] to: &str) {
        for (shape, text) in CX8_SHAPES.iter() {
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
    #[case("different opponent", "same opponent")]
    #[case("behold two", "behold three")]
    #[case("mana value 1 ", "mana value 2 ")]
    #[case("two creature cards", "three creature cards")]
    #[case("a nonland card", "a land card")]
    fn cx8_a_near_miss_reads_as_nothing(#[case] from: &str, #[case] to: &str) {
        for (_, text) in CX8_SHAPES.iter() {
            if text.contains(from) {
                let near = text.replacen(from, to, 1);
                assert_eq!(cx8_whole_read(&near), None, "{near}");
            }
        }
    }
}
