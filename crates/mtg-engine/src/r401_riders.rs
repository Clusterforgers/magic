//! The riders of a cast permission that change the permanent a spell cast through it becomes once it is on the
//! battlefield: a keyword until end of turn ("it gains haste until end of turn") and a subtype in addition to its other
//! types. The replacement riders are in `replace/r401_riders.rs`.

use mtg_cards::modify::CastRider;
use mtg_core::ids::{EffectId, ObjectId};

use crate::effects::{ContinuousEffect, Duration, Modification, Selector as Affected};
use crate::events::Event;
use crate::object::ObjectRef;
use crate::state::GameState;

/// The effects the spell `id` gives the permanent it becomes from the riders it was cast with, numbered after the
/// effects `before` already makes in the same batch.
pub(crate) fn r401_entering(state: &GameState, id: ObjectId, before: &[Event]) -> Vec<Event> {
    let Some(spell) = state.object(id) else {
        return Vec::new();
    };
    let on_stack = spell.reference();
    let permanent = ObjectRef {
        counter: on_stack.counter + 1,
        ..on_stack
    };
    let made = u64::try_from(
        before
            .iter()
            .filter(|event| matches!(event, Event::EffectCreated { .. }))
            .count(),
    )
    .unwrap_or(0);
    spell
        .cast_riders()
        .iter()
        .filter_map(|rider| match rider {
            CastRider::GainsThisTurn(keyword) => Some((
                Modification::AddKeyword(keyword.clone()),
                Duration::UntilEndOfTurn,
            )),
            CastRider::AlsoSubtype(subtype) => {
                Some((Modification::AddSubtype(subtype.clone()), Duration::Forever))
            }
            _ => None,
        })
        .zip(made..)
        .map(|((modification, duration), at)| Event::EffectCreated {
            effect: ContinuousEffect::new(
                EffectId(state.next_effect + at),
                Affected::Objects(vec![permanent]),
                modification,
                duration,
                state.next_timestamp + at,
            )
            .begun(spell.controller(), state.turn().number())
            .into(),
        })
        .collect()
}
