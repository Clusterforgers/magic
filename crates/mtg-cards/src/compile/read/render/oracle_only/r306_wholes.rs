//! Whole abilities of the r30-6 card pack whose steps the renderers do not join: each is matched whole as data and
//! worded as the card prints it, so another count, target, condition or duration in the same ability never reads back.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording (`r306_wholes.json`: a list of `[ability, wording]` pairs, the card's
/// name as `~`).
const R306_WHOLES: &str = include_str!("r306_wholes.json");

/// The entries parsed once.
static R306_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R306_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R306_SHAPES.iter() {
        if shape == ability {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the r30-6 table parses as an ability, so none is silently dropped.
    use super::{R306_SHAPES, R306_WHOLES};

    #[test]
    fn r306_every_whole_parses() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R306_WHOLES).unwrap();
        assert_eq!(R306_SHAPES.len(), entries.len());
    }
}
