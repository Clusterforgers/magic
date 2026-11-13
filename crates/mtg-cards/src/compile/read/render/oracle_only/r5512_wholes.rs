//! Whole abilities of the r55-12 pack whose wordings no renderer joins (Aura moves, a spell exiled as a cost, a new duration, paired colors of mana): the ability is matched whole as data and
//! worded as the card prints it.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording (`r5512_wholes.json`: a list of `[ability, wording]` pairs).
const R5512_WHOLES: &str = include_str!("r5512_wholes.json");

/// The entries parsed once.
static R5512_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R5512_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r5512_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R5512_SHAPES.iter() {
        if shape == ability {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses as an ability, so none is silently dropped.
    use super::{R5512_SHAPES, R5512_WHOLES};

    #[test]
    fn r5512_every_whole_parses() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R5512_WHOLES).unwrap();
        assert_eq!(R5512_SHAPES.len(), entries.len());
    }
}
