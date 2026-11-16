//! `Destination::R6502BattlefieldAs`: a card put onto the battlefield as an object with changes from the moment it
//! enters ("return it to the battlefield. It's an Aura enchantment with enchant creature you control and ..., and it
//! loses all other abilities"). The changes are continuous effects of the resolving spell or ability on the new object
//! (CR 400.7, 611.2c), proposed with the move as the counters it enters with are, and the card is judged with them as
//! it enters: an Aura enchants what it may enchant as it is (CR 303.4g), and the replacement step reads it as it is
//! (`entering_view`, CR 614.12).

use std::borrow::Cow;

use mtg_cards::modify::{Duration, Modification as Wanted};
use mtg_core::ids::EffectId;

use super::continuous::lower;
use crate::effects::{ContinuousEffect, Modification, Selector};
use crate::events::Event;
use crate::object::{GameObject, ObjectRef};
use crate::resolve::{Cx, ResolveError};
use crate::state::GameState;
use crate::zones::Zone;

/// The changes worked out as the move begins, the loss of all abilities first so what the same list gives stays (CR
/// 613.1f).
pub(super) fn lowered(cx: &Cx<'_>, becomes: &[Wanted]) -> Result<Vec<Modification>, ResolveError> {
    let (lost, kept): (Vec<&Wanted>, Vec<&Wanted>) = becomes.iter().partition(|wanted| {
        matches!(
            wanted,
            Wanted::LoseAbilities | Wanted::LoseAbilitiesExceptMana
        )
    });
    lost.into_iter()
        .chain(kept)
        .map(|wanted| lower(&cx.scope, wanted))
        .collect()
}

/// The effects on the object `object` becomes once it is on the battlefield, numbered from `made` on (each is counted
/// into it): they last as long as that object does.
pub(super) fn arriving(
    cx: &Cx<'_>,
    object: &GameObject,
    mods: &[Modification],
    made: &mut u64,
) -> Vec<Event> {
    let state = cx.state();
    let arrived = ObjectRef {
        id: object.id(),
        counter: object.counter() + 1,
    };
    mods.iter()
        .map(|modification| {
            let mut effect = ContinuousEffect::new(
                EffectId(state.next_effect + *made),
                Selector::Objects(vec![arrived]),
                modification.clone(),
                Duration::Forever,
                state.next_timestamp + *made,
            )
            .from_source(cx.scope.source)
            .begun(cx.controller(), state.turn().number());
            effect.entering = true;
            *made += 1;
            Event::EffectCreated {
                effect: Box::new(effect),
            }
        })
        .collect()
}

/// The state as the card `object` is about to enter: it already has the changes where it is, so what it may enchant and
/// what it is are read as it will be.
pub(super) fn as_it_enters<'a>(
    state: &'a GameState,
    object: &GameObject,
    mods: &[Modification],
) -> Cow<'a, GameState> {
    if mods.is_empty() {
        return Cow::Borrowed(state);
    }
    let mut view = state.clone();
    let here = object.reference();
    for (made, modification) in (0u64..).zip(mods) {
        let effect = ContinuousEffect::new(
            EffectId(state.next_effect + made),
            Selector::Objects(vec![here]),
            modification.clone(),
            Duration::Forever,
            state.next_timestamp + made,
        );
        view.effects.push(effect);
        view.layers.reset();
    }
    Cow::Owned(view)
}

/// The state the replacement step reads a batch against: a card the batch puts onto the battlefield with effects made
/// for its arrival (`ContinuousEffect::entering`) already has them where it is.
pub(crate) fn entering_view<'a>(state: &'a GameState, proposed: &[Event]) -> Cow<'a, GameState> {
    let arriving: Vec<ContinuousEffect> = proposed
        .iter()
        .filter_map(|event| match event {
            Event::EffectCreated { effect } if effect.entering => {
                let Selector::Objects(named) = &effect.selector else {
                    return None;
                };
                let here: Vec<ObjectRef> = named
                    .iter()
                    .filter_map(|arrived| {
                        let object = state.object(arrived.id)?;
                        (object.zone() != Zone::Battlefield
                            && object.counter() + 1 == arrived.counter)
                            .then(|| object.reference())
                    })
                    .collect();
                (!here.is_empty()).then(|| {
                    let mut now = (**effect).clone();
                    now.selector = Selector::Objects(here);
                    now.entering = false;
                    now
                })
            }
            _ => None,
        })
        .collect();
    if arriving.is_empty() {
        return Cow::Borrowed(state);
    }
    let mut view = state.clone();
    for effect in arriving {
        view.effects.push(effect);
        view.layers.reset();
    }
    Cow::Owned(view)
}
