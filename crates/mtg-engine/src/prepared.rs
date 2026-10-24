//! The spell of a prepared permanent (the `prepare` layout): while the permanent is prepared, its controller may cast a
//! copy of the spell face of its card at the timing of that spell, and doing so unprepares it.
//!
//! The copy is made as the cast begins: a token card in exile owned by the caster (`Event::CardCopied`, the copy of a
//! card of CR 707.12) that `ways` lets them cast for its mana cost, and that ceases to exist anywhere but on the stack.
//! The cast is the ordinary one (`announce/`); the `Unprepared` of the permanent is logged with its `SpellCast`.

use std::sync::Arc;

use mtg_cards::definition::Layout;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::controller_of_object;
use crate::copies::CardCopy;
use crate::decision::FaceOption;
use crate::events::Event;
use crate::faces;
use crate::fold::apply_event;
use crate::mana::{ManaOption, ManaPool};
use crate::object::{GameObject, ObjectRef, PermanentState};
use crate::state::GameState;
use crate::ways;
use crate::zones::Zone;

/// The position of the spell face among the faces of a prepare card.
pub(crate) const SPELL_FACE: u8 = 1;

/// The prepared permanent of `player` that `id` is, as it stands: on the battlefield, a prepare card, prepared, and
/// controlled by `player`.
fn prepared_permanent(state: &GameState, player: PlayerId, id: ObjectId) -> Option<&GameObject> {
    state.object(id).filter(|object| {
        object.zone() == Zone::Battlefield
            && object.whole_card().layout == Layout::Prepare
            && object.permanent().is_some_and(PermanentState::prepared)
            && controller_of_object(state, object) == player
    })
}

/// The event that copies the spell face of the prepared permanent `id` for `player`, and the object the copy will be.
pub(crate) fn copy_of(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
) -> Option<(Event, ObjectId)> {
    let permanent = prepared_permanent(state, player, id)?;
    let face = permanent.whole_card().faces.get(usize::from(SPELL_FACE))?;
    let object = state.next_object();
    let event = Event::CardCopied(Box::new(CardCopy {
        object,
        of: permanent.reference(),
        owner: player,
        definition: Arc::new(face.copiable()),
    }));
    Some((event, object))
}

/// Whether `player` can cast a copy of the spell face of the prepared permanent `id` now: the copy is tried in a state
/// that already holds it, so its timing, targets and cost are those of the spell.
pub(crate) fn castable(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
) -> bool {
    let Some((event, copy)) = copy_of(state, player, id) else {
        return false;
    };
    let mut trial = state.clone();
    apply_event(&mut trial, &event).is_ok()
        && ways::castable_as_is(&trial, player, copy, pool, sources, life)
}

/// The spell faces of the prepared permanents of `player` that can be cast now, in battlefield order, for the options of a
/// player with priority.
pub(crate) fn options(
    state: &GameState,
    player: PlayerId,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
) -> Vec<FaceOption> {
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| castable(state, player, *id, pool, sources, life))
        .map(|card| {
            let (name, cost) = faces::describe(state, card, Some(SPELL_FACE));
            FaceOption {
                card,
                face: SPELL_FACE,
                name,
                cost,
                land: false,
            }
        })
        .collect()
}

/// The prepared permanent that the card copy `copy` was made from, while it still is on the battlefield and prepared.
pub(crate) fn permanent_of(state: &GameState, copy: &GameObject) -> Option<ObjectRef> {
    let of = copy.copied_from().filter(|_| copy.is_card_copy())?;
    state
        .object(of.id)
        .filter(|object| {
            object.counter() == of.counter
                && object.zone() == Zone::Battlefield
                && object.permanent().is_some_and(PermanentState::prepared)
        })
        .map(GameObject::reference)
}

/// The event that unprepares the permanent whose spell `spell` is a copy of, when it is one.
pub(crate) fn unprepare(state: &GameState, spell: ObjectId) -> Option<Event> {
    let permanent = permanent_of(state, state.object(spell)?)?;
    Some(Event::Unprepared {
        object: permanent.id,
    })
}
