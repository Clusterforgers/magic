//! The card doubles its own power: "Whenever another creature you control enters, double this creature's power until
//! end of turn." The renderers write the bonus as an amount: "~ gets +X/+0 until end of turn, where X is ~'s power".

use crate::ability::Ability;

/// The bonus as the renderers word it and as the oracle text does.
const BONUS: (&str, &str) = (
    ", ~ gets +X/+0 until end of turn, where X is ~'s power.",
    ", double ~'s power until end of turn.",
);

/// Adds the wording of an ability that gives the card a bonus to power of its own power.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    if !matches!(
        ability,
        Ability::Triggered { .. } | Ability::Activated { .. }
    ) {
        return;
    }
    let (said, wanted) = BONUS;
    let found: Vec<String> = texts
        .iter()
        .filter_map(|text| Some(format!("{}{wanted}", text.strip_suffix(said)?)))
        .collect();
    texts.extend(found);
}
