//! Mana abilities of a shape the payment planner does not take (a discard, a mill, an untap or an X in the cost, a
//! choice of player): activated at priority like any ability, they resolve at once, before any player receives
//! priority, and their controller keeps priority (CR 605.3a, 605.3b, 117.3c).

use mtg_cards::ability::Ability;
use mtg_cards::assemble::{mana_shape, plain_mana_ability};
use mtg_core::ids::{ObjectId, PlayerId};

use crate::abilities::AbilityRef;
use crate::ability_reader;
use crate::events::Event;
use crate::state::GameState;

/// Whether `rules` is a mana ability that the planner never activates, so it is activated at priority and resolves
/// at once.
pub(crate) fn r281_at_priority(rules: &Ability) -> bool {
    rules.is_mana_ability() && plain_mana_ability(rules).is_none() && mana_shape(rules).is_none()
}

/// The player who keeps priority once the ability object `id` stops resolving: the controller of a mana ability that
/// resolved at once; `None` for any other object.
pub(crate) fn r281_keeps_priority(state: &GameState, id: ObjectId) -> Option<PlayerId> {
    let object = state.objects.get(&id)?;
    let ability = object.ability()?;
    r281_at_priority(ability.rules()).then(|| object.controller())
}

/// Ends the events of an activation, mana paid, with the start of its resolution when the ability put on the stack is a
/// mana ability activated at priority (CR 605.3a).
pub(crate) fn r281_resolve_at_once(state: &GameState, events: &mut Vec<Event>) {
    let at_once = events.iter().find_map(|event| match event {
        Event::AbilityPutOnStack(put) if put.cause.is_none() => {
            let address = AbilityRef {
                object: put.source.id,
                ability: put.ability,
                source: put.origin,
            };
            ability_reader::ability(state, address)
                .is_some_and(|rules| r281_at_priority(&rules))
                .then_some(put.object)
        }
        _ => None,
    });
    if let Some(object) = at_once {
        events.push(Event::ResolutionBegan { object });
    }
}
