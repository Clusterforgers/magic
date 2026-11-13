//! "Until a player casts a creature spell" (Soul Sculptor): the game keeps the timestamp clock as the latest creature
//! spell was cast, and an effect of that duration ends once a creature spell is cast after its own timestamp.

use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;

use crate::state::GameState;

/// Notes the spell `object` just cast when it is a creature spell (CR 601.2i), at the clock of the next timestamp, so
/// every effect created before it ends.
pub(crate) fn r5512_note_cast(state: &mut GameState, object: ObjectId) {
    let creature = crate::characteristics::characteristics(state, object)
        .is_some_and(|chars| chars.types.contains(CardType::Creature));
    if creature {
        state.r5512_creature_cast_at = Some(state.next_timestamp);
    }
}

/// Whether a creature spell was cast after the effect with this timestamp began.
pub(crate) fn r5512_creature_cast_since(state: &GameState, timestamp: u64) -> bool {
    state
        .r5512_creature_cast_at
        .is_some_and(|at| at > timestamp)
}
