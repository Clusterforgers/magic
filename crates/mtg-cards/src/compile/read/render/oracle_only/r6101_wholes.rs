//! Whole abilities of the r61-01 pack (stickers and text changes) whose steps the renderers do not join: the ability is matched whole as data and
//! worded as the card prints it.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording (`r6101_wholes.json`: a list of `[ability, wording]` pairs).
const R6101_WHOLES: &str = include_str!("r6101_wholes.json");

/// The entries parsed once.
static R6101_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6101_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r6101_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R6101_SHAPES.iter() {
        if shape == ability {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses as an ability, so none is silently dropped.
    use super::{R6101_SHAPES, R6101_WHOLES};

    #[test]
    fn r6101_every_whole_parses() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6101_WHOLES).unwrap();
        assert_eq!(R6101_SHAPES.len(), entries.len());
    }
}
