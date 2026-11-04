//! The manual actions that turn a double-faced permanent over, put a double-faced card onto the battlefield back face up,
//! and give a creature a combat permission until end of turn.

use mtg_cards::definition::Layout;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::day_night::turned_over;
use crate::events::Event;
use crate::manual::{ManualRefusal, ManualZone, moves, own};
use crate::state::GameState;
use crate::zones::Zone;

/// The events that turn the transforming double-faced permanent `id`, which the seat controls, to its other face.
pub(crate) fn transform(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
) -> Result<Vec<Event>, ManualRefusal> {
    let object = own(state, player, id)?;
    if object.controller() != player {
        return Err(ManualRefusal::NotYours);
    }
    if object.zone() != Zone::Battlefield || object.whole_card().layout != Layout::Transform {
        return Err(ManualRefusal::NotTransformable(id));
    }
    Ok(vec![Event::Transformed {
        object: id,
        face: turned_over(object.face()),
    }])
}

/// The events that put the double-faced card `id`, which is not on the battlefield, onto it with its back face up.
pub(crate) fn move_back(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
) -> Result<Vec<Event>, ManualRefusal> {
    let object = own(state, player, id)?;
    if !matches!(
        object.whole_card().layout,
        Layout::Transform | Layout::ModalDfc
    ) || object.zone() == Zone::Battlefield
    {
        return Err(ManualRefusal::NotDoubleFaced(id));
    }
    let mut events = moves(state, player, id, ManualZone::Battlefield)?;
    // The face is set right after the card enters, as a card put onto the battlefield transformed does.
    events.insert(
        1,
        Event::Transformed {
            object: id,
            face: Some(1),
        },
    );
    Ok(events)
}
