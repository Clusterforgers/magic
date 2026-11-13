//! Whole abilities of the r55-13 pack whose events no renderer joins ("fight or become blocked", a creature bound for a delayed trigger): the ability is matched
//! whole as data and worded as the card prints it.

use crate::ability::Ability;
use std::sync::LazyLock;

/// The abilities, as JSON, each with its wording (`r5513_wholes.json`: a list of `[ability, wording]` pairs).
const R5513_WHOLES: &str = include_str!("r5513_wholes.json");

/// The entries parsed once.
static R5513_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R5513_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r5513_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R5513_SHAPES.iter() {
        if shape == ability {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses as an ability, so none is silently dropped.
    use super::{R5513_SHAPES, R5513_WHOLES};

    #[test]
    fn r5513_every_whole_parses() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R5513_WHOLES).unwrap();
        assert_eq!(R5513_SHAPES.len(), entries.len());
    }
}
