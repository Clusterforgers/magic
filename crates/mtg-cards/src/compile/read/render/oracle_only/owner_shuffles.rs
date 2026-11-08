//! The owner of a target shuffles it into their library: "Target creature's owner shuffles it into their library." and
//! "The owner of target nonland permanent shuffles it into their library, then draws two cards."

use super::super::super::amounts::cards_text;
use super::super::super::selectors::spec_text;
use super::standin::with_effects;
use crate::ability::Ability;
use crate::effect::{Destination, Effect, Place};
use crate::refs::{Pred, Ref, TargetSpec, Targetable, ZoneKind};
use crate::value::Amount;

/// Adds the wordings of an ability whose one target is shuffled into its owner's library, the owner drawing after it.
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
    let Some(target) = spec_text(spec) else {
        return;
    };
    let shuffled = Effect::Move {
        what: Ref::Target(0),
        to: Destination::Library {
            place: Place::Shuffled,
            order: None,
        },
        bind: None,
    };
    let sentence = match effects.as_slice() {
        // "You may shuffle up to three target cards from your graveyard into your library": none may be chosen.
        [first] if *first == shuffled && g9d_your_graveyard_up_to(spec) => {
            format!("you may shuffle {target} into your library")
        }
        [first] if *first == shuffled => format!("{target}'s owner shuffles it into their library"),
        [
            first,
            Effect::Draw {
                who: Ref::OwnerOf(owner),
                count,
                ..
            },
        ] if *first == shuffled && **owner == Ref::Target(0) => {
            let Some(cards) = cards_text(count) else {
                return;
            };
            format!("the owner of {target} shuffles it into their library, then draws {cards}")
        }
        _ => return,
    };
    texts.extend(with_effects(ability, &sentence));
}

/// Whether the target is any number up to some, none allowed, of cards in your own graveyard (so your library is theirs).
fn g9d_your_graveyard_up_to(spec: &TargetSpec) -> bool {
    let Targetable::Object(selector) = &spec.what else {
        return false;
    };
    spec.bounds.min == Amount::Fixed(0)
        && spec.bounds.max.is_some()
        && selector.0.contains(&Pred::OwnedBy(Ref::You))
        && selector.0.contains(&Pred::In(ZoneKind::Graveyard))
}
