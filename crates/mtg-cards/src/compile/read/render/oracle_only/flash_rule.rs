//! The permission to cast spells as though they had flash: "You may cast spells as though they had flash." and "You may
//! cast artifact spells as though they had flash." The kind is a card type or its negation.

use super::super::super::selectors::type_name;
use crate::ability::Ability;
use crate::modify::{CastPermission, GameRule};
use crate::refs::{Pred, Ref};

/// The kind of spells a permission covers as the card words it in front of "spells".
fn kind(permission: &CastPermission) -> Option<String> {
    match permission.what.0.as_slice() {
        [] => Some(String::new()),
        [Pred::Type(card_type)] => Some(format!("{} ", type_name(*card_type))),
        [Pred::Not(inner)] => match inner.as_ref() {
            Pred::Type(card_type) => Some(format!("non{} ", type_name(*card_type))),
            _ => None,
        },
        _ => None,
    }
}

/// Adds the wording of a rule that lets the controller cast a kind of spell with flash.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Rule {
        rule: GameRule::MayCast(permission),
        condition: None,
        ..
    } = ability
    else {
        return;
    };
    let plain = permission.who == Ref::You
        && permission.flash
        && !permission.free
        && !permission.play
        && permission.from.is_none()
        && permission.only.is_none();
    if let Some(kind) = kind(permission).filter(|_| plain) {
        texts.push(format!(
            "You may cast {kind}spells as though they had flash."
        ));
    }
}
