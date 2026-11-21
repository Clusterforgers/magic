//! r114-04 abilities whose wordings no other reader or writer joins (the abilities of ability stickers on other
//! objects): each entry
//! of `r114_04_wholes.json` is an ability, or a list of two that one sentence holds, with the words the card prints for
//! it (the card's name as `~`), read when a clause says exactly those words and written back from the abilities.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;
use crate::refs::ZoneKind;
use crate::trigger::TriggerPattern;

/// The entries, as JSON: a list of `[ability, wording]` pairs; a list of two abilities is one sentence that holds both.
const R114_04_WHOLES: &str = include_str!("r114_04_wholes.json");

/// The entries parsed once; one that does not parse is dropped (a test holds that none is).
static R114_04_SHAPES: LazyLock<Vec<(Vec<Ability>, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R114_04_WHOLES)
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
pub(super) fn r114_04_whole_read(text: &str) -> Option<Ability> {
    R114_04_SHAPES
        .iter()
        .find_map(|(shape, wording)| match shape.as_slice() {
            [one] if same_text(wording, text) => Some(one.clone()),
            _ => None,
        })
}

/// Adds the wording of an ability that one of the entries matches whole, a Saga chapter (CR 714.2) as the sentence of
/// its spell.
pub(super) fn r114_04_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    let spell = match ability {
        Ability::Triggered {
            when: TriggerPattern::Chapter { .. },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Battlefield,
            targets,
            effects,
        } => Some(Ability::Spell {
            targets: targets.clone(),
            effects: effects.clone(),
        }),
        _ => None,
    };
    for (shape, text) in R114_04_SHAPES.iter() {
        if matches!(shape.as_slice(), [one] if one == ability || spell.as_ref() == Some(one))
            && !texts.contains(text)
        {
            texts.push(text.clone());
        }
    }
}

/// Reads a sentence of the table that holds two abilities.
pub(super) fn r114_04_pair_read(text: &str) -> Option<Vec<Ability>> {
    R114_04_SHAPES
        .iter()
        .find(|(shape, wording)| shape.len() == 2 && same_text(wording, text))
        .map(|(shape, _)| shape.clone())
}

/// The wordings of the table for two abilities in one sentence, in that order.
pub(super) fn r114_04_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    R114_04_SHAPES
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
    fn r114_04_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R114_04_WHOLES).unwrap();
        assert_eq!(R114_04_SHAPES.len(), entries.len());
        for (shape, text) in R114_04_SHAPES.iter() {
            match shape.as_slice() {
                [one] => {
                    assert_eq!(r114_04_whole_read(text).as_ref(), Some(one), "{text}");
                    assert!(written_variants(one).contains(text), "{text}");
                }
                [first, second] => {
                    assert_eq!(r114_04_pair_read(text).as_ref(), Some(shape), "{text}");
                    assert_eq!(
                        r114_04_pair_texts(first, second),
                        vec![text.clone()],
                        "{text}"
                    );
                    assert!(r114_04_pair_texts(second, first).is_empty(), "{text}");
                }
                _ => panic!("an entry holds one or two abilities: {text}"),
            }
        }
    }

    #[test]
    fn r114_04_a_number_changed_is_not_written_whole() {
        for (shape, text) in R114_04_SHAPES.iter() {
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
    #[case("ability stickers", "name stickers")]
    #[case("other permanents you own", "permanents you own")]
    #[case("in your graveyard", "in exile")]
    #[case("on this Equipment", "on equipped creature")]
    #[case("+1/+1", "+2/+2")]
    #[case("Roll a d8", "Roll a d6")]
    #[case("Creature spells you cast", "Artifact spells you cast")]
    #[case("sticker kicker {1}", "sticker kicker {2}")]
    #[case("where X is the result", "where X is twice the result")]
    fn r114_04_a_near_miss_reads_as_nothing(#[case] from: &str, #[case] to: &str) {
        for (_, text) in R114_04_SHAPES.iter() {
            if text.contains(from) {
                let near = text.replacen(from, to, 1);
                assert_eq!(r114_04_whole_read(&near), None, "{near}");
                assert_eq!(r114_04_pair_read(&near), None, "{near}");
            }
        }
    }
}
