//! Whole abilities of the r64-04 card pack that no other writer words: each is matched whole as data and worded as the
//! card prints it, and the compiler reads the same wording back as the same data (`r6404_read_whole`).

use std::sync::LazyLock;

use crate::ability::Ability;

/// The abilities, as JSON, each with its wording (`r6404_wholes.json`: a list of `[ability, wording]` pairs, the card's
/// name as `~`).
const R6404_WHOLES: &str = include_str!("r6404_wholes.json");

/// The entries parsed once.
static R6404_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6404_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(in crate::compile::read) fn r6404_add(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R6404_SHAPES.iter() {
        if shape == ability {
            texts.push(text.clone());
        }
    }
}

/// The ability of the entry worded exactly as `text`.
pub(in crate::compile::read) fn r6404_read_whole(text: &str) -> Option<Ability> {
    R6404_SHAPES
        .iter()
        .find(|(_, wording)| wording == text)
        .map(|(shape, _)| shape.clone())
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the r64-04 table parses, and reads back as its own data only from its exact wording.
    use super::{R6404_SHAPES, R6404_WHOLES, r6404_read_whole};

    #[test]
    fn r6404_every_whole_parses() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6404_WHOLES).unwrap();
        assert_eq!(R6404_SHAPES.len(), entries.len());
    }

    #[test]
    fn r6404_only_the_exact_wording_reads() {
        for (shape, text) in R6404_SHAPES.iter() {
            assert_eq!(r6404_read_whole(text).as_ref(), Some(shape));
            assert_eq!(r6404_read_whole(&text.replacen(' ', "  ", 1)), None);
            assert_eq!(
                r6404_read_whole(&text.to_lowercase().replacen('.', "", 1)),
                None
            );
        }
    }
}
