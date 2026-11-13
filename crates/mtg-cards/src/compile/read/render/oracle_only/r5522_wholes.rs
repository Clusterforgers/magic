//! Whole abilities of the r55-22 pack whose wordings no renderer joins (facts of the turn or the game in a sentence of
//! their own): the ability is matched whole as data and worded as the card prints it, and the printed wording reads back
//! into the same ability.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording (`r5522_wholes.json`: a list of `[ability, wording]` pairs).
const R5522_WHOLES: &str = include_str!("r5522_wholes.json");

/// The entries parsed once.
static R5522_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R5522_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r5522_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R5522_SHAPES.iter() {
        if shape == ability {
            texts.push(text.clone());
        }
    }
}

/// The ability of the entry whose wording is the text, compared as `same` compares clauses.
pub(crate) fn r5522_read_whole(text: &str, same: impl Fn(&str, &str) -> bool) -> Option<Ability> {
    R5522_SHAPES
        .iter()
        .find(|(_, wording)| same(wording, text))
        .map(|(shape, _)| shape.clone())
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses as an ability, so none is silently dropped.
    use super::{R5522_SHAPES, R5522_WHOLES};

    #[test]
    fn r5522_every_whole_parses() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R5522_WHOLES).unwrap();
        assert_eq!(R5522_SHAPES.len(), entries.len());
    }
}
