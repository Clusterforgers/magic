//! A card that returns itself from its graveyard after a payment: "Whenever a Dragon you control enters, you may pay
//! {R}. If you do, return this card from your graveyard to your hand." The renderers write "return ~ to its owner's
//! hand".

use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::{Ref, ZoneKind};

/// Whether the effects move the source to its owner's hand, directly or as what a payment buys.
fn returns_itself(effects: &[Effect]) -> bool {
    effects.iter().any(|effect| match effect {
        Effect::Move {
            what: Ref::This,
            to: Destination::Hand,
        } => true,
        Effect::Pay { paid, .. } => returns_itself(paid),
        _ => false,
    })
}

/// Adds the wordings of an ability that works from the graveyard and returns its card to the hand.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        from: ZoneKind::Graveyard,
        effects,
        ..
    } = ability
    else {
        return;
    };
    if !returns_itself(effects) {
        return;
    }
    let (from, into) = (
        "return ~ to its owner's hand",
        "return this card from your graveyard to your hand",
    );
    let said: Vec<String> = texts
        .iter()
        .filter(|text| text.contains(from))
        .map(|text| text.replacen(from, into, 1))
        .collect();
    texts.extend(said);
}
