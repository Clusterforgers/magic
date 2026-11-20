//! r110-02 abilities whose wordings no other reader or writer joins (choices made as a spell is cast, copies, text
//! changes and type slots): each entry of `r110_02_wholes.json` is an ability with the words the card prints for it (the card's
//! name as `~`), read when a clause says exactly those words and written back from the ability.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R110_02_WHOLES: &str = include_str!("r110_02_wholes.json");

/// The entries parsed once.
static R110_02_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R110_02_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The ability of the entry whose wording is the clause.
pub(super) fn r110_02_whole_read(text: &str) -> Option<Ability> {
    R110_02_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r110_02_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R110_02_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, and a near miss of its
    //! words (a zone, a duration, a kind or a number changed) reads as nothing.
    use super::{R110_02_SHAPES, R110_02_WHOLES, r110_02_add_wholes, r110_02_whole_read};

    #[test]
    fn r110_02_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R110_02_WHOLES).unwrap();
        assert_eq!(R110_02_SHAPES.len(), entries.len());
        for (ability, text) in R110_02_SHAPES.iter() {
            assert_eq!(r110_02_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r110_02_add_wholes(ability, &mut texts);
            assert!(texts.contains(text), "{text}");
        }
    }

    #[test]
    fn r110_02_a_near_miss_reads_as_nothing() {
        let swaps = [
            ("between 0 and 5", "between 1 and 5"),
            ("target creature", "target permanent"),
            ("creature type", "color"),
            ("1/1 Spirit", "2/2 Spirit"),
            ("with flying", "with haste"),
            ("from your graveyard", "from a graveyard"),
            ("X additional", "two additional"),
        ];
        for (_, text) in R110_02_SHAPES.iter() {
            for (from, to) in swaps {
                if text.contains(from) {
                    let near = text.replacen(from, to, 1);
                    assert_eq!(r110_02_whole_read(&near), None, "{near}");
                }
            }
        }
    }
}
