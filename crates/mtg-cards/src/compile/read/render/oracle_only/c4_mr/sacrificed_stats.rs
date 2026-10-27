//! Cards and life by the characteristics of the creature sacrificed as a cost: "You draw cards equal to the sacrificed
//! creature's power, then you gain life equal to its toughness."

use crate::ability::Ability;

/// The draw by the power and the gain by the toughness of the sacrificed creature, in the order a card says them.
const DRAW_THEN_GAIN: &str = r#""effects":[{"Draw":{"count":{"Stat":{"of":"Paid","stat":"Power"}}}},{"GainLife":{"amount":{"Stat":{"of":"Paid","stat":"Toughness"}}}}]"#;

/// Adds the wording of a spell that draws by the power of the sacrificed creature and gains life by its toughness.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let json = serde_json::to_string(ability).unwrap_or_default();
    if matches!(ability, Ability::Spell { .. }) && json.contains(DRAW_THEN_GAIN) {
        texts.push(
            "You draw cards equal to the sacrificed creature's power, then you gain life equal to its toughness."
                .to_owned(),
        );
    }
}
