//! Whole abilities of the r55-08 pack whose wordings no renderer joins: the ability is matched whole as data and
//! worded as the card prints it.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording (`r5508_wholes.json`: a list of `[ability, wording]` pairs).
const R5508_WHOLES: &str = include_str!("r5508_wholes.json");

/// The entries parsed once.
static R5508_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R5508_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r5508_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R5508_SHAPES.iter() {
        if shape == ability {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses as an ability, so none is silently dropped.
    use super::{R5508_SHAPES, R5508_WHOLES};

    #[test]
    fn r5508_every_whole_parses() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R5508_WHOLES).unwrap();
        assert_eq!(R5508_SHAPES.len(), entries.len());
    }
}
