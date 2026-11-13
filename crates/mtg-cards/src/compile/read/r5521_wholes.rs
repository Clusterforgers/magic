//! Whole abilities of the r55-21 pack whose wordings no renderer joins (an enters trigger that returns the card and
//! ventures in one sentence, a temptation followed by a draw under two conditions, ...): each is read and written whole
//! from one table of `[ability, wording]` pairs (`r5521_wholes.json`), so the reader reads exactly what the writer writes.

use std::sync::LazyLock;

use super::ClauseKind;
use super::tokens::same_text;
use crate::ability::Ability;

/// The abilities, as JSON, each with its wording, `~` for the card's name.
const R5521_WHOLES: &str = include_str!("r5521_wholes.json");

/// The entries parsed once; one that does not parse is dropped (a test holds that none is).
static R5521_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R5521_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Whether a clause of this kind can hold the ability.
fn r5521_kind_fits(ability: &Ability, kind: ClauseKind) -> bool {
    match ability {
        Ability::Triggered { .. } => kind == ClauseKind::Triggered,
        Ability::Activated { .. } => kind == ClauseKind::Activated,
        Ability::Loyalty { .. } => kind == ClauseKind::Loyalty,
        Ability::Spell { .. } => matches!(kind, ClauseKind::Spell | ClauseKind::Modal),
        _ => kind == ClauseKind::Static,
    }
}

/// Reads a clause that is one of the wordings, whole.
pub(super) fn r5521_read(text: &str, kind: ClauseKind) -> Option<Ability> {
    R5521_SHAPES
        .iter()
        .find(|(shape, wording)| r5521_kind_fits(shape, kind) && same_text(wording, text))
        .map(|(shape, _)| shape.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r5521_wordings(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in R5521_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back as its ability from its wording and is written as it, and the
    //! same ability with any one number changed is not written as that wording.
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::compile::read::render::written_variants;

    #[test]
    fn r5521_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R5521_WHOLES).unwrap();
        assert_eq!(R5521_SHAPES.len(), entries.len());
        for (shape, text) in R5521_SHAPES.iter() {
            let kind = match shape {
                Ability::Triggered { .. } => ClauseKind::Triggered,
                Ability::Activated { .. } => ClauseKind::Activated,
                Ability::Loyalty { .. } => ClauseKind::Loyalty,
                Ability::Spell { .. } => ClauseKind::Spell,
                _ => ClauseKind::Static,
            };
            assert_eq!(r5521_read(text, kind).as_ref(), Some(shape), "{text}");
            assert!(written_variants(shape).contains(text), "{text}");
        }
    }

    #[test]
    fn r5521_a_number_changed_is_not_written_whole() {
        for (shape, text) in R5521_SHAPES.iter() {
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
    fn r5521_a_wording_of_another_kind_is_not_read() {
        for (_, text) in R5521_SHAPES.iter() {
            assert_eq!(r5521_read(text, ClauseKind::Keyword), None, "{text}");
        }
    }
}
