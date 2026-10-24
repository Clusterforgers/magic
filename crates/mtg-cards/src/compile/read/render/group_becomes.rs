//! A group that becomes creatures until end of turn: "Artifacts you control become artifact creatures with base power
//! and toughness 5/5 until end of turn."

use super::super::selectors::type_name;
use crate::modify::{Duration, Modification};
use crate::refs::{Pred, Selector};
use crate::value::Amount;
use mtg_core::card::CardType;

/// The sentence of a group (`subject`, "artifacts you control") that gains the creature type and a base size; `None`
/// for any other change or when the group has no single type to keep.
pub(super) fn text(
    subject: &str,
    group: &Selector,
    mods: &[Modification],
    duration: Duration,
) -> Option<String> {
    (duration == Duration::UntilEndOfTurn).then_some(())?;
    let ([
        Modification::AddType(CardType::Creature),
        Modification::SetPT {
            power: Amount::Fixed(power),
            toughness: Amount::Fixed(toughness),
        },
    ]
    | [
        Modification::SetPT {
            power: Amount::Fixed(power),
            toughness: Amount::Fixed(toughness),
        },
        Modification::AddType(CardType::Creature),
    ]) = mods
    else {
        return None;
    };
    let kept: Vec<CardType> = group
        .0
        .iter()
        .filter_map(|pred| match pred {
            Pred::Type(card_type) if *card_type != CardType::Creature => Some(*card_type),
            _ => None,
        })
        .collect();
    let noun = match kept.as_slice() {
        [] => "creatures".to_owned(),
        [kept] => format!("{} creatures", type_name(*kept)),
        _ => return None,
    };
    Some(format!(
        "{subject} become {noun} with base power and toughness {power}/{toughness} until end of turn"
    ))
}
