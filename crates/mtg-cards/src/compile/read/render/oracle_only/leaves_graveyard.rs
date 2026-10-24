//! A permanent that is not a creature goes to a graveyard: "Whenever an enchantment you control is put into a graveyard
//! from the battlefield, ..." The renderers word every such event "dies", which only a creature does.

use crate::ability::Ability;
use crate::refs::{Pred, ZoneKind};
use crate::trigger::TriggerPattern;
use mtg_core::card::CardType;

/// Adds the wordings that put a permanent that is not the card itself or a creature "into a graveyard from the
/// battlefield".
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when:
            TriggerPattern::ZoneChange {
                what,
                from: Some(ZoneKind::Battlefield),
                to: Some(ZoneKind::Graveyard),
            },
        ..
    } = ability
    else {
        return;
    };
    if what.0.contains(&Pred::This) || what.0.contains(&Pred::Type(CardType::Creature)) {
        return;
    }
    let found: Vec<String> = texts
        .iter()
        .filter(|text| text.starts_with("Whenever ") && text.contains(" dies, "))
        .map(|text| {
            text.replacen(
                " dies, ",
                " is put into a graveyard from the battlefield, ",
                1,
            )
        })
        .collect();
    texts.extend(found);
}
