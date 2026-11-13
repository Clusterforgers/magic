//! r55-10 abilities whose wordings no other reader or writer joins: each entry of `r5510_wholes.json` is an ability with
//! the words the card prints for it, read when a clause says exactly those words and written back from the ability.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const R5510_WHOLES: &str = include_str!("r5510_wholes.json");

/// The entries parsed once.
static R5510_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(R5510_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// "Crew 1. Activate only once each turn.": crew that may be activated once each turn.
fn r5510_crew_once(text: &str) -> Option<Ability> {
    let power = text
        .trim()
        .trim_end_matches('.')
        .strip_prefix("Crew ")?
        .strip_suffix(". Activate only once each turn")?;
    let power: u32 = power.parse().ok().filter(|power| *power > 0)?;
    Some(Ability::Keyword(crate::keyword::Keyword::CrewOnce(power)))
}

/// The ability of the entry whose wording is the clause.
pub(super) fn r5510_whole_read(text: &str) -> Option<Ability> {
    if let Some(found) = r5510_crew_once(text) {
        return Some(found);
    }
    R5510_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(ability, _)| ability.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn r5510_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    if let Ability::Keyword(crate::keyword::Keyword::CrewOnce(power)) = ability {
        texts.push(format!("Crew {power}. Activate only once each turn"));
    }
    for (shape, text) in R5510_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and from no other entry's, and a near miss
    //! of its words (a number changed) reads as nothing.
    use super::{R5510_SHAPES, R5510_WHOLES, r5510_add_wholes, r5510_whole_read};
    use crate::ability::Ability;
    use crate::keyword::Keyword;
    use rstest::rstest;

    #[rstest]
    #[case::one("Crew 1. Activate only once each turn.", Some(1))]
    #[case::three("Crew 3. Activate only once each turn.", Some(3))]
    #[case::zero("Crew 0. Activate only once each turn.", None)]
    #[case::twice("Crew 1. Activate only twice each turn.", None)]
    #[case::plain("Crew 1", None)]
    fn r5510_crew_once_reads_and_writes_back(#[case] text: &str, #[case] power: Option<u32>) {
        let found = r5510_whole_read(text);
        assert_eq!(
            found,
            power.map(|power| Ability::Keyword(Keyword::CrewOnce(power)))
        );
        if let Some(ability) = found {
            let mut texts = Vec::new();
            r5510_add_wholes(&ability, &mut texts);
            assert_eq!(texts, [text.trim_end_matches('.')]);
        }
    }

    #[test]
    fn r5510_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(R5510_WHOLES).unwrap();
        assert_eq!(R5510_SHAPES.len(), entries.len());
        for (ability, text) in R5510_SHAPES.iter() {
            assert_eq!(r5510_whole_read(text).as_ref(), Some(ability), "{text}");
            let mut texts = Vec::new();
            r5510_add_wholes(ability, &mut texts);
            assert!(texts.contains(text));
        }
    }

    #[test]
    fn r5510_a_changed_number_reads_as_nothing() {
        for (_, text) in R5510_SHAPES.iter() {
            for (from, to) in [("3", "4"), ("2", "7"), ("5", "6")] {
                if text.contains(from) {
                    assert_eq!(
                        r5510_whole_read(&text.replacen(from, to, 1)),
                        None,
                        "{text}"
                    );
                }
            }
        }
    }
}
