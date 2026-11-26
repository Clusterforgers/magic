//! p162 abilities (face-down piles and exiles, delay counters, lands of others tapped for mana, a land entering then a sacrifice, exact damage or life loss) whose wordings no other reader or writer joins: each entry of `p162_wholes.json` is an ability with
//! the words the card prints for it, read when a clause says exactly those words and written back from the ability.

use std::sync::LazyLock;

use super::tokens::same_text;
use crate::ability::Ability;

/// The entries, as JSON: a list of `[ability, wording]` pairs.
const P162_WHOLES: &str = include_str!("p162_wholes.json");

/// The entries parsed once; one that does not parse is dropped (a test holds that none is).
static P162_SHAPES: LazyLock<Vec<(Ability, String)>> = LazyLock::new(|| {
    serde_json::from_str::<Vec<(serde_json::Value, String)>>(P162_WHOLES)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(shape, text)| Some((serde_json::from_value(shape).ok()?, text)))
        .collect()
});

/// Reads a clause that is the wording of one ability of the table.
pub(super) fn p162_whole_read(text: &str) -> Option<Ability> {
    P162_SHAPES
        .iter()
        .find(|(_, wording)| same_text(wording, text))
        .map(|(shape, _)| shape.clone())
}

/// Adds the wording of an ability that one of the entries matches whole.
pub(super) fn p162_add_wholes(ability: &Ability, texts: &mut Vec<String>) {
    for (shape, text) in P162_SHAPES.iter() {
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
    fn p162_every_whole_parses_and_reads_back() {
        let entries: Vec<serde_json::Value> = serde_json::from_str(P162_WHOLES).unwrap();
        assert_eq!(P162_SHAPES.len(), entries.len());
        for (shape, text) in P162_SHAPES.iter() {
            assert_eq!(p162_whole_read(text).as_ref(), Some(shape), "{text}");
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
    #[case("\"amount\":1", "\"amount\":2")]
    #[case("\"R406Kept\"", "\"OnTop\"")]
    #[case("\"count\":3", "\"count\":2")]
    #[case("\"who\":\"Opponents\"", "\"who\":\"EachPlayer\"")]
    #[case("\"UntilEndOfTurn\"", "\"Forever\"")]
    #[case("\"AnyType\"", "\"AnyColor\"")]
    #[case("{\"Other\":\"delay\"}", "{\"Other\":\"time\"}")]
    fn p162_a_changed_part_is_not_written_whole(#[case] from: &str, #[case] to: &str) {
        let mut hit = false;
        for (shape, text) in P162_SHAPES.iter() {
            let json = serde_json::to_string(shape).unwrap();
            if !json.contains(from) {
                continue;
            }
            hit = true;
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
        assert!(hit, "no entry has {from}");
    }

    #[rstest::rstest]
    #[case("exactly 1 damage", "exactly 2 damage")]
    #[case("seven cards", "six cards")]
    #[case("top three cards", "top two cards")]
    #[case("at least as many lands", "more lands")]
    #[case("Until end of turn", "Until your next turn")]
    #[case("Mana of any type", "Mana of any color")]
    #[case("X delay counters", "X time counters")]
    fn p162_a_near_miss_reads_as_nothing(#[case] from: &str, #[case] to: &str) {
        assert!(P162_SHAPES.iter().any(|(_, text)| text.contains(from)));
        for (_, text) in P162_SHAPES.iter() {
            if text.contains(from) {
                let near = text.replacen(from, to, 1);
                assert_eq!(p162_whole_read(&near), None, "{near}");
            }
        }
    }
}
