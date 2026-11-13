//! r47-05 readers of whole abilities whose wording one card prints: each entry of `r4705_wholes.json` pairs an ability
//! with its printed wording, read only when the text is that wording and written back by the same table.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The abilities, as JSON, each with its wording (a list of `[ability, wording]` pairs; a list of two abilities is one
/// sentence that holds both).
const R4705_WHOLES: &str = include_str!("render/oracle_only/r4705_wholes.json");

/// The entries parsed once.
pub(in crate::compile::read) static R4705_SHAPES: LazyLock<Vec<(Vec<Ability>, String)>> =
    LazyLock::new(|| {
        serde_json::from_str::<Vec<(serde_json::Value, String)>>(R4705_WHOLES)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(shape, text)| {
                let shape = match shape {
                    serde_json::Value::Array(list) => list
                        .into_iter()
                        .map(serde_json::from_value)
                        .collect::<Result<Vec<Ability>, _>>()
                        .ok()?,
                    one => vec![serde_json::from_value(one).ok()?],
                };
                Some((shape, text))
            })
            .collect()
    });

/// Reads a whole ability of the r47-05 table.
pub(super) fn r4705_read(text: &str) -> Option<Ability> {
    R4705_SHAPES
        .iter()
        .find_map(|(shape, wording)| match shape.as_slice() {
            [one] if same_text(wording, text) => Some(one.clone()),
            _ => None,
        })
}

/// Reads a sentence of the r47-05 table that holds two abilities.
pub(super) fn r4705_pair_read(text: &str) -> Option<Vec<Ability>> {
    R4705_SHAPES
        .iter()
        .find(|(shape, wording)| shape.len() == 2 && same_text(wording, text))
        .map(|(shape, _)| shape.clone())
}

/// The wordings of the r47-05 table for two abilities in one sentence.
pub(super) fn r4705_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    R4705_SHAPES
        .iter()
        .filter(|(shape, _)| matches!(shape.as_slice(), [a, b] if a == first && b == second))
        .map(|(_, wording)| wording.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the r47-05 table parses as an ability and reads back from its own wording alone.
    use super::{R4705_SHAPES, R4705_WHOLES, r4705_pair_read, r4705_read};

    #[test]
    fn r4705_every_whole_parses_and_reads() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R4705_WHOLES).unwrap();
        assert_eq!(R4705_SHAPES.len(), entries.len());
        for (shape, text) in R4705_SHAPES.iter() {
            let read = match shape.as_slice() {
                [_] => r4705_read(text).map(|found| vec![found]),
                _ => r4705_pair_read(text),
            };
            assert_eq!(read.as_ref(), Some(shape), "{text}");
            assert!(
                r4705_read(&format!("{text} Draw a card.")).is_none(),
                "{text}"
            );
            assert!(
                r4705_pair_read(&format!("{text} Draw a card.")).is_none(),
                "{text}"
            );
        }
    }
}
