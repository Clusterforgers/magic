//! r110-01 abilities whose wordings no other reader or writer joins (foretell given to cards in a hand, foretelling costs
//! and timing, mana spent as though it were another color): each entry of `r110_01_wholes.json` is an ability, or a list
//! of two that one sentence holds, with the words the card prints for it (the card's name as `~`), read when a clause
//! says exactly those words and written back from the abilities.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs; a list of two abilities is one sentence that holds both.
const R110_01_WHOLES: &str = include_str!("r110_01_wholes.json");

/// The entries parsed once; one that does not parse is dropped (a test holds that none is).
static R110_01_SHAPES: LazyLock<Vec<(Vec<Ability>, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R110_01_WHOLES)
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
pub(super) fn r110_01_whole_read(text: &str) -> Option<Ability> {
    R110_01_SHAPES
        .iter()
        .find_map(|(shape, wording)| match shape.as_slice() {
            [one] if same_text(wording, text) => Some(one.clone()),
            _ => None,
        })
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r110_01_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R110_01_SHAPES.iter() {
        if matches!(shape.as_slice(), [one] if one == ability) && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

/// Reads a sentence of the table that holds two abilities.
pub(super) fn r110_01_pair_read(text: &str) -> Option<Vec<Ability>> {
    R110_01_SHAPES
        .iter()
        .find(|(shape, wording)| shape.len() == 2 && same_text(wording, text))
        .map(|(shape, _)| shape.clone())
}

/// The wordings of the table for two abilities in one sentence, in that order.
pub(super) fn r110_01_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    R110_01_SHAPES
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
    fn r110_01_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R110_01_WHOLES).unwrap();
        assert_eq!(R110_01_SHAPES.len(), entries.len());
        for (shape, text) in R110_01_SHAPES.iter() {
            match shape.as_slice() {
                [one] => {
                    assert_eq!(r110_01_whole_read(text).as_ref(), Some(one), "{text}");
                    assert!(written_variants(one).contains(text), "{text}");
                }
                [first, second] => {
                    assert_eq!(r110_01_pair_read(text).as_ref(), Some(shape), "{text}");
                    assert_eq!(
                        r110_01_pair_texts(first, second),
                        std::slice::from_ref(text)
                    );
                    assert!(r110_01_pair_texts(second, first).is_empty(), "{text}");
                }
                _ => panic!("an entry holds one or two abilities: {text}"),
            }
        }
    }

    #[test]
    fn r110_01_a_number_changed_is_not_written_whole() {
        for (shape, text) in R110_01_SHAPES.iter() {
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
    fn r110_01_a_near_miss_reads_as_nothing() {
        let swaps = [
            ("without foretell", "with foretell"),
            ("nonland card", "card"),
            ("reduced by {2}", "reduced by {3}"),
            ("in your hand", "in your graveyard"),
            ("costs {1} less", "costs {1} more"),
            ("any player's turn", "your turn"),
            ("first card", "second card"),
            ("costs {0}", "costs {1}"),
            ("white mana", "blue mana"),
            ("red mana", "green mana"),
            ("you foretell", "an opponent foretells"),
            ("+2/+0", "+0/+2"),
            ("legendary spell", "spell"),
            ("black mana symbol", "white mana symbol"),
            ("a Swamp", "a Plains"),
            ("with flying and", "with haste and"),
            ("card's mana value", "card's power"),
            ("Spells cost", "Creature spells cost"),
            ("one or more colors", "two or more colors"),
            ("{G}{W}{U}", "{G}{W}"),
        ];
        for (_, text) in R110_01_SHAPES.iter() {
            for (from, to) in swaps {
                if text.contains(from) {
                    let near = text.replacen(from, to, 1);
                    assert_eq!(r110_01_whole_read(&near), None, "{near}");
                    assert_eq!(r110_01_pair_read(&near), None, "{near}");
                }
            }
        }
    }
}
