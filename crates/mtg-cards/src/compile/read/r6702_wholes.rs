//! r67-02 abilities whose wordings no other reader or writer joins: each entry of `r6702_wholes.json` is an ability with
//! the words the card prints for it (the card's name as `~`), read when a clause says exactly those words and written
//! back from the ability, so another count, step or event in the same ability never reads back.

use std::sync::LazyLock;

use super::tokens::{Reader, same_text};
use crate::ability::Ability;
use crate::effect::Effect;

/// Steps of a spell or ability, as JSON, each with the words of its sentence without the period
/// (`r6702_effects.json`: a list of `[effect, wording]`).
const R6702_EFFECTS: &str = include_str!("r6702_effects.json");

/// The steps parsed once.
static R6702_EFFECT_SHAPES: LazyLock<Vec<(Effect, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6702_EFFECTS)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The step whose words the sentence says, up to its end.
pub(super) fn r6702_effect_read(r: &mut Reader<'_>) -> Option<Vec<Effect>> {
    R6702_EFFECT_SHAPES
        .iter()
        .find_map(|(effect, text)| r.attempt(|r| r.r376_text(text).then(|| vec![effect.clone()])))
}

/// The words of a step one of the entries matches whole.
pub(super) fn r6702_effect_text(effect: &Effect) -> Option<String> {
    R6702_EFFECT_SHAPES
        .iter()
        .find(|(shape, _)| shape == effect)
        .map(|(_, text)| text.clone())
}

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R6702_WHOLES: &str = include_str!("r6702_wholes.json");

/// The entries parsed once.
static R6702_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6702_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose wording is the clause.
pub(super) fn r6702_whole_read(text: &str) -> Option<Ability> {
    R6702_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r6702_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R6702_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, and a near miss of its
    //! words (a number, a player or a duration changed) reads as nothing.
    use super::{
        R6702_EFFECT_SHAPES, R6702_EFFECTS, R6702_SHAPES, R6702_WHOLES, r6702_add_wholes,
        r6702_effect_text, r6702_whole_read,
    };

    #[test]
    fn r6702_every_step_parses_and_writes_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6702_EFFECTS).unwrap();
        assert_eq!(R6702_EFFECT_SHAPES.len(), entries.len());
        for (effect, text) in R6702_EFFECT_SHAPES.iter() {
            assert_eq!(r6702_effect_text(effect).as_deref(), Some(text.as_str()));
        }
    }

    #[test]
    fn r6702_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6702_WHOLES).unwrap();
        assert_eq!(R6702_SHAPES.len(), entries.len());
        for (ability, text) in R6702_SHAPES.iter() {
            assert_eq!(r6702_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r6702_add_wholes(ability, &mut texts);
            assert!(texts.contains(text), "{text}");
        }
    }

    #[test]
    fn r6702_a_near_miss_reads_as_nothing() {
        let swaps = [
            ("a creature", "an artifact"),
            ("may attach", "attach"),
            ("this Aura", "this creature"),
            ("face up or face down", "face up"),
            ("1 life", "2 life"),
            ("Double", "Triple"),
            ("three or more", "two or more"),
            ("up to three", "up to two"),
            ("total power", "total toughness"),
            ("first strike, ", ""),
            ("each creature and planeswalker", "each creature"),
            ("evidence X", "evidence 4"),
            ("most life", "least life"),
            ("Silver Surfer", "Norin"),
            ("activated and triggered", "activated"),
            ("an opponent's graveyard", "your graveyard"),
            ("if able", "if you like"),
        ];
        for (_, text) in R6702_SHAPES.iter() {
            for (from, to) in swaps {
                if text.contains(from) {
                    let near = text.replacen(from, to, 1);
                    assert_eq!(r6702_whole_read(&near), None, "{near}");
                }
            }
        }
    }
}
