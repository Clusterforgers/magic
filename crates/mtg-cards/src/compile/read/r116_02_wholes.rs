//! r116-02 abilities of the Star Trek Commander set whose wordings no other reader or writer joins: each entry of
//! `r116_02_wholes.json` is an ability, or a list of two that one sentence holds, with the words the card prints for it
//! (the card's name as `~`), read when a clause says exactly those words and written back from the abilities.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs; a list of two abilities is one sentence that holds both.
const R116_02_WHOLES: &str = include_str!("r116_02_wholes.json");

/// The entries parsed once; one that does not parse is dropped (a test holds that none is).
static R116_02_SHAPES: LazyLock<Vec<(Vec<Ability>, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R116_02_WHOLES)
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
pub(super) fn r116_02_whole_read(text: &str) -> Option<Ability> {
    R116_02_SHAPES
        .iter()
        .find_map(|(shape, wording)| match shape.as_slice() {
            [one] if same_text(wording, text) => Some(one.clone()),
            _ => None,
        })
}

/// Whether the ability is the entry's, as the writers see it: an entry holds the logs of its investigations, which the
/// writers leave out (`l31a_unlogged`).
fn is_entry(one: &Ability, ability: &Ability) -> bool {
    one == ability || super::l31a_performed::l31a_unlogged(one).as_ref() == Some(ability)
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r116_02_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R116_02_SHAPES.iter() {
        if matches!(shape.as_slice(), [one] if is_entry(one, ability)) && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

/// Reads a sentence of the table that holds two abilities.
pub(super) fn r116_02_pair_read(text: &str) -> Option<Vec<Ability>> {
    R116_02_SHAPES
        .iter()
        .find(|(shape, wording)| shape.len() == 2 && same_text(wording, text))
        .map(|(shape, _)| shape.clone())
}

/// The wordings of the table for two abilities in one sentence, in that order.
pub(super) fn r116_02_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    R116_02_SHAPES
        .iter()
        .filter(|(shape, _)| matches!(shape.as_slice(), [a, b] if a == first && b == second))
        .map(|(_, wording)| wording.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, the same ability with
    //! any one number changed is not written as that wording, and a near miss of the words reads as nothing.
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::compile::read::render::written_variants;

    #[test]
    fn r116_02_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R116_02_WHOLES).unwrap();
        assert_eq!(R116_02_SHAPES.len(), entries.len());
        for (shape, text) in R116_02_SHAPES.iter() {
            match shape.as_slice() {
                [one] => {
                    assert_eq!(r116_02_whole_read(text).as_ref(), Some(one), "{text}");
                    assert!(written_variants(one).contains(text), "{text}");
                }
                [first, second] => {
                    assert_eq!(r116_02_pair_read(text).as_ref(), Some(shape), "{text}");
                    assert_eq!(
                        r116_02_pair_texts(first, second),
                        std::slice::from_ref(text)
                    );
                    assert!(r116_02_pair_texts(second, first).is_empty(), "{text}");
                }
                _ => panic!("an entry holds one or two abilities: {text}"),
            }
        }
    }

    #[test]
    fn r116_02_investigating_twice_logs_each_investigation() {
        let text = R116_02_SHAPES
            .iter()
            .map(|(_, text)| text)
            .find(|text| text.ends_with("Investigate twice."))
            .unwrap();
        let read = serde_json::to_string(&r116_02_whole_read(text).unwrap()).unwrap();
        assert_eq!(read.matches(r#""Performed""#).count(), 2, "{read}");
        // One investigation is not the words of two.
        let once = read.replacen(
            r#"{"CreateToken":{"token":{"Named":"clue"}}},{"Performed":{"action":"Investigated"}},"#,
            "",
            1,
        );
        let once: Ability = serde_json::from_str(&once).unwrap();
        assert!(!written_variants(&once).contains(text), "{once:?}");
    }

    #[test]
    fn r116_02_a_number_changed_is_not_written_whole() {
        for (shape, text) in R116_02_SHAPES.iter() {
            let [one] = shape.as_slice() else {
                continue;
            };
            let json = serde_json::to_string(one).unwrap();
            for (at, digit) in json.char_indices().filter(|(_, c)| c.is_ascii_digit()) {
                let raised = char::from_digit((digit.to_digit(10).unwrap() + 1) % 10, 10).unwrap();
                let changed = format!("{}{raised}{}", &json[..at], &json[at + 1..]);
                if let Ok(near) = serde_json::from_str::<Ability>(&changed) {
                    assert!(!written_variants(&near).contains(text), "{changed}");
                }
            }
        }
    }

    #[test]
    fn r116_02_a_near_miss_reads_as_nothing() {
        let swaps = [
            ("opponent", "player"),
            ("this turn", "next turn"),
            ("graveyard", "library"),
            ("three or more", "four or more"),
            ("six or more", "five or more"),
            ("seven or more", "eight or more"),
            ("tapped", "untapped"),
            ("plus one", "plus two"),
            ("land card", "creature card"),
        ];
        for (_, text) in R116_02_SHAPES.iter() {
            for (from, to) in swaps {
                if text.contains(from) {
                    let near = text.replacen(from, to, 1);
                    assert_eq!(r116_02_whole_read(&near), None, "{near}");
                    assert_eq!(r116_02_pair_read(&near), None, "{near}");
                }
            }
        }
    }
}
