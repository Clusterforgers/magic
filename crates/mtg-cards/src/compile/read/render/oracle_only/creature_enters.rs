//! A creature that enters under your control, the card among them: "Whenever this creature or another creature you
//! control enters, you gain life equal to that creature's toughness." The renderers write the group and call the creature
//! "that permanent".

use crate::ability::Ability;
use crate::refs::{Pred, Ref};
use crate::trigger::TriggerPattern;
use mtg_core::card::CardType;

/// Adds the wordings of a trigger on a creature you control entering.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        when: TriggerPattern::ZoneChange { what, .. },
        ..
    } = ability
    else {
        return;
    };
    if what.0 != [Pred::Type(CardType::Creature), Pred::ControlledBy(Ref::You)] {
        return;
    }
    let (group, card) = (
        "Whenever a creature you control enters",
        "Whenever this creature or another creature you control enters",
    );
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains("that permanent's "))
        .flat_map(|text| {
            let named = text.replace("that permanent's ", "that creature's ");
            let with_card = named.replacen(group, card, 1);
            [named, with_card]
        })
        .collect();
    texts.extend(said);
}
