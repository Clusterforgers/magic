//! r117-03 abilities whose wordings no other reader or writer joins (Urgent Necropsy):
//! each entry of `r117_03_wholes.json` is an ability, or a list of two that one clause holds, with the words the card
//! prints for it (the card's name as `~`), read when a clause says exactly those words and written back from the
//! abilities.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs; a list of two abilities is one clause that holds both.
const R117_03_WHOLES: &str = include_str!("r117_03_wholes.json");

/// The entries parsed once; one that does not parse is dropped (a test holds that none is).
static R117_03_SHAPES: LazyLock<Vec<(Vec<Ability>, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R117_03_WHOLES)
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
pub(super) fn r117_03_whole_read(text: &str) -> Option<Ability> {
    R117_03_SHAPES
        .iter()
        .find_map(|(shape, wording)| match shape.as_slice() {
            [one] if same_text(wording, text) => Some(one.clone()),
            _ => None,
        })
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r117_03_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R117_03_SHAPES.iter() {
        if matches!(shape.as_slice(), [one] if one == ability) && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

/// Reads a clause of the table that holds two abilities.
pub(super) fn r117_03_pair_read(text: &str) -> Option<Vec<Ability>> {
    R117_03_SHAPES
        .iter()
        .find(|(shape, wording)| shape.len() == 2 && same_text(wording, text))
        .map(|(shape, _)| shape.clone())
}

/// The wordings of the table for two abilities in one clause, in that order.
pub(super) fn r117_03_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    R117_03_SHAPES
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
    fn r117_03_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R117_03_WHOLES).unwrap();
        assert_eq!(R117_03_SHAPES.len(), entries.len());
        for (shape, text) in R117_03_SHAPES.iter() {
            match shape.as_slice() {
                [one] => {
                    assert_eq!(r117_03_whole_read(text).as_ref(), Some(one), "{text}");
                    assert!(written_variants(one).contains(text), "{text}");
                }
                [first, second] => {
                    assert_eq!(r117_03_pair_read(text).as_ref(), Some(shape), "{text}");
                    assert_eq!(
                        r117_03_pair_texts(first, second),
                        vec![text.clone()],
                        "{text}"
                    );
                    assert!(r117_03_pair_texts(second, first).is_empty(), "{text}");
                }
                _ => panic!("an entry holds one or two abilities: {text}"),
            }
        }
    }

    #[test]
    fn r117_03_a_number_changed_is_not_written_whole() {
        for (shape, text) in R117_03_SHAPES.iter() {
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

    #[rstest::rstest]
    #[case("collect evidence X", "collect evidence 6")]
    #[case("chosen at random", "of your choice")]
    #[case("up to one target planeswalker", "up to one target battle")]
    #[case("and/or {1}{G} any number of times", "and/or {1}{G}")]
    #[case("you gain 1 life", "you gain 2 life")]
    #[case(
        "basic land type among lands you control",
        "basic land type among lands an opponent controls"
    )]
    #[case("has landwalk of that type", "has protection from that type")]
    #[case("if they share a color", "if they share a creature type")]
    #[case("by another creature", "by a creature")]
    #[case("total mana value", "total power")]
    #[case("this spell targets", "you control")]
    #[case("you've surveilled this turn", "you've milled this turn")]
    #[case("play lands and cast spells", "cast spells")]
    #[case("pay life equal to its mana value", "pay 2 life")]
    fn r117_03_a_near_miss_reads_as_nothing(#[case] from: &str, #[case] to: &str) {
        for (_, text) in R117_03_SHAPES.iter() {
            if text.contains(from) {
                let near = text.replacen(from, to, 1);
                assert_eq!(r117_03_whole_read(&near), None, "{near}");
                assert_eq!(r117_03_pair_read(&near), None, "{near}");
            }
        }
    }
}
