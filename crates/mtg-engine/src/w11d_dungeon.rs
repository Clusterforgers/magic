//! Dungeons (CR 309): the dungeon object in its owner's command zone, the player's venture marker, the `Ventured` and
//! `DungeonCompleted` events and the state-based action that completes a dungeon.

use std::collections::BTreeMap;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::assemble::token_definition;
use mtg_cards::card::TokenSpec;
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_cards::trigger::TriggerPattern;
use mtg_cards::w11d_dungeon::{W11dDungeon, w11d_dungeons};
use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

use crate::error::EngineError;
use crate::events::Event;
use crate::fold::{invalid, reset_passes};
use crate::object::{GameObject, ObjectKind};
use crate::state::GameState;
use crate::zones::{Placement, Zone};

/// Where a player's venture marker is: the dungeon object, its index among the embedded dungeons and the room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct W11dMarker {
    /// The dungeon in the player's command zone.
    pub object: ObjectId,
    /// The dungeon, as an index into `w11d_dungeons()`.
    pub dungeon: u8,
    /// The room, as an index into the dungeon's rooms.
    pub room: u8,
}

/// The dungeons of a game: each player's venture marker and how many dungeons each player has completed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct W11dDungeons {
    pub(crate) at: BTreeMap<PlayerId, W11dMarker>,
    pub(crate) completed: BTreeMap<PlayerId, u32>,
}

impl W11dDungeons {
    /// Whether no player has ventured.
    pub fn is_empty(&self) -> bool {
        self.at.is_empty() && self.completed.is_empty()
    }
}

/// A player ventured (CR 701.49): into the first room of a new dungeon, whose object `definition` makes in their
/// command zone, or down an arrow into `room` of the dungeon they are in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct W11dVentured {
    /// Who ventured.
    pub player: PlayerId,
    /// The dungeon object.
    pub object: ObjectId,
    /// The dungeon's name.
    pub dungeon: String,
    /// The room, as an index into the dungeon's rooms.
    pub room: u8,
    /// The room's name.
    pub room_name: String,
    /// The definition of a dungeon entered by this venture; none for a move down an arrow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(skip))]
    pub definition: Option<Arc<CardDefinition>>,
}

impl GameState {
    /// Where the player's venture marker is, if they are in a dungeon.
    pub fn w11d_marker(&self, player: PlayerId) -> Option<W11dMarker> {
        self.w11d.at.get(&player).copied()
    }

    /// How many dungeons the player has completed.
    pub fn w11d_completed(&self, player: PlayerId) -> u32 {
        self.w11d.completed.get(&player).copied().unwrap_or(0)
    }
}

/// The embedded dungeon at this index.
pub(crate) fn w11d_dungeon_at(index: u8) -> Option<&'static W11dDungeon> {
    w11d_dungeons().ok()?.get(usize::from(index))
}

/// The definition of a dungeon object: its name, and one room ability for each room, a triggered ability that works
/// from the command zone and triggers as its owner enters that room.
pub(crate) fn w11d_definition(dungeon: &W11dDungeon) -> CardDefinition {
    let abilities = dungeon
        .rooms
        .iter()
        .enumerate()
        .map(|(index, room)| Ability::Triggered {
            when: TriggerPattern::Ventured {
                who: Ref::You,
                room: Some(u8::try_from(index).unwrap_or(u8::MAX)),
            },
            intervening_if: None,
            limit: None,
            from: ZoneKind::Command,
            targets: room.targets.clone(),
            effects: room.effects.clone(),
        })
        .collect();
    let spec = TokenSpec {
        name: dungeon.name.clone(),
        colors: Vec::new(),
        supertypes: Vec::new(),
        types: Vec::new(),
        subtypes: Vec::new(),
        power: None,
        toughness: None,
        abilities,
    };
    token_definition(&spec, &crate::caps::caps())
}

/// Folds a venture: makes the dungeon object of a new dungeon and moves the player's marker.
pub(crate) fn w11d_fold_ventured(state: &mut GameState, event: &W11dVentured) -> Result<(), EngineError> {
    let dungeon = w11d_dungeons()
        .ok()
        .and_then(|all| all.iter().position(|d| d.name == event.dungeon))
        .and_then(|index| u8::try_from(index).ok())
        .ok_or_else(|| invalid(format!("{} is not a dungeon", event.dungeon)))?;
    if let Some(definition) = &event.definition {
        let id = event.object;
        if state.objects.contains_key(&id) || state.ceased.contains_key(&id) {
            return Err(invalid(format!("{id:?} already exists")));
        }
        let zone = Zone::Command(event.player);
        let mut object = GameObject::new(id, event.player, Arc::clone(definition), zone);
        object.kind = ObjectKind::Dungeon;
        crate::copies::note_definition(state, definition);
        state.objects.insert(id, object);
        state.zones.insert(zone, id, Placement::Top);
        state.next_object = state.next_object.max(id.get() + 1);
    } else if state.w11d_marker(event.player).map(|m| m.object) != Some(event.object) {
        return Err(invalid(format!("{:?} is not in {:?}", event.player, event.object)));
    }
    state.w11d.at.insert(
        event.player,
        W11dMarker {
            object: event.object,
            dungeon,
            room: event.room,
        },
    );
    reset_passes(state);
    Ok(())
}

/// Folds a completed dungeon: the dungeon leaves the game and the player has completed one more.
pub(crate) fn w11d_fold_completed(state: &mut GameState, player: PlayerId, object: ObjectId) -> Result<(), EngineError> {
    if state.w11d_marker(player).map(|m| m.object) != Some(object) {
        return Err(invalid(format!("{player:?} is not in {object:?}")));
    }
    state.w11d.at.remove(&player);
    *state.w11d.completed.entry(player).or_insert(0) += 1;
    state.zones.remove(Zone::Command(player), object);
    state.objects.remove(&object);
    reset_passes(state);
    Ok(())
}

/// The state-based action of CR 704.5t: a player whose marker is on the last room of a dungeon that is not the source
/// of a room ability waiting to be put on the stack or on the stack completes it.
pub(crate) fn w11d_sba(state: &GameState) -> Vec<Event> {
    state
        .w11d
        .at
        .iter()
        .filter(|(_, marker)| {
            w11d_dungeon_at(marker.dungeon)
                .and_then(|dungeon| dungeon.rooms.get(usize::from(marker.room)))
                .is_some_and(|room| room.next.is_empty())
        })
        .filter(|(_, marker)| !w11d_room_waits(state, marker.object))
        .map(|(player, marker)| Event::DungeonCompleted {
            player: *player,
            object: marker.object,
        })
        .collect()
}

/// Whether a room ability of this dungeon has triggered and not yet left the stack.
fn w11d_room_waits(state: &GameState, dungeon: ObjectId) -> bool {
    state
        .pending_triggers
        .iter()
        .any(|pending| pending.trigger.source.id == dungeon)
        || state.zone(Zone::Stack).iter().any(|id| {
            state
                .object(*id)
                .and_then(|object| object.ability.as_ref())
                .is_some_and(|ability| ability.source.id == dungeon)
        })
}
