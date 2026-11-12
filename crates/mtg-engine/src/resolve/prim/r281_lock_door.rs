//! `Effect::LockOrUnlockDoor`: the controller of the effect locks an unlocked door or unlocks a locked door of the Room it
//! names, the door they choose ("Lock or unlock a door of target Room you control", CR 709.5).

use mtg_cards::effect::Effect;

use super::wrong_effect;
use crate::choose::{ChoiceOption, OptionChooser, Optioned, pick_options};
use crate::dk1c5_rooms::dk1c5_door;
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// Asks for the door to lock or unlock, then changes it.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::LockOrUnlockDoor { what } = effect else {
        return Err(wrong_effect("LockOrUnlockDoor"));
    };
    let state = cx.state();
    let player = cx.controller();
    let Some(room) = cx
        .scope
        .objects(what)?
        .into_iter()
        .map(|object| object.id)
        .find(|id| state.dk1c5_doors(*id).is_some())
    else {
        return Ok(Run::events(Vec::new()));
    };
    let Some(doors) = state.dk1c5_doors(room) else {
        return Ok(Run::events(Vec::new()));
    };
    let options = (0..2u8)
        .map(|door| {
            let name = dk1c5_door(state, room, door).map_or("", |face| face.name.as_str());
            let verb = if doors[usize::from(door)] {
                "Lock"
            } else {
                "Unlock"
            };
            ChoiceOption::Label(format!("{verb} {name}"))
        })
        .collect();
    let chooser = OptionChooser {
        player,
        options,
        text: "Choose a door to lock or unlock.".to_owned(),
    };
    let door = match pick_options(cx, &[chooser])? {
        Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
        Optioned::Done(taken) => taken.first().map_or(0, |(_, door)| *door),
    };
    let door = u8::try_from(door).unwrap_or(0);
    let event = if doors[usize::from(door)] {
        Event::DoorLocked {
            object: room,
            door,
            player,
        }
    } else {
        Event::DoorUnlocked {
            object: room,
            door,
            player,
        }
    };
    Ok(Run::events(vec![event]))
}
