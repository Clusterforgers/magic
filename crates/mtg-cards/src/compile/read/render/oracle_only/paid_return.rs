//! A card in its graveyard that returns itself for a payment: "Whenever you cast a creature spell, you may pay {G}{G}. If
//! you do, return this card from your graveyard to your hand."

use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::{Ref, ZoneKind};

/// Adds the wordings of a trigger of a card in its graveyard that pays to return it to its owner's hand.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        from: ZoneKind::Graveyard,
        effects,
        ..
    } = ability
    else {
        return;
    };
    let [Effect::Pay { paid, unpaid, .. }] = effects.as_slice() else {
        return;
    };
    let returns = matches!(
        paid.as_slice(),
        [Effect::Move {
            what: Ref::This,
            to: Destination::Hand
        }]
    );
    if !returns || !unpaid.is_empty() {
        return;
    }
    let said: Vec<String> = texts
        .iter()
        .map(|text| {
            text.replacen(
                "return ~ to its owner's hand",
                "return ~ from your graveyard to your hand",
                1,
            )
        })
        .collect();
    texts.extend(said);
}
