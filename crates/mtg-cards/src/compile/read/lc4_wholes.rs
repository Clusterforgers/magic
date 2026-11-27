//! lc4 abilities whose wordings no other reader or writer joins (Agent Maria Hill, Captain America, First Avenger,
//! Knowledge Vault, Swirl the Mists): each entry of `lc4_wholes.json` is an ability with the words the card prints for
//! it, read when a clause says exactly those words and written back from the ability.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const LC4_WHOLES: &str = include_str!("lc4_wholes.json");

/// The entries parsed once; one that does not parse is dropped (a test holds that none is).
static LC4_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(LC4_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Reads a clause that is the wording of one ability of the table. An ability word before a dash is flavor and left out
/// ("Throw ... — ").
pub(super) fn lc4_whole_read(text: &str) -> Option<Ability> {
    let head = text.lines().next().unwrap_or_default();
    let text = text
        .get(crate::compile::classify::ability_word_len(head).unwrap_or(0)..)
        .unwrap_or(text);
    LC4_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(shape, _)| shape.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn lc4_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in LC4_SHAPES.iter() {
        if shape == ability && !texts.contains(text) {
            texts.push(text.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: every entry of the table parses, reads back from its own words and writes them back, the same ability with
    //! one part changed is not written as that wording, and a near miss of the words reads as nothing.
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::compile::read::render::written_variants;

    #[test]
    fn lc4_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(LC4_WHOLES).unwrap();
        assert_eq!(LC4_SHAPES.len(), entries.len());
        for (shape, text) in LC4_SHAPES.iter() {
            assert_eq!(lc4_whole_read(text).as_ref(), Some(shape), "{text}");
            assert!(
                written_variants(shape)
                    .iter()
                    .any(|written| super::super::tokens::same_written_text(written, text)),
                "{text} {:?}",
                written_variants(shape)
            );
        }
    }

    #[rstest::rstest]
    #[case("\"Teamwork\"", "\"Convoke\"")]
    #[case("\"Equipment\"", "\"Aura\"")]
    #[case("\"ManaValue\"", "\"Power\"")]
    #[case("\"max\":3", "\"max\":2")]
    #[case("\"sealed\":true", "\"sealed\":false")]
    #[case("\"to\":\"Hand\"", "\"to\":\"Graveyard\"")]
    #[case("\"to\":\"Graveyard\"", "\"to\":\"Hand\"")]
    #[case("\"Choose\":\"Color\"", "\"Choose\":\"CreatureType\"")]
    #[case("\"Spell\",", "\"Creature\",")]
    fn lc4_a_changed_part_is_not_written_whole(#[case] from: &str, #[case] to: &str) {
        for (shape, text) in LC4_SHAPES.iter() {
            let json = serde_json::to_string(shape).unwrap();
            if !json.contains(from) {
                continue;
            }
            let changed = json.replacen(from, to, 1);
            if let Ok(near) = serde_json::from_str::<Ability>(&changed) {
                assert!(
                    !written_variants(&near)
                        .iter()
                        .any(|written| super::super::tokens::same_written_text(written, text)),
                    "{changed}"
                );
            }
        }
    }

    #[rstest::rstest]
    #[case("a teamwork cost", "a convoke cost")]
    #[case("an Equipment", "an Aura")]
    #[case("mana value", "power")]
    #[case("three targets", "four targets")]
    #[case("face down", "face up")]
    #[case("discard your hand", "discard a card")]
    #[case("owner's graveyard", "owner's hand")]
    #[case("color word.", "creature type.")]
    #[case("spells and permanents", "permanents")]
    fn lc4_a_near_miss_reads_as_nothing(#[case] from: &str, #[case] to: &str) {
        for (_, text) in LC4_SHAPES.iter() {
            if text.contains(from) {
                let near = text.replacen(from, to, 1);
                assert_eq!(lc4_whole_read(&near), None, "{near}");
            }
        }
    }
}
