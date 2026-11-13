//! Whole abilities of the r47-11 pack whose wordings no renderer joins (draft notes in a sentence of their own): the ability is matched whole as data and
//! worded as the card prints it.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording (`r4711_wholes.json`: a list of `[ability, wording]` pairs).
const R4711_WHOLES: &str = include_str!("r4711_wholes.json");

/// The entries parsed once.
static R4711_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R4711_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r4711_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R4711_SHAPES.iter() {
        if shape == ability {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses as an ability, so none is silently dropped.
    use super::{R4711_SHAPES, R4711_WHOLES};

    #[test]
    fn r4711_every_whole_parses() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R4711_WHOLES).unwrap();
        assert_eq!(R4711_SHAPES.len(), entries.len());
    }
}
