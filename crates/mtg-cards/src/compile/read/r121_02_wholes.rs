//! r121-02 abilities whose wordings no other reader or writer joins (Psychic Paper, Dinosaur Headdress):
//! each entry of `r121_02_wholes.json` is an ability, or a list of two that one clause holds, with the words the card
//! prints for it, read when a clause says exactly those words and written back from the abilities.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs; a list of two abilities is one clause that holds both.
const R121_02_WHOLES: &str = include_str!("r121_02_wholes.json");

/// The entries parsed once; one that does not parse is dropped (a test holds that none is).
static R121_02_SHAPES: LazyLock<Vec<(Vec<Ability>, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R121_02_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| {
            let shape = match shape {
                serde_json::Value::Array(list) => list
                    .into_iter()
                    .map(serde_json::from_value)
                    .collect::<Result<Vec<Ability>, _>>()
                    .ok()?,
                one => vec![serde_json::from_value(one).ok()?],
            };
            Some((shape, text))
        })
        .collect()
});

/// Reads a clause that is the wording of one ability of the table.
/// An ability word before a dash is flavor and left out ("Vivid — ").
pub(super) fn r121_02_whole_read(text: &str) -> Option<Ability> {
    let head = text.lines().next().unwrap_or_default();
    let text = text
        .get(crate::compile::classify::ability_word_len(head).unwrap_or(0)..)
        .unwrap_or(text);
    R121_02_SHAPES
        .iter()
        .find_map(|(shape, wording)| match shape.as_slice() {
            [one] if same_text(wording, text) => Some(one.clone()),
            _ => None,
        })
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r121_02_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R121_02_SHAPES.iter() {
        if matches!(shape.as_slice(), [one] if one == ability) && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

/// Reads a clause of the table that holds two abilities.
pub(super) fn r121_02_pair_read(text: &str) -> Option<Vec<Ability>> {
    R121_02_SHAPES
        .iter()
        .find(|(shape, wording)| shape.len() == 2 && same_text(wording, text))
        .map(|(shape, _)| shape.clone())
}

/// The wordings of the table for two abilities in one clause, in that order.
pub(super) fn r121_02_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    R121_02_SHAPES
        .iter()
        .filter(|(shape, _)| matches!(shape.as_slice(), [a, b] if a == first && b == second))
        .map(|(_, wording)| wording.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, the same ability with
    //! any one number or mana type changed is not written as that wording, and a near miss of the words reads as nothing.
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::compile::read::render::written_variants;

    #[test]
    fn r121_02_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R121_02_WHOLES).unwrap();
        assert_eq!(R121_02_SHAPES.len(), entries.len());
        for (shape, text) in R121_02_SHAPES.iter() {
            match shape.as_slice() {
                [one] => {
                    assert_eq!(r121_02_whole_read(text).as_ref(), Some(one), "{text}");
                    assert!(written_variants(one).contains(text), "{text}");
                }
                [first, second] => {
                    assert_eq!(r121_02_pair_read(text).as_ref(), Some(shape), "{text}");
                    assert_eq!(
                        r121_02_pair_texts(first, second),
                        vec![text.clone()],
                        "{text}"
                    );
                    assert!(r121_02_pair_texts(second, first).is_empty(), "{text}");
                }
                _ => panic!("an entry holds one or two abilities: {text}"),
            }
        }
    }

    #[rstest::rstest]
    #[case("\"R121CreatureNameAndType\"", "\"CreatureType\"")]
    #[case("\"R121SetChosenName\"", "\"SetChosenColor\"")]
    #[case("\"{1}\"", "\"{2}\"")]
    #[case("\"Exile\"", "\"Graveyard\"")]
    #[case("\"R6702CraftedWith\"", "\"Attached\"")]
    fn r121_02_a_changed_part_is_not_written_whole(#[case] from: &str, #[case] to: &str) {
        for (shape, text) in R121_02_SHAPES.iter() {
            let [one] = shape.as_slice() else {
                continue;
            };
            let json = serde_json::to_string(one).unwrap();
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
    #[case("creature card name", "card name")]
    #[case("ward {1}", "ward {2}")]
    #[case("exiled creature card", "exiled card")]
    #[case("last chosen card", "first chosen card")]
    fn r121_02_a_near_miss_reads_as_nothing(#[case] from: &str, #[case] to: &str) {
        for (_, text) in R121_02_SHAPES.iter() {
            if text.contains(from) {
                let near = text.replacen(from, to, 1);
                assert_eq!(r121_02_whole_read(&near), None, "{near}");
                assert_eq!(r121_02_pair_read(&near), None, "{near}");
            }
        }
    }
}
