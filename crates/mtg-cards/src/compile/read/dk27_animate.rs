//! Animate Dead's trigger: "When ~ enters, if it's on the battlefield, it loses "enchant creature card in a graveyard"
//! and gains "enchant creature put onto the battlefield with ~." Return enchanted creature card to the battlefield under
//! your control and attach ~ to it. When ~ leaves the battlefield, that creature's controller sacrifices it." reads as
//! the return of the enchanted card, the attachment to it and a delayed sacrifice of it. The change of enchant ability
//! is the engine's rule for an Aura that enchants a card in a graveyard: on the battlefield it enchants the creature
//! it is attached to (`attach::may_enchant`).

use super::tokens::same_text;
use crate::ability::Ability;

/// The words of the trigger, with the card's name as `~`.
const TEXT: &str = "When ~ enters, if it's on the battlefield, it loses \"enchant creature card in a graveyard\" and gains \"enchant creature put onto the battlefield with ~.\" Return enchanted creature card to the battlefield under your control and attach ~ to it. When ~ leaves the battlefield, that creature's controller sacrifices it.";

/// The ability the words stand for: the enchanted card returns under your control (bound as `returned`), the source
/// is attached to it, and a delayed trigger sacrifices it when the source leaves the battlefield.
const RULES: &str = r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"to":"Battlefield"}},"if":{"Matches":{"subject":"This","selector":[{"In":"Battlefield"}]}},"effects":[{"Move":{"what":"Attached","to":{"Battlefield":{"under":"You"}},"as":"returned"}},{"Attach":{"what":"This","to":{"Bound":"returned"}}},{"DelayedTrigger":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield"}},"effects":[{"Sacrifice":{"who":{"ControllerOf":{"Bound":"returned"}},"filter":[{"Is":{"Bound":"returned"}}]}}]}}]}}"#;

fn rules() -> Option<&'static Ability> {
    static PARSED: std::sync::OnceLock<Option<Ability>> = std::sync::OnceLock::new();
    PARSED
        .get_or_init(|| serde_json::from_str(RULES).ok())
        .as_ref()
}

/// The ability, when the clause is the trigger's words.
pub(super) fn dk27_animate_read(text: &str) -> Option<Ability> {
    (text.contains("put onto the battlefield with") && same_text(text, TEXT))
        .then(rules)
        .flatten()
        .cloned()
}

/// The words of the ability, when it is the trigger [`dk27_animate_read`] reads.
pub(super) fn dk27_animate_variants(ability: &Ability) -> Option<Vec<String>> {
    (Some(ability) == rules()).then(|| vec![TEXT.to_owned()])
}
