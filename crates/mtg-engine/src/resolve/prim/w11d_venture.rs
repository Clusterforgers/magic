//! `Effect::Venture`: each player named ventures into the dungeon (CR 701.49).

use std::sync::Arc;

use mtg_cards::effect::Effect;
use mtg_cards::w11d_dungeon::{W11dDungeon, w11d_dungeons};
use mtg_core::ids::{ObjectId, PlayerId};

use super::wrong_effect;
use crate::choose::{ChoiceOption, OptionChooser, Optioned, pick_options};
use crate::events::Event;
use crate::resolve::{Cx, ResolveError, Run};
use crate::state::GameState;
use crate::w11d_dungeon::{W11dMarker, W11dVentured, w11d_definition, w11d_dungeon_at};

/// What a venture of one player does once its choice is made.
enum W11dPlan {
    /// Into the first room of one of these dungeons (indices into the embedded dungeons), after completing the one the
    /// marker is on the last room of.
    Enter {
        completing: Option<ObjectId>,
        dungeons: Vec<u8>,
    },
    /// Down an arrow of the dungeon the marker is in, to one of these rooms.
    Down { marker: W11dMarker, rooms: Vec<u8> },
}

/// Each player the effect names who is in the game ventures: a player in a dungeon moves down an arrow (choosing the
/// room where two arrows leave theirs); a player not in one, or on its last room, completes it and enters the first
/// room of a dungeon they choose among those ready to play (only Undercity for "venture into Undercity").
pub(super) fn run(cx: &Cx<'_>, effect: &Effect) -> Result<Run, ResolveError> {
    let Effect::Venture { who, undercity } = effect else {
        return Err(wrong_effect("Venture"));
    };
    let state = cx.state();
    let plans: Vec<(PlayerId, W11dPlan)> = cx
        .scope
        .players(who)?
        .into_iter()
        .filter(|player| state.is_alive(*player))
        .map(|player| (player, w11d_plan(state, player, *undercity)))
        .collect();
    let choosers: Vec<OptionChooser> = plans
        .iter()
        .map(|(player, plan)| w11d_chooser(*player, plan))
        .collect();
    let taken = match pick_options(cx, &choosers)? {
        Optioned::Ask(decision) => return Ok(Run::Ask(decision)),
        Optioned::Done(taken) => taken,
    };
    let mut events = Vec::new();
    let mut next_object = state.next_object;
    let mut answers = taken.iter();
    for ((player, plan), chooser) in plans.iter().zip(&choosers) {
        if chooser.options.is_empty() {
            continue;
        }
        let picked = answers.next().map_or(0, |(_, picked)| *picked);
        events.extend(w11d_events(*player, plan, picked, &mut next_object));
    }
    Ok(Run::events(events))
}

fn w11d_plan(state: &GameState, player: PlayerId, undercity: bool) -> W11dPlan {
    let marker = state.w11d_marker(player);
    let room = marker.and_then(|m| w11d_dungeon_at(m.dungeon)?.rooms.get(usize::from(m.room)));
    match (marker, room) {
        (Some(marker), Some(room)) if !room.next.is_empty() => W11dPlan::Down {
            marker,
            rooms: room.next.clone(),
        },
        _ => W11dPlan::Enter {
            completing: marker.map(|m| m.object),
            dungeons: w11d_enterable(undercity),
        },
    }
}

/// The dungeons a venture can enter: those whose every room plays, Undercity only by "venture into Undercity".
fn w11d_enterable(undercity: bool) -> Vec<u8> {
    w11d_dungeons()
        .unwrap_or_default()
        .iter()
        .enumerate()
        .filter(|(_, dungeon)| dungeon.undercity == undercity && dungeon.w11d_ready())
        .filter_map(|(index, _)| u8::try_from(index).ok())
        .collect()
}

fn w11d_chooser(player: PlayerId, plan: &W11dPlan) -> OptionChooser {
    let (options, text) = match plan {
        W11dPlan::Enter { dungeons, .. } => (
            dungeons
                .iter()
                .filter_map(|index| w11d_dungeon_at(*index))
                .map(|dungeon| ChoiceOption::Label(dungeon.name.clone()))
                .collect(),
            "Choose a dungeon to enter.",
        ),
        W11dPlan::Down { marker, rooms } => (
            rooms
                .iter()
                .filter_map(|room| w11d_room_name(marker.dungeon, *room))
                .map(|name| ChoiceOption::Label(name.to_owned()))
                .collect(),
            "Choose the next room.",
        ),
    };
    OptionChooser {
        player,
        options,
        text: text.to_owned(),
    }
}

fn w11d_room_name(dungeon: u8, room: u8) -> Option<&'static str> {
    Some(
        w11d_dungeon_at(dungeon)?
            .rooms
            .get(usize::from(room))?
            .name
            .as_str(),
    )
}

fn w11d_events(
    player: PlayerId,
    plan: &W11dPlan,
    picked: usize,
    next_object: &mut u64,
) -> Vec<Event> {
    match plan {
        W11dPlan::Down { marker, rooms } => {
            let Some((room, dungeon)) = rooms.get(picked).zip(w11d_dungeon_at(marker.dungeon))
            else {
                return Vec::new();
            };
            vec![w11d_ventured(player, marker.object, dungeon, *room, None)]
        }
        W11dPlan::Enter {
            completing,
            dungeons,
        } => {
            let Some(dungeon) = dungeons
                .get(picked)
                .and_then(|index| w11d_dungeon_at(*index))
            else {
                return Vec::new();
            };
            let mut events: Vec<Event> = completing
                .map(|object| Event::DungeonCompleted { player, object })
                .into_iter()
                .collect();
            let object = ObjectId(*next_object);
            *next_object += 1;
            let definition = Arc::new(w11d_definition(dungeon));
            events.push(w11d_ventured(player, object, dungeon, 0, Some(definition)));
            events
        }
    }
}

fn w11d_ventured(
    player: PlayerId,
    object: ObjectId,
    dungeon: &W11dDungeon,
    room: u8,
    definition: Option<Arc<mtg_cards::CardDefinition>>,
) -> Event {
    Event::Ventured(Box::new(W11dVentured {
        player,
        object,
        dungeon: dungeon.name.clone(),
        room,
        room_name: dungeon
            .rooms
            .get(usize::from(room))
            .map(|r| r.name.clone())
            .unwrap_or_default(),
        definition,
    }))
}
