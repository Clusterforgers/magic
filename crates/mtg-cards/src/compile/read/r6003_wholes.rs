//! r60-03 sentences no template shares, read and written whole: each printed sentence is paired with the one ability it
//! means (`r6003_wholes.json`: a list of `[sentence, ability]` pairs), so a sentence reads only when it is exactly the
//! printed one and the ability is worded only as that sentence.

use super::tokens::same_text;
use crate::ability::Ability;
use std::sync::LazyLock;

/// The pairs, as JSON.
const R6003_WHOLES: &str = include_str!("r6003_wholes.json");

/// The pairs parsed once.
static R6003_PAIRS: LazyLock<Vec<(String, Ability)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(String, serde_json::Value)>>(R6003_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(text, shape)| Some((text, serde_json::from_value(shape).ok()?)))
        .collect()
});

/// The ability of a printed sentence of the table.
pub(super) fn r6003_whole_read(text: &str) -> Option<Ability> {
    R6003_PAIRS
        .iter()
        .find(|(sentence, _)| same_text(sentence, text))
        .map(|(_, ability)| ability.clone())
}

/// Pairs only written, never read: the sentence is also printed on a card whose other clauses the engine leaves to the
/// table, which would then run this one alone ("Activated abilities of sources with the chosen name can't be activated
/// unless they're mana abilities." after a name chosen by a trigger).
const R6003_WRITTEN: &str = include_str!("r6003_written.json");

/// The written-only pairs parsed once.
static R6003_WRITTEN_PAIRS: LazyLock<Vec<(String, Ability)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(String, serde_json::Value)>>(R6003_WRITTEN)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(text, shape)| Some((text, serde_json::from_value(shape).ok()?)))
        .collect()
});

/// Adds the sentence of an ability of the table.
pub(super) fn r6003_whole_texts(ability: &Ability, texts: &mut Vec<String>) {
    for (sentence, shape) in R6003_PAIRS.iter().chain(R6003_WRITTEN_PAIRS.iter()) {
        if shape == ability && !texts.contains(sentence) {
            texts.push(sentence.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every pair of the tables parses, so none is silently dropped; each pair reads and writes back as itself, and
    //! a written-only pair is written and never read.
    #![allow(clippy::unwrap_used)]
    use super::{
        R6003_PAIRS, R6003_WHOLES, R6003_WRITTEN, R6003_WRITTEN_PAIRS, r6003_whole_read,
        r6003_whole_texts,
    };

    #[test]
    fn r6003_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6003_WHOLES).unwrap();
        assert_eq!(R6003_PAIRS.len(), entries.len());
        for (sentence, ability) in R6003_PAIRS.iter() {
            assert_eq!(r6003_whole_read(sentence).as_ref(), Some(ability));
            let mut texts = Vec::new();
            r6003_whole_texts(ability, &mut texts);
            assert_eq!(texts, vec![sentence.clone()]);
        }
        let written: Vec<serde_json::Value> = serde_json::from_str(R6003_WRITTEN).unwrap();
        assert_eq!(R6003_WRITTEN_PAIRS.len(), written.len());
        for (sentence, ability) in R6003_WRITTEN_PAIRS.iter() {
            assert_eq!(
                r6003_whole_read(sentence),
                None,
                "{sentence} is written only"
            );
            let mut texts = Vec::new();
            r6003_whole_texts(ability, &mut texts);
            assert_eq!(texts, vec![sentence.clone()]);
        }
    }
}
