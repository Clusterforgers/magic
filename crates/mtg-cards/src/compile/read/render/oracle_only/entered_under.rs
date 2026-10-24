//! An intervening "if" that a creature entered the battlefield under a player's control this turn: "At the beginning of
//! your end step, if a creature entered the battlefield under your control this turn, ..." The renderers write "if you
//! control a creature that entered the battlefield this turn" for yourself and have no wording for an opponent.

use super::super::variants;
use crate::ability::Ability;
use crate::refs::{Pred, Ref};
use crate::value::{Amount, Cmp, Condition};
use mtg_core::card::CardType;

/// Whose control the creature entered under, as the card words it.
fn whose(who: &Ref) -> Option<&'static str> {
    match who {
        Ref::You => Some("your"),
        Ref::Opponents => Some("an opponent's"),
        _ => None,
    }
}

/// The words of the condition when it is "a creature entered the battlefield under X control this turn".
fn entered(condition: &Condition) -> Option<String> {
    let Condition::Compare {
        left: Amount::Count(selector),
        op: Cmp::Ge,
        right: Amount::Fixed(1),
    } = condition
    else {
        return None;
    };
    let [
        Pred::Type(CardType::Creature),
        Pred::ControlledBy(who),
        Pred::EnteredThisTurn,
    ] = selector.0.as_slice()
    else {
        return None;
    };
    Some(format!(
        "a creature entered the battlefield under {} control this turn",
        whose(who)?
    ))
}

/// Adds the wordings of a trigger with that intervening "if" after its event.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered {
        intervening_if: Some(condition),
        ..
    } = ability
    else {
        return;
    };
    let Some(words) = entered(condition) else {
        return;
    };
    let mut plain = ability.clone();
    if let Ability::Triggered { intervening_if, .. } = &mut plain {
        *intervening_if = None;
    }
    let said: Vec<String> = variants(&plain)
        .iter()
        .filter_map(|text| text.split_once(", "))
        .map(|(event, rest)| format!("{event}, if {words}, {rest}"))
        .collect();
    texts.extend(said);
}
