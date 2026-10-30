//! `Effect::UnlockDoor`: each player named unlocks a locked door of a Room they control, the one they choose
//! (CR 709.5e); a player with none unlocks nothing.

use mtg_cards::effect::Effect;
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::choose::{ChoiceOption, OptionChooser, Optioned, pick_options};
use crate::dk1c5_rooms::{dk1c5_door, dk1c5_locked};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};

/// Asks each player for the door they unlock, then unlocks them.
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::UnlockDoor { who } = effect else {
        return Err(wrong_effect("UnlockDoor"));
    };
    let state = cx.state();
    let doors: Vec<(PlayerId, Vec<(ObjectId, u8)>)> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .map(|player| (player, dk1c5_locked(state, player)))
        .collect();
    let choosers: Vec<OptionChooser> = doors
        .iter()
        .map(|(player, locked)| OptionChooser {
            player: *player,
            options: locked
                .iter()
                .map(|(id, door)| {
                    let name = dk1c5_door(state, *id, *door).map_or("", |face| face.name.as_str());
                    ChoiceOption::Label(name.to_owned())
                })
                .collect(),
            text: "Choose a door to unlock.".to_owned(),
        })
        .collect();
    let taken = match pick_options(cx, &choosers)? {
        Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
        Optioned::Done(taken) => taken,
    };
    let mut answers = taken.iter();
    let mut events = Vec::new();
    for (player, locked) in &doors {
        if locked.is_empty() {
            continue;
        }
        let picked = answers.next().map_or(0, |(_, picked)| *picked);
        if let Some((object, door)) = locked.get(picked) {
            events.push(Event::DoorUnlocked {
                object: *object,
                door: *door,
                player: *player,
            });
        }
    }
    Ok(Run::events(events))
}
