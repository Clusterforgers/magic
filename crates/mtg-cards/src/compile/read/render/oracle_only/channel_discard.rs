//! An ability a card activates from the hand by discarding itself: "{U}, Discard this card: Draw a card." The renderers
//! have no wording for the cost.

use super::super::variants;
use crate::ability::Ability;
use crate::cost::CostItem;
use crate::refs::ZoneKind;

/// Adds the wordings of an activation from the hand whose cost includes discarding the card.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Activated {
        cost,
        from: ZoneKind::Hand,
        ..
    } = ability
    else {
        return;
    };
    if !cost.0.contains(&CostItem::DiscardSelf) || cost.0.len() < 2 {
        return;
    }
    let mut rest = ability.clone();
    if let Ability::Activated { cost, from, .. } = &mut rest {
        cost.0.retain(|item| *item != CostItem::DiscardSelf);
        *from = ZoneKind::Battlefield;
    }
    for text in variants(&rest) {
        if let Some((paid, effect)) = text.split_once(": ") {
            texts.push(format!("{paid}, Discard this card: {effect}"));
        }
    }
}
