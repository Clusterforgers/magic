//! The card sacrificed unless a payment is made: "sacrifice it unless you return an untapped Forest you control to its
//! owner's hand." The renderers name the card with "~".

use crate::ability::Ability;
use crate::cost::CostItem;
use crate::effect::Effect;
use crate::refs::Ref;

/// Adds the wordings that call the card "it" in a sacrifice that a payment avoids.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let avoided = ability.effects().iter().any(|effect| {
        matches!(
            effect,
            Effect::Pay { cost, unpaid, .. }
                if matches!(cost.0.as_slice(), [CostItem::ReturnToHand { .. }])
                    && matches!(unpaid.as_slice(), [Effect::Sacrifice { who: Ref::You, .. }])
        )
    });
    if !avoided {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("sacrifice ~ unless"))
        .map(|text| text.replacen("sacrifice ~ unless", "sacrifice it unless", 1))
        .collect();
    texts.extend(said);
}
