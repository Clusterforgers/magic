//! The top cards looked at and sorted: "Look at the top two cards of your library. Put one of them into your hand and the
//! other on the bottom of your library." The renderers have no wording for the choice of one card and the place of the
//! rest.

use super::super::variants;
use crate::ability::Ability;
use crate::effect::{Destination, Effect, Place};
use crate::refs::Ref;

/// The effects of an ability and a copy of it with other effects, for the kinds that have effects.
fn with_effects(ability: &Ability, effects: Vec<Effect>) -> Option<Ability> {
    let mut copy = ability.clone();
    match &mut copy {
        Ability::Spell { effects: steps, .. } | Ability::Triggered { effects: steps, .. } => {
            *steps = effects;
            Some(copy)
        }
        _ => None,
    }
}

/// "Put one of them into your hand and the other on the bottom of your library." (two cards), and the same for three
/// cards with one more place.
fn sentence(effects: &[Effect]) -> Option<&'static str> {
    let bottom = Destination::Library {
        place: Place::Bottom,
        order: None,
    };
    let top = Destination::Library {
        place: Place::Top,
        order: None,
    };
    match effects {
        [
            Effect::LookAtTop { bind: top_name, .. },
            Effect::Choose {
                who: Ref::You,
                from: Ref::Bound(looked),
                bind: kept,
                rest_as: Some(rest),
                ..
            },
            Effect::Move {
                what: Ref::Bound(moved),
                to: Destination::Hand,
            },
            Effect::Move {
                what: Ref::Bound(others),
                to,
            },
        ] if looked == top_name && moved == kept && others == rest && *to == bottom => {
            Some(" Put one of them into your hand and the other on the bottom of your library.")
        }
        [
            Effect::LookAtTop { bind: top_name, .. },
            Effect::Choose {
                who: Ref::You,
                from: Ref::Bound(looked),
                bind: kept,
                rest_as: Some(rest),
                ..
            },
            Effect::Move {
                what: Ref::Bound(moved),
                to: Destination::Hand,
            },
            Effect::Choose {
                who: Ref::You,
                from: Ref::Bound(left),
                bind: put_on_top,
                rest_as: Some(put_on_bottom),
                ..
            },
            Effect::Move {
                what: Ref::Bound(on_top),
                to: first,
            },
            Effect::Move {
                what: Ref::Bound(on_bottom),
                to: second,
            },
        ] if looked == top_name
            && moved == kept
            && left == rest
            && on_top == put_on_top
            && on_bottom == put_on_bottom
            && *first == top
            && *second == bottom =>
        {
            Some(
                " Put one of those cards into your hand, one on top of your library, and one on the bottom of your library.",
            )
        }
        _ => None,
    }
}

/// Adds the wordings of a look at the top cards, one kept in the hand and the rest put in order.
pub(super) fn add(ability: &Ability, texts: &mut Vec<String>) {
    let effects = ability.effects();
    let Some(put) = sentence(effects) else {
        return;
    };
    let Some(look) = effects.first().cloned() else {
        return;
    };
    let Some(alone) = with_effects(ability, vec![look]) else {
        return;
    };
    let said: Vec<String> = variants(&alone)
        .iter()
        .map(|text| format!("{text}{put}"))
        .collect();
    texts.extend(said);
}
