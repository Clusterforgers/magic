//! `Effect::RingTempts`: the Ring tempts players (CR 701.54).

use mtg_cards::effect::Effect;
use mtg_cards::refs::{Pred, Selector};
use mtg_core::card::CardType;

use super::wrong_effect;
use crate::characteristics::controller_of;
use crate::choose::{Chooser, Picked, pick_cards};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::ring::MAX_LEVEL;
use crate::zones::Zone;

/// Each player the effect names who is still in the game has their Ring level rise by one (4 at most) and chooses
/// a creature they control as their Ring-bearer, the same one again if they like; with no creature there is no new
/// Ring-bearer. One `RingTempted` carries the level and the choice, asked only when the player controls several creatures.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::RingTempts { who } = effect else {
        return Err(wrong_effect("RingTempts"));
    };
    let state = cx.state();
    let creatures = cx
        .scope
        .matching(&Selector(vec![Pred::Type(CardType::Creature)]))?;
    let tempted: Vec<_> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .collect();
    let choosers: Vec<Chooser> = tempted
        .iter()
        .map(|player| {
            let from: Vec<_> = creatures
                .iter()
                .filter(|creature| {
                    state
                        .object(creature.id)
                        .is_some_and(|object| object.zone() == Zone::Battlefield)
                        && controller_of(state, creature.id) == Some(*player)
                })
                .map(|creature| creature.id)
                .collect();
            Chooser {
                player: *player,
                take: usize::from(!from.is_empty()),
                from,
                text: "Choose a creature you control as your Ring-bearer.".to_owned(),
            }
        })
        .collect();
    let chosen = match pick_cards(cx, &choosers)? {
        Picked::Ask(decision) => return Ok(Run::Ask(decision)),
        Picked::Done(chosen) => chosen,
    };
    let events = tempted
        .into_iter()
        .map(|player| Event::RingTempted {
            player,
            level: state.ring_level(player).saturating_add(1).min(MAX_LEVEL),
            bearer: chosen
                .iter()
                .find(|(who, _)| *who == player)
                .and_then(|(_, cards)| cards.first().copied()),
        })
        .collect();
    Ok(Run::events(events))
}
