//! Whole abilities of the r60-08 pack whose sentences no other reader or writer words (granted quoted abilities,
//! optional replacements, choices for each player): each table entry is read from its sentence and worded back as the
//! card prints it.

use std::sync::LazyLock;

use super::tokens::same_written_text;
use crate::ability::Ability;

/// The abilities, as JSON, each with its wording (`r6008_wholes.json`: a list of `[ability, wording]` pairs).
const R6008_WHOLES: &str = include_str!("r6008_wholes.json");

/// The entries parsed once.
static R6008_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6008_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose sentence the clause is.
pub(super) fn r6008_read(text: &str) -> Option<Ability> {
    R6008_SHAPES
        .iter()
        .find(|(_, whole)| same_written_text(whole, text))
        .map(|(shape, _)| shape.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r6008_wordings(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R6008_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses as an ability and reads back from its own sentence and from no near miss of
    //! it, so none is silently dropped or over-read.
    use super::{R6008_SHAPES, R6008_WHOLES, r6008_read};

    #[test]
    fn r6008_every_whole_parses_and_reads() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6008_WHOLES).unwrap();
        assert_eq!(R6008_SHAPES.len(), entries.len());
        for (shape, text) in R6008_SHAPES.iter() {
            assert_eq!(r6008_read(text).as_ref(), Some(shape), "{text}");
            // A near miss: the same sentence with more after it, or with a word changed, reads as other text.
            assert!(
                r6008_read(&format!("{text} Draw a card.")).is_none(),
                "{text}"
            );
            assert!(
                r6008_read(&text.replacen(' ', " not ", 1)).is_none(),
                "{text}"
            );
        }
    }
}
