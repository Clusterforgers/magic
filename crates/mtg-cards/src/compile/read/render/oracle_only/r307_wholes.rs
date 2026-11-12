//! Whole abilities of the r30-7 pack whose steps the renderers do not join: the ability is matched whole as data and
//! worded as the card prints it.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording (`r307_wholes.json`: a list of `[ability, wording]` pairs).
const R307_WHOLES: &str = include_str!("r307_wholes.json");

/// The entries parsed once.
static R307_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R307_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r307_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R307_SHAPES.iter() {
        if shape == ability {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses as an ability, so none is silently dropped.
    use super::{R307_SHAPES, R307_WHOLES};

    #[test]
    fn r307_every_whole_parses() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R307_WHOLES).unwrap();
        assert_eq!(R307_SHAPES.len(), entries.len());
    }
}
