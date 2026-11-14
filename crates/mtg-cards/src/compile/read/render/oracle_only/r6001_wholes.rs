//! Whole abilities of the r60-01 card pack that no other writer words: each is matched whole as data and worded as the
//! card prints it, and the compiler reads the same wording back as the same data (`r6001_read_whole`).

use std::sync::LazyLock;

use crate::ability::Ability;

/// The abilities, as JSON, each with its wording (`r6001_wholes.json`: a list of `[ability, wording]` pairs, the card's
/// name as `~`).
const R6001_WHOLES: &str = include_str!("r6001_wholes.json");

/// Pairs of abilities that one sentence prints, as JSON, each with its wording (`r6001_pairs.json`: a list of
/// `[[first, second], wording]`).
const R6001_PAIRS: &str = include_str!("r6001_pairs.json");

/// The pairs parsed once.
static R6001_PAIR_SHAPES: LazyLock<Vec<([Ability; 2], String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<([serde_json::Value; 2], String)>>(R6001_PAIRS)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|([first, second], text)| {
            Some((
                [
                    serde_json::from_value(first).ok()?,
                    serde_json::from_value(second).ok()?,
                ],
                text,
            ))
        })
        .collect()
});

/// The wordings of two abilities one of the pairs matches in order.
pub(in crate::compile::read) fn r6001_pair_texts(first: &Ability, second: &Ability) -> Vec<String> {
    R6001_PAIR_SHAPES
        .iter()
        .filter(|([one, two], _)| one == first && two == second)
        .map(|(_, text)| text.clone())
        .collect()
}

/// The two abilities of the pair whose wording is the whole of the rest of the sentence.
pub(in crate::compile::read) fn r6001_pair_read(
    r: &mut crate::compile::read::tokens::Reader<'_>,
) -> Option<Vec<Ability>> {
    R6001_PAIR_SHAPES
        .iter()
        .find_map(|([first, second], text)| {
            r.attempt(|r| {
                (r.r376_text(text) && r.end()).then(|| vec![first.clone(), second.clone()])
            })
        })
}

/// The entries parsed once.
static R6001_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R6001_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Adds the wording of an ability that one of the entries matches whole.
pub(in crate::compile::read) fn add(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R6001_SHAPES.iter() {
        if shape == ability {
            texts.push(text.clone());
        }
    }
}

/// The ability of the entry worded exactly as `text`.
pub(in crate::compile::read) fn r6001_read_whole(text: &str) -> Option<Ability> {
    R6001_SHAPES
        .iter()
        .find(|(_, wording)| wording == text)
        .map(|(shape, _)| shape.clone())
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the r60-01 table parses, and reads back as its own data only from its exact wording.
    use super::{
        R6001_PAIR_SHAPES, R6001_PAIRS, R6001_SHAPES, R6001_WHOLES, r6001_pair_texts,
        r6001_read_whole,
    };

    #[test]
    fn r6001_every_whole_parses() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6001_WHOLES).unwrap();
        assert_eq!(R6001_SHAPES.len(), entries.len());
    }

    #[test]
    fn r6001_every_pair_parses_and_writes_its_wording() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R6001_PAIRS).unwrap();
        assert_eq!(R6001_PAIR_SHAPES.len(), entries.len());
        for ([first, second], text) in R6001_PAIR_SHAPES.iter() {
            assert_eq!(r6001_pair_texts(first, second), std::slice::from_ref(text));
            assert!(
                r6001_pair_texts(second, first).is_empty(),
                "the order matters"
            );
        }
    }

    #[test]
    fn r6001_only_the_exact_wording_reads() {
        for (shape, text) in R6001_SHAPES.iter() {
            assert_eq!(r6001_read_whole(text).as_ref(), Some(shape));
            assert_eq!(r6001_read_whole(&text.replacen(' ', "  ", 1)), None);
            assert_eq!(
                r6001_read_whole(&text.to_lowercase().replacen('.', "", 1)),
                None
            );
        }
    }
}
