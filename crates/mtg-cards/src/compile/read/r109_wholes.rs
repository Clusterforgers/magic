//! Whole abilities of the r109 pack whose wordings no renderer joins (an enters trigger that reads the emerge sacrifice,
//! a sacrifice while casting a spell with emerge, a spell's waterbend cost): each is read and written whole from one
//! table of `[ability, wording]` pairs (`r109_wholes.json`), so the reader reads exactly what the writer writes.

use std::sync::LazyLock;

use super::ClauseKind;
use super::tokens::same_text;
use crate::ability::Ability;

/// The abilities, as JSON, each with its wording, `~` for the card's name.
const R109_WHOLES: &str = include_str!("r109_wholes.json");

/// The entries parsed once; one that does not parse is dropped (a test holds that none is).
static R109_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R109_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// The kind of clause that holds the ability.
fn r109_kind_of(ability: &Ability) -> ClauseKind {
    match ability {
        Ability::Triggered { .. } => ClauseKind::Triggered,
        Ability::Activated { .. } => ClauseKind::Activated,
        Ability::Loyalty { .. } => ClauseKind::Loyalty,
        Ability::Spell { .. } => ClauseKind::Spell,
        _ => ClauseKind::Static,
    }
}

/// Whether a clause of this kind can hold the ability.
fn r109_kind_fits(ability: &Ability, kind: ClauseKind) -> bool {
    match r109_kind_of(ability) {
        ClauseKind::Spell => matches!(kind, ClauseKind::Spell | ClauseKind::Modal),
        own => own == kind,
    }
}

/// Reads a clause that is one of the wordings, whole.
pub(super) fn r109_read(text: &str, kind: ClauseKind) -> Option<Ability> {
    R109_SHAPES
        .iter()
        .find(|(shape, wording)| r109_kind_fits(shape, kind) && same_text(wording, text))
        .map(|(shape, _)| shape.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r109_wordings(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R109_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back as its ability from its wording and is written as it, the same
    //! ability with any one number changed is not written as that wording, and a wording of another kind is not read.
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::compile::read::render::written_variants;

    #[test]
    fn r109_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R109_WHOLES).unwrap();
        assert_eq!(R109_SHAPES.len(), entries.len());
        for (shape, text) in R109_SHAPES.iter() {
            assert_eq!(
                r109_read(text, r109_kind_of(shape)).as_ref(),
                Some(shape),
                "{text}"
            );
            assert!(written_variants(shape).contains(text), "{text}");
        }
    }

    #[test]
    fn r109_a_number_changed_is_not_written_whole() {
        for (shape, text) in R109_SHAPES.iter() {
            let json = serde_json::to_string(shape).unwrap();
            for (at, digit) in json.char_indices().filter(|(_, c)| c.is_ascii_digit()) {
                let raised = char::from_digit((digit.to_digit(10).unwrap() + 1) % 10, 10).unwrap();
                let changed = format!("{}{raised}{}", &json[..at], &json[at + 1..]);
                if let Ok(near) = serde_json::from_str::<Ability>(&changed) {
                    assert!(!written_variants(&near).contains(text), "{changed}");
                }
            }
        }
    }

    #[test]
    fn r109_a_wording_of_another_kind_is_not_read() {
        for (_, text) in R109_SHAPES.iter() {
            assert_eq!(r109_read(text, ClauseKind::Keyword), None, "{text}");
        }
    }
}
