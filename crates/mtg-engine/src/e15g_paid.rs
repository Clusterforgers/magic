//! Paying to end an effect: "You may pay {W} to end this effect." Ending it is a special action of the effect's
//! controller (CR 116.2c): any time they have priority, it does not use the stack, and the effect ends at once.

use mtg_cards::modify::Duration;
use mtg_core::ids::{EffectId, ObjectId, PlayerId};
use mtg_core::mana::ManaCost;

use crate::decision::PaidEnd;
use crate::effects::{ContinuousEffect, Selector};
use crate::events::Event;
use crate::mana::{ManaOption, ManaPool, can_afford};
use crate::mana_takes::for_card;
use crate::spend::Spend;
use crate::state::GameState;

/// The mana that ends the effect, for an effect that lasts until it is paid.
fn price(effect: &ContinuousEffect) -> Option<&ManaCost> {
    match &effect.duration {
        Duration::UntilPaid(cost) => Some(cost.rest()),
        _ => None,
    }
}

/// Whether two effects are parts of one effect: one resolution made them, for the same objects and as long.
fn same_effect(one: &ContinuousEffect, other: &ContinuousEffect) -> bool {
    one.source == other.source
        && one.controller == other.controller
        && one.turn == other.turn
        && one.duration == other.duration
        && one.selector == other.selector
}

/// The first object the effect names that is still that object.
fn named(state: &GameState, effect: &ContinuousEffect) -> Option<ObjectId> {
    let Selector::Objects(named) = &effect.selector else {
        return None;
    };
    named
        .iter()
        .find(|r| state.object(r.id).is_some_and(|o| o.counter() == r.counter))
        .map(|r| r.id)
}

/// The effects `player` may end now by paying, each named by its first part, with the object it changes and the price,
/// when the mana they could make pays it.
pub(crate) fn endable(
    state: &GameState,
    player: PlayerId,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
) -> Vec<PaidEnd> {
    let mut found: Vec<PaidEnd> = Vec::new();
    for effect in &state.effects {
        let Some(cost) = price(effect).filter(|_| effect.controller == player) else {
            continue;
        };
        let first = state
            .effects
            .iter()
            .find(|other| same_effect(effect, other))
            .map_or(effect.id, |other| other.id);
        if first != effect.id {
            continue;
        }
        let Some(object) = named(state, effect) else {
            continue;
        };
        if can_afford(
            cost,
            pool,
            &for_card(state, sources, object),
            life,
            &Spend::other(),
        ) {
            found.push(PaidEnd {
                effect: effect.id,
                object,
                cost: cost.clone(),
            });
        }
    }
    found
}

/// The price and the object of the effect `id` names, while it lasts until paid.
pub(crate) fn offer(state: &GameState, id: EffectId) -> Option<(ObjectId, ManaCost)> {
    let effect = state.effects.iter().find(|effect| effect.id == id)?;
    Some((named(state, effect)?, price(effect)?.clone()))
}

/// The events of ending the effect `id` for `player`: every part of it expires. None when it is not theirs to end.
pub(crate) fn events(state: &GameState, player: PlayerId, id: EffectId) -> Option<Vec<Event>> {
    let effect = state.effects.iter().find(|effect| effect.id == id)?;
    price(effect).filter(|_| effect.controller == player)?;
    Some(
        state
            .effects
            .iter()
            .filter(|other| same_effect(effect, other))
            .map(|other| Event::EffectExpired { id: other.id })
            .collect(),
    )
}
