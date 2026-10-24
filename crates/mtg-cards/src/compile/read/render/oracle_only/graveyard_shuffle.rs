//! Cards of the player's graveyard shuffled into their library: "Shuffle any number of target cards from your graveyard
//! into your library."

use super::super::super::amounts::count_word;
use super::super::super::selectors::{Form, phrase_text};
use super::standin::with_effects;
use crate::ability::Ability;
use crate::effect::{Destination, Effect, Place};
use crate::refs::{Pred, Ref, Selector, Targetable, ZoneKind};
use crate::value::Amount;

/// Adds the wordings of an ability that shuffles the cards of one target from the graveyard of its controller into
/// their library.
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
    let [spec] = targets.as_slice() else {
        return;
    };
    let shuffled = Effect::Move {
        what: Ref::Target(0),
        to: Destination::Library {
            place: Place::Shuffled,
            order: None,
        },
    };
    if effects.as_slice() != [shuffled] {
        return;
    }
    let Targetable::Object(Selector(preds)) = &spec.what else {
        return;
    };
    if !preds.contains(&Pred::In(ZoneKind::Graveyard)) || !preds.contains(&Pred::OwnedBy(Ref::You))
    {
        return;
    }
    let Some(phrase) = phrase_text(&Selector(preds.clone()), Form::Targets) else {
        return;
    };
    let opening = match (spec.bounds.fixed_min(), &spec.bounds.max) {
        (Some(0), None) => "any number of".to_owned(),
        (Some(0), Some(Amount::Fixed(most))) => {
            let Some(word) = u32::try_from(*most).ok().and_then(count_word) else {
                return;
            };
            format!("up to {word}")
        }
        _ => return,
    };
    let sentence = format!("shuffle {opening} target {phrase} into your library");
    texts.extend(with_effects(ability, &sentence));
}
