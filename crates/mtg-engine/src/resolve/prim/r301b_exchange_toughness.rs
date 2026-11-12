//! `Effect::ExchangeLifeWithToughness`: a player's life total and a creature's toughness are exchanged (CR 701.12c). The
//! player gains or loses the difference, so triggers and replacements see it, and the creature's toughness becomes the
//! former life total in layer 7b for good (CR 613.4b). Nothing is exchanged when the creature is gone or the life total
//! can't change that way.

use mtg_cards::effect::Effect;
use mtg_core::ids::EffectId;

use super::fe1n_life::{blocked, move_to};
use super::wrong_effect;
use crate::effects::{ContinuousEffect, Modification, Selector};
use crate::events::Event;
use crate::object::GameObject;
use crate::resolve::{Cx, ResolveError, Run};
use crate::zones::Zone;

/// The first player `who` names who is in the game exchanges life with the first creature `what` names on the battlefield.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::ExchangeLifeWithToughness { who, what } = effect else {
        return Err(wrong_effect("ExchangeLifeWithToughness"));
    };
    let state = cx.state();
    let nothing = || Ok(Run::events(Vec::new()));
    let Some((player, life)) = cx.scope.players(who)?.into_iter().find_map(|player| {
        state
            .player(player)
            .filter(|_| state.is_alive(player))
            .map(|held| (player, i64::from(held.life())))
    }) else {
        return nothing();
    };
    let Some(creature) = cx
        .scope
        .live_objects(what)?
        .into_iter()
        .find(|object| object.zone() == Zone::Battlefield)
        .map(GameObject::reference)
    else {
        return nothing();
    };
    let Some(toughness) = crate::characteristics::characteristics(state, creature.id)
        .and_then(|chars| chars.toughness)
        .map(i64::from)
    else {
        return nothing();
    };
    if blocked(cx, player, life, toughness) {
        return nothing();
    }
    let Ok(former) = i32::try_from(life) else {
        return nothing();
    };
    let mut events: Vec<Event> = move_to(cx, player, life, toughness).into_iter().collect();
    let duration = mtg_cards::modify::Duration::Forever;
    events.push(Event::EffectCreated {
        effect: Box::new(
            ContinuousEffect::new(
                EffectId(state.next_effect),
                Selector::Objects(vec![creature]),
                Modification::SetToughness(former),
                duration.clone(),
                state.next_timestamp,
            )
            .from_source(cx.scope.source)
            .begun(
                cx.controller(),
                crate::effects::began_turn(state, &duration, cx.controller()),
            ),
        ),
    });
    Ok(Run::events(events))
}
