//! A loss of life by the size of the target: "Destroy target creature. Its controller loses life equal to its power plus
//! its toughness."

use super::twin_variants;
use crate::ability::Ability;

/// The sum of the power and the toughness of the target, and the 1 life the renderers write instead.
const SUM: &str = r#"{"Sum":[{"Stat":{"of":{"Target":0},"stat":"Power"}},{"Stat":{"of":{"Target":0},"stat":"Toughness"}}]}"#;

/// Adds the wordings of a spell that makes a player lose life equal to the power plus the toughness of its target.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let said: Vec<String> = twin_variants(ability, SUM, "1")
        .iter()
        .filter(|text| text.contains(" loses 1 life."))
        .map(|text| {
            text.replacen(
                " loses 1 life.",
                " loses life equal to its power plus its toughness.",
                1,
            )
        })
        .collect();
    texts.extend(said);
}
