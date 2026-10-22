//! `Effect::MyriadCopies`: token copies of an attacking creature attack the other opponents (CR 702.116a).

use mtg_cards::effect::Effect;
use mtg_core::ids::{ObjectId, PlayerId};

use super::create_token::{MadeFrom, copy_of_object};
use super::wrong_effect;
use crate::choose::Target;
use crate::combat::AttackTarget;
use crate::events::Event;
use crate::object::ObjectRef;
use crate::resolve::{Cx, Next, ResolveError, Run};
use crate::tokens::TokenCreated;

/// The opponents of the controller who are still in the game and are not `defending`, in turn order.
fn others(cx: &Cx<'_>, defending: Option<PlayerId>) -> Vec<PlayerId> {
    let state = cx.state();
    state
        .turn_order_from(state.turn().active())
        .into_iter()
        .filter(|player| *player != cx.scope.controller && Some(*player) != defending)
        .collect()
}

/// For each creature `of` names that is still the object named and is attacking, a token that is a copy of it for every
/// opponent of the controller other than the player it attacks, or whose planeswalker it attacks, tapped and attacking
/// that opponent. The tokens are never declared as attackers, so nothing that triggers on attacking sees them
/// (CR 508.4). Nothing happens for a creature that is not attacking.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::MyriadCopies { of, bind } = effect else {
        return Err(wrong_effect("MyriadCopies"));
    };
    let state = cx.state();
    let combat = state.combat();
    let first = state.next_object().get();
    let mut made: Vec<(MadeFrom, PlayerId)> = Vec::new();
    for object in cx.scope.live_objects(of)? {
        let Some(attacker) = combat
            .attackers()
            .iter()
            .find(|attacker| attacker.object() == object.id())
        else {
            continue;
        };
        let Some(copy) = copy_of_object(cx, object.reference(), &[], false)? else {
            continue;
        };
        for opponent in others(cx, attacker.defender(state)) {
            made.push((copy.clone(), opponent));
        }
    }
    let events: Vec<Event> = made
        .iter()
        .zip(first..)
        .map(|(((definition, copy_of), opponent), id)| {
            Event::TokenCreated(TokenCreated {
                object: ObjectId(id),
                copy_of: *copy_of,
                controller: cx.scope.controller,
                name: definition.name.clone(),
                tapped: true,
                attacking: Some(AttackTarget::Player(*opponent)),
                definition: definition.clone(),
            })
        })
        .collect();
    // A token is a new object: its zone-change counter starts at 0.
    let bound = bind
        .iter()
        .map(|name| {
            let named = (first..).zip(&made).map(|(id, _)| {
                Target::Object(ObjectRef {
                    id: ObjectId(id),
                    counter: 0,
                })
            });
            (name.clone(), named.collect())
        })
        .collect();
    Ok(Run::Done {
        events,
        bound,
        snapshots: Vec::new(),
        next: Next::Sibling,
    })
}
