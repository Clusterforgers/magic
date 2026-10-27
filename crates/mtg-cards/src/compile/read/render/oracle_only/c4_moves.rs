//! A choice of cards followed by their move, and a search whose card has a basic land type: "Return a creature you control
//! to its owner's hand, then destroy all creatures.", "~ deals X damage to any target. Put up to X land cards from your
//! hand and/or graveyard onto the battlefield tapped." and "search your library for a land card with a basic land type,
//! reveal it, put it into your hand, then shuffle."

use super::super::super::selectors::{Form, phrase_text, spec_text};
use super::super::super::triggers::article;
use super::standin::with_effects;
use crate::ability::Ability;
use crate::effect::{Destination, Effect};
use crate::refs::{Bounds, Pred, Ref, Selector, ZoneKind};
use crate::value::Amount;
use mtg_core::card::{CardType, Subtype};

const BASIC_TYPES: [&str; 5] = ["Plains", "Island", "Swamp", "Mountain", "Forest"];

/// The selector is a land with any one of the five basic land types.
fn basic_typed_land(selector: &Selector) -> bool {
    let [Pred::Type(CardType::Land), Pred::AnyOf(types)] = selector.0.as_slice() else {
        return false;
    };
    let named: Vec<&str> = types
        .iter()
        .filter_map(|pred| match pred {
            Pred::Subtype(Subtype(name)) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    named.len() == types.len() && BASIC_TYPES.iter().all(|kind| named.contains(kind))
}

/// Adds the wordings of the moves this module knows.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let (Ability::Spell { targets, effects }
    | Ability::Activated {
        targets, effects, ..
    }
    | Ability::Triggered {
        targets, effects, ..
    }) = ability
    else {
        return;
    };
    let sentence = match effects.as_slice() {
        [
            Effect::Choose {
                who: Ref::You,
                from: Ref::Each(chosen),
                count,
                bind,
                rest_as: None,
            },
            Effect::Move {
                what: Ref::Bound(moved),
                to: Destination::Hand,
            },
            Effect::Destroy {
                what: Ref::Each(destroyed),
                ..
            },
        ] if count.is_one() && moved == bind && targets.is_empty() => {
            phrase_text(chosen, Form::Each).zip(phrase_text(destroyed, Form::All)).map(
                |(chosen, destroyed)| {
                    format!("return {} {chosen} to its owner's hand, then destroy all {destroyed}", article(&chosen))
                },
            )
        }
        [
            Effect::DealDamage {
                source: None,
                to: Ref::Target(0),
                amount: Amount::X,
            },
            Effect::Choose {
                who: Ref::You,
                from: Ref::Each(lands),
                count:
                    Bounds {
                        min: Amount::Fixed(0),
                        max: Some(Amount::X),
                    },
                bind,
                rest_as: None,
            },
            Effect::Move {
                what: Ref::Bound(moved),
                to: Destination::Battlefield { tapped: true, .. },
            },
        ] if moved == bind && is_own_lands_in_hand_and_graveyard(lands) => targets
            .first()
            .and_then(spec_text)
            .map(|target| {
                format!(
                    "~ deals X damage to {target}. Put up to X land cards from your hand and/or graveyard onto the battlefield tapped"
                )
            }),
        _ => None,
    };
    if let Some(sentence) = sentence {
        texts.extend(with_effects(ability, &sentence));
    }
    searched_basic_typed(ability, texts);
}

/// "land cards" of the player, in a hand or a graveyard.
fn is_own_lands_in_hand_and_graveyard(selector: &Selector) -> bool {
    matches!(
        selector.0.as_slice(),
        [
            Pred::Type(CardType::Land),
            Pred::OwnedBy(Ref::You),
            Pred::AnyOf(zones)
        ] if zones.as_slice() == [Pred::In(ZoneKind::Hand), Pred::In(ZoneKind::Graveyard)]
    )
}

/// A search for a land card with a basic land type, revealed and put into the hand: the effects stand in for one sentence.
fn searched_basic_typed(ability: &Ability, texts: &mut Vec<String>) {
    let Ability::Triggered { effects, .. } = ability else {
        return;
    };
    let [
        Effect::Search {
            filter,
            reveal: true,
            bind,
            ..
        },
        Effect::Move {
            what: Ref::Bound(moved),
            to: Destination::Hand,
        },
        Effect::Shuffle { who: Ref::You },
    ] = effects.as_slice()
    else {
        return;
    };
    if moved == bind && basic_typed_land(filter) {
        texts.extend(with_effects(
            ability,
            "search your library for a land card with a basic land type, reveal it, put it into your hand, then shuffle",
        ));
    }
}
