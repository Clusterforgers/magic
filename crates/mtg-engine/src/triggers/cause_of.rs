//! Who caused the events of a batch: the controller of the spell or ability behind them, derived at detection with no field on any event.

use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;

use crate::characteristics::controller_of;
use crate::choose::Target;
use crate::events::Event;
use crate::object::ObjectRef;
use crate::state::GameState;

/// The spell or ability that caused a batch of events, and the player who controls it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CausedBy {
    /// The controller of the spell or ability (of the source of the damage).
    pub(crate) player: PlayerId,
    /// The spell, or the object an ability comes from, or the source of the damage.
    pub(crate) object: Option<ObjectRef>,
}

/// The cause of the events: the spell or ability that is resolving; else the spell being cast or the ability being
/// activated, whose costs are paid in the batch; else the source of the damage dealt (combat damage). None for the
/// actions of the turn and for state-based actions, which no spell or ability causes.
///
/// `pre` is the state before the batch (the state after when nothing looks back) and `post` the state after.
pub(crate) fn cause_of(pre: &GameState, events: &[Event], post: &GameState) -> Option<CausedBy> {
    let resolving = post.resolving().or_else(|| pre.resolving());
    if let Some(running) = resolving
        && let Some(object) = post
            .object(running.object)
            .or_else(|| pre.object(running.object))
    {
        let origin = object.ability().map_or_else(
            || object.reference(),
            crate::abilities::AbilityObject::source,
        );
        return Some(CausedBy {
            player: object.controller(),
            object: Some(origin),
        });
    }
    for event in events {
        match event {
            Event::AbilityPutOnStack(put) if put.cause.is_none() => {
                return Some(CausedBy {
                    player: put.controller,
                    object: Some(put.source),
                });
            }
            Event::SpellCast { player, object, .. } => {
                let spell = post
                    .object(*object)
                    .map(crate::object::GameObject::reference);
                return Some(CausedBy {
                    player: *player,
                    object: spell,
                });
            }
            _ => {}
        }
    }
    events.iter().find_map(|event| match event {
        Event::DamageDealt { source, .. } => {
            let object = post
                .object(*source)
                .or_else(|| pre.object(*source))
                .map(crate::object::GameObject::reference)?;
            let player = controller_of(post, *source).or_else(|| controller_of(pre, *source))?;
            Some(CausedBy {
                player,
                object: Some(object),
            })
        }
        // CR 714.3b: the turn-based lore counter of a Saga is put by the Saga's controller.
        Event::CountersChanged(changed) if changed.delta > 0 => {
            let Target::Object(saga) = changed.on else {
                return None;
            };
            if changed.counter != CounterKind::lore() {
                return None;
            }
            let player = controller_of(post, saga.id).or_else(|| controller_of(pre, saga.id))?;
            Some(CausedBy {
                player,
                object: Some(saga),
            })
        }
        _ => None,
    })
}
