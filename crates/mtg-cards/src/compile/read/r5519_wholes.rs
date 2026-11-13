//! r55-19 sentences and spell texts no template shares, read and written whole: each printed sentence is paired with the one ability it
//! means (`r5519_wholes.json`: a list of `[sentence, ability]` pairs), so a sentence reads only when it is exactly the
//! printed one and the ability is worded only as that sentence.

use super::tokens::same_text;
use crate::ability::Ability;
use std::sync::LazyLock;

/// The pairs, as JSON.
const R5519_WHOLES: &str = include_str!("r5519_wholes.json");

/// The pairs parsed once.
static R5519_PAIRS: LazyLock<Vec<(String, Ability)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(String, serde_json::Value)>>(R5519_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(text, shape)| Some((text, serde_json::from_value(shape).ok()?)))
        .collect()
});

/// The ability of a printed sentence of the table.
pub(super) fn r5519_whole_read(text: &str) -> Option<Ability> {
    R5519_PAIRS
        .iter()
        .find(|(sentence, _)| same_text(sentence, text))
        .map(|(_, ability)| ability.clone())
}

/// Adds the sentence of an ability of the table.
pub(super) fn r5519_whole_texts(ability: &Ability, texts: &mut Vec<String>) {
    for (sentence, shape) in R5519_PAIRS.iter() {
        if shape == ability && !texts.contains(sentence) {
            texts.push(sentence.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every pair of the table parses, so none is silently dropped, and each reads and writes back as itself.
    #![allow(clippy::unwrap_used)]
    use super::{R5519_PAIRS, R5519_WHOLES, r5519_whole_read, r5519_whole_texts};

    #[test]
    fn r5519_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R5519_WHOLES).unwrap();
        assert_eq!(R5519_PAIRS.len(), entries.len());
        for (sentence, ability) in R5519_PAIRS.iter() {
            assert_eq!(r5519_whole_read(sentence).as_ref(), Some(ability));
            let mut texts = Vec::new();
            r5519_whole_texts(ability, &mut texts);
            assert_eq!(texts, vec![sentence.clone()]);
        }
    }
}
