//! A look at the top cards that keeps some in any order: "Look at the top seven cards of your library. Put two of them
//! into your hand and the rest on the bottom of your library in any order." The renderers write the look alone, which
//! is the twin with only the first step; the choice and the two moves follow it as one sentence.

use super::c5_af_variants;
use crate::ability::Ability;
use crate::compile::read::amounts::count_word;
use crate::effect::{Destination, Effect, Order, Place};
use crate::refs::{Bounds, Ref};
use crate::value::Amount;

/// How many cards the choice keeps, as a word; `None` unless the steps after the look choose from the looked-at cards,
/// put the choice into the hand and the rest on the bottom in an order the player chooses.
fn c5_af_kept(effects: &[Effect]) -> Option<&'static str> {
    let [
        Effect::LookAtTop { bind: looked, .. },
        Effect::Choose {
            from: Ref::Bound(from),
            count,
            bind: chosen,
            rest_as: Some(rest),
            ..
        },
        Effect::Move {
            what: Ref::Bound(kept),
            to: Destination::Hand,
        },
        Effect::Move {
            what: Ref::Bound(others),
            to:
                Destination::Library {
                    place: Place::Bottom,
                    order: Some(Order::Chosen),
                },
        },
    ] = effects
    else {
        return None;
    };
    if from != looked || kept != chosen || others != rest {
        return None;
    }
    let number = match count {
        Bounds {
            min: Amount::Fixed(min),
            max: Some(Amount::Fixed(max)),
        } if min == max => *max,
        _ => return None,
    };
    count_word(u32::try_from(number).ok()?)
}

/// Adds the wording of a look at the top cards that keeps some and puts the rest on the bottom in any order.
pub(super) fn c5_af_add(ability: &Ability, texts: &mut Vec<String>) {
    let effects = match ability {
        Ability::Spell { effects, .. } | Ability::Triggered { effects, .. } => effects,
        _ => return,
    };
    let (Some(kept), Some(look)) = (c5_af_kept(effects), effects.first()) else {
        return;
    };
    let mut twin = ability.clone();
    if let Ability::Spell { effects, .. } | Ability::Triggered { effects, .. } = &mut twin {
        *effects = vec![look.clone()];
    }
    let card = format!(
        " Put {kept} of them into your hand and the rest on the bottom of your library in any order."
    );
    texts.extend(
        c5_af_variants(&twin)
            .iter()
            .filter(|text| text.ends_with(" cards of your library."))
            .map(|text| format!("{text}{card}")),
    );
}
