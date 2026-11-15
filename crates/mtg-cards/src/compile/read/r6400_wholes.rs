//! r64-00 abilities whose wordings no other reader or writer joins: each entry of `r6400_wholes.json` is an ability with
//! the words the card prints for it (the card's name as `~`), and each entry of `r6400_pairs.json` two abilities one
//! sentence prints. A clause reads as the entry whose words it says exactly and is written back from the ability, so
//! another cost, count or creature in the same ability never reads back.

use std::sync::LazyLock;

use super::tokens::{Reader, same_text};
use crate::ability::Ability;
use crate::effect::Effect;

/// Steps of a spell or ability, as JSON, each with the words of its sentence without the period
/// (`r6400_effects.json`: a list of `[effect, wording]`).
const R6400_EFFECTS: &str = include_str!("r6400_effects.json");

/// The steps parsed once.
static R6400_EFFECT_SHAPES: LazyLock<Vec<(Effect, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6400_EFFECTS)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The step whose words the sentence says, up to its end.
pub(super) fn r6400_effect_read(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    R6400_EFFECT_SHAPES
        .iter()
        .find_map(|(effect, text)| r.attempt(|r| r.r376_text(text).then(|| vec![effect.clone()])))
}

/// The words of a step one of the entries matches whole.
pub(super) fn r6400_effect_text(effect: &Effect) -> Option<String> {
    R6400_EFFECT_SHAPES
        .iter()
        .find(|(shape, _)| shape == effect)
        .map(|(_, text)| text.clone())
}

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R6400_WHOLES: &str = include_str!("r6400_wholes.json");

/// The pairs, as JSON: a list of `[[first, second], wording]`.
const R6400_PAIRS: &str = include_str!("r6400_pairs.json");

/// The entries parsed once.
static R6400_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6400_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The pairs parsed once.
static R6400_PAIR_SHAPES: LazyLock<Vec<([Ability; 2], String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<([serde_json::Value; 2], String)>>(R6400_PAIRS)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|([first, second], text)| {
            Some((
                [
                    serde_json::from_value(first).ok()?,
                    serde_json::from_value(second).ok()?,
                ],
                text,
            ))
        })
        .collect()
});

/// The ability of the entry whose wording is the clause.
pub(super) fn r6400_whole_read(text: &str) -> Option<Ability> {
    R6400_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r6400_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R6400_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

/// The two abilities of the pair whose wording is the whole of the rest of the sentence.
pub(super) fn r6400_pair_read(r: &mut Reader<'_>) -> Option<Vec<Ability>> {
    R6400_PAIR_SHAPES
        .iter()
        .find_map(|([first, second], text)| {
            r.attempt(|r| {
                (r.r376_text(text) && r.end()).then(|| vec![first.clone(), second.clone()])
            })
        })
}

/// The wordings of two abilities one of the pairs matches in order.
pub(super) fn r6400_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    R6400_PAIR_SHAPES
        .iter()
        .filter(|([one, two], _)| one == first && two == second)
        .map(|(_, text)| text.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: every entry and pair of the tables parses, reads back from its own words and writes them back, and a near
    //! miss of its words (a number, a color, a cost or a creature changed) reads as nothing.
    use super::{
        R6400_EFFECT_SHAPES, R6400_EFFECTS, R6400_PAIR_SHAPES, R6400_PAIRS, R6400_SHAPES,
        R6400_WHOLES, r6400_add_wholes, r6400_effect_text, r6400_pair_texts, r6400_whole_read,
    };

    #[test]
    fn r6400_every_entry_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6400_WHOLES).unwrap();
        assert_eq!(R6400_SHAPES.len(), entries.len());
        for (ability, text) in R6400_SHAPES.iter() {
            assert_eq!(r6400_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r6400_add_wholes(ability, &mut texts);
            assert!(texts.contains(text), "{text}");
        }
        let effects: Vec<serde_json::Value> = serde_json::from_str(R6400_EFFECTS).unwrap();
        assert_eq!(R6400_EFFECT_SHAPES.len(), effects.len());
        for (effect, text) in R6400_EFFECT_SHAPES.iter() {
            assert_eq!(r6400_effect_text(effect).as_ref(), Some(text));
        }
        let pairs: Vec<serde_json::Value> = serde_json::from_str(R6400_PAIRS).unwrap();
        assert_eq!(R6400_PAIR_SHAPES.len(), pairs.len());
        for ([first, second], text) in R6400_PAIR_SHAPES.iter() {
            assert!(r6400_pair_texts(first, second).contains(text), "{text}");
            assert!(r6400_pair_texts(second, first).is_empty(), "{text}");
        }
    }

    #[test]
    fn r6400_a_near_miss_reads_as_nothing() {
        let swaps = [
            ("two", "three"),
            ("Islands", "Swamps"),
            ("a land", "a creature"),
            ("Green", "Red"),
            ("Black", "White"),
            ("enchantment", "artifact"),
            ("sacrifice", "exile"),
            ("you control", "an opponent controls"),
            ("attack", "block"),
        ];
        for (_, text) in R6400_SHAPES.iter() {
            for (from, to) in swaps {
                if text.contains(from) {
                    let near = text.replacen(from, to, 1);
                    assert_eq!(r6400_whole_read(&near), None, "{near}");
                }
            }
        }
    }
}
