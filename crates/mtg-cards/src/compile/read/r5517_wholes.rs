//! Whole abilities of the r55-17 pack whose sentences no other reader or writer words (a search of the library,
//! graveyard and outside the game, a reveal until a number of cards, replacements that move control): each table entry
//! is read from its sentence and worded back as the card prints it.

use std::sync::LazyLock;

use super::tokens::same_written_text;
use crate::ability::Ability;

/// The abilities, as JSON, each with its wording (`r5517_wholes.json`: a list of `[ability, wording]` pairs).
const R5517_WHOLES: &str = include_str!("r5517_wholes.json");

/// The entries parsed once.
static R5517_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R5517_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose sentence the clause is.
pub(super) fn r5517_read(text: &str) -> Option<Ability> {
    R5517_SHAPES
        .iter()
        .find(|(_, whole)| same_written_text(whole, text))
        .map(|(shape, _)| shape.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r5517_wordings(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R5517_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses as an ability and reads back from its own sentence, so none is silently
    //! dropped.
    use super::{R5517_SHAPES, R5517_WHOLES, r5517_read};

    #[test]
    fn r5517_every_whole_parses_and_reads() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R5517_WHOLES).unwrap();
        assert_eq!(R5517_SHAPES.len(), entries.len());
        for (shape, text) in R5517_SHAPES.iter() {
            assert_eq!(r5517_read(text).as_ref(), Some(shape), "{text}");
        }
    }
}
