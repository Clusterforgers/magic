//! The permission to cast spells as though they had flash: "You may cast historic spells as though they had flash.",
//! "Any player may cast Sliver spells as though they had flash." and "As long as ~ is tapped, you may cast spells as
//! though they had flash."

use super::super::capitalize;
use crate::ability::Ability;
use crate::modify::GameRule;
use crate::refs::{Pred, Ref, Selector};
use crate::value::Condition;
use mtg_core::card::{CardType, Subtype, Supertype};

/// Adds the wording of a rule that lets players cast a kind of spell as though it had flash.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Rule {
        rule: GameRule::MayCast(permission),
        condition,
        ..
    } = ability
    else {
        return;
    };
    if permission.from.is_some() || permission.free || !permission.flash || permission.play {
        return;
    }
    let who = match permission.who {
        Ref::You => "you",
        Ref::EachPlayer => "any player",
        _ => return,
    };
    let (Some(kind), None) = (kind(&permission.what), &permission.only) else {
        return;
    };
    let sentence = format!("{who} may cast {kind} as though they had flash.");
    match condition {
        None => texts.push(capitalize(&sentence)),
        Some(Condition::Matches {
            subject: Ref::This,
            selector,
        }) if selector.0 == [Pred::Tapped] => {
            texts.push(format!("As long as ~ is tapped, {sentence}"));
        }
        Some(_) => {}
    }
}

/// The spells the permission covers: all, those of a subtype, or the historic ones.
fn kind(what: &Selector) -> Option<String> {
    match what.0.as_slice() {
        [] => Some("spells".to_owned()),
        [Pred::Subtype(Subtype(name))] => Some(format!("{name} spells")),
        [Pred::AnyOf(kinds)] if historic(kinds) => Some("historic spells".to_owned()),
        _ => None,
    }
}

/// Artifacts, legendary permanents and Sagas.
fn historic(kinds: &[Pred]) -> bool {
    kinds
        == [
            Pred::Type(CardType::Artifact),
            Pred::Supertype(Supertype::Legendary),
            Pred::Subtype(Subtype("Saga".to_owned())),
        ]
}
