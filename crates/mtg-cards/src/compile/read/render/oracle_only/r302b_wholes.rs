//! Whole abilities of the r30-2b pack whose steps the renderers do not join: each is matched whole as data and worded
//! as the card prints it, so another count, target, condition or duration in the same ability never reads back.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording (`r302b_wholes.json`: a list of `[ability, wording]` pairs, the card's
/// name as `~`).
const R302B_WHOLES: &str = include_str!("r302b_wholes.json");

/// The entries parsed once.
static R302B_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R302B_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r302b_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R302B_SHAPES.iter() {
        if shape == ability {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses as an ability, so none is silently dropped.
    use super::{R302B_SHAPES, R302B_WHOLES};

    #[test]
    fn r302b_every_whole_parses() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R302B_WHOLES).unwrap();
        assert_eq!(R302B_SHAPES.len(), entries.len());
    }
}
