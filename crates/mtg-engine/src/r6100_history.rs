//! Facts of the turn and the game for pack r61-00, folded from events: the creatures that fought
//! this turn, the creatures exiled from the battlefield the creature cards that left a graveyard, and the
//! permanents each source dealt damage to this game.

use mtg_cards::refs::Selector;
use mtg_cards::value::TallyEvent;
use mtg_core::card::CardType;
use mtg_core::ids::ObjectId;

use crate::choose::Target;
use crate::object::{GameObject, ObjectRef};
use crate::resolve::{ResolveError, Scope};
use crate::state::{GameState, Tally};
use crate::zones::Zone;

/// Logs a creature that fought for its controller, with the creature it fought (`TallyEvent::Fought`).
pub(crate) fn r6100_fold_fought(state: &mut GameState, object: ObjectId, other: ObjectId) {
    let Some(controller) = state.object(object).map(GameObject::controller) else {
        return;
    };
    let other = state
        .object(other)
        .map(|fought| Target::Object(fought.reference()));
    state.record(TallyEvent::Fought, Some(controller), (object, other), 1);
}

/// Counts a move before it happens, while the object is still what it was: a creature put into exile from the battlefield
/// for the player who controlled it, a creature card that leaves a graveyard for its owner.
pub(crate) fn r6100_before_move(state: &mut GameState, id: ObjectId, to: Zone) {
    let Some(object) = state.object(id) else {
        return;
    };
    let (from, controller, owner) = (object.zone(), object.controller(), object.owner());
    let exiled = from == Zone::Battlefield && to == Zone::Exile;
    let left_graveyard = matches!(from, Zone::Graveyard(_)) && !matches!(to, Zone::Graveyard(_));
    if !exiled && !left_graveyard {
        return;
    }
    let creature = crate::characteristics::characteristics(state, id)
        .is_some_and(|chars| chars.types.contains(CardType::Creature));
    if !creature {
        return;
    }
    if exiled {
        state.tally(controller, Tally::R6100CreatureExiledUnder);
    } else if object_is_card(state, id) {
        state.tally(owner, Tally::R6100CreatureCardLeftGraveyard);
    }
}

/// Whether the object is a card, not a token or a copy.
fn object_is_card(state: &GameState, id: ObjectId) -> bool {
    state
        .object(id)
        .is_some_and(|object| object.kind() == crate::object::ObjectKind::Card)
}

/// Keeps a damage dealt to a permanent with its source as it is now, for the rest of the game.
pub(crate) fn r6100_note_struck(state: &mut GameState, source: ObjectId, permanent: ObjectId) {
    let Some(struck) = state.object(permanent).map(GameObject::reference) else {
        return;
    };
    if let Some(shot) = crate::snapshot::snapshot(state, source) {
        state.r5522.r6100_struck.push((struck, shot));
    }
}

impl Scope<'_> {
    /// Whether a source the selector matches, as it was then, dealt damage this game to the permanent as it is now.
    pub(crate) fn r6100_damaged_this_game(
        &self,
        by: &Selector,
        permanent: ObjectRef,
    ) -> Result<bool, ResolveError> {
        for (struck, shot) in &self.state.r5522.r6100_struck {
            if *struck != permanent {
                continue;
            }
            let mut scope = self.clone();
            scope.as_was = true;
            scope.snapshots.insert(0, shot);
            if scope.selects(by, shot.reference())? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
