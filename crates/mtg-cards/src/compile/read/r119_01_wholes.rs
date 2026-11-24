//! r119-01 abilities whose wordings no other reader or writer joins (Itazura, Lingering Wick): each entry of
//! `r119_01_wholes.json` is an ability with the words the card prints for it (the card's name as `~`), read when a
//! clause says exactly those words and written back from the ability.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R119_01_WHOLES: &str = include_str!("r119_01_wholes.json");

/// The entries parsed once; one that does not parse is dropped (a test holds that none is).
static R119_01_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R119_01_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Reads a clause that is the wording of one ability of the table.
pub(super) fn r119_01_whole_read(text: &str) -> Option<Ability> {
    R119_01_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(shape, _)| shape.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r119_01_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R119_01_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, the same ability with
    //! any one number changed is not written as that wording, and a near miss of the words reads as nothing.
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::compile::read::render::written_variants;

    #[test]
    fn r119_01_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R119_01_WHOLES).unwrap();
        assert_eq!(R119_01_SHAPES.len(), entries.len());
        for (shape, text) in R119_01_SHAPES.iter() {
            // Chapters that print one text share it: the text reads as the first of them.
            let read = r119_01_whole_read(text);
            assert!(
                R119_01_SHAPES
                    .iter()
                    .any(|(other, wording)| wording == text && read.as_ref() == Some(other)),
                "{text}"
            );
            assert!(written_variants(shape).contains(text), "{text}");
        }
    }

    #[test]
    fn r119_01_a_number_changed_is_not_written_whole() {
        for (shape, text) in R119_01_SHAPES.iter() {
            let json = serde_json::to_string(shape).unwrap();
            for (at, digit) in json.char_indices().filter(|(_, c)| c.is_ascii_digit()) {
                let raised = char::from_digit((digit.to_digit(10).unwrap() + 1) % 10, 10).unwrap();
                let changed = format!("{}{raised}{}", &json[..at], &json[at + 1..]);
                // Another chapter that prints the same text is an entry of its own.
                if let Ok(near) = serde_json::from_str::<Ability>(&changed)
                    && !R119_01_SHAPES.iter().any(|(other, _)| *other == near)
                {
                    assert!(!written_variants(&near).contains(text), "{changed}");
                }
            }
        }
    }

    #[rstest::rstest]
    #[case("the top three cards", "the top two cards")]
    #[case("Each opponent secretly", "Each player secretly")]
    #[case("the highest number", "the lowest number")]
    #[case("without paying its mana cost", "by paying its mana cost")]
    #[case("into your hand", "into their hand")]
    fn r119_01_a_near_miss_reads_as_nothing(#[case] from: &str, #[case] to: &str) {
        for (_, text) in R119_01_SHAPES.iter() {
            if text.contains(from) {
                let near = text.replacen(from, to, 1);
                assert_eq!(r119_01_whole_read(&near), None, "{near}");
            }
        }
    }
}
