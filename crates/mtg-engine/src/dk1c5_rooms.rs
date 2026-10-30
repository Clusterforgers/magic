//! Rooms (CR 709.5): a Room permanent has two doors, each locked or unlocked (`Doors`, state of the permanent). A Room
//! cast as one door enters with that door unlocked; one put onto the battlefield in any other way enters with both
//! locked. Its characteristics are those of its unlocked doors: one door is that face, both are the whole card, none is
//! an Enchantment — Room with nothing else (`mtg_cards::dk1c5_rooms`). Unlocking a locked door is a special action at
//! sorcery timing for that door's mana cost (`Event::DoorUnlocked`); a door also unlocks by an effect. The door state is
//! public.

use std::hash::{Hash, Hasher};

use mtg_cards::definition::CardDefinition;
use mtg_cards::dk1c5_rooms::DK1C5_LOCKED;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::characteristics::controller_of;
use crate::decision::FaceOption;
use crate::error::EngineError;
use crate::events::Event;
use crate::fold::invalid;
use crate::mana::{ManaOption, ManaPool, can_afford};
use crate::mana_takes::for_card;
use crate::priority::sorcery_timing;
use crate::spend::Spend;
use crate::state::GameState;
use crate::zones::Zone;

/// The doors of a Room permanent: which are unlocked. A permanent that is not a Room has none and adds nothing to the
/// hash of the game.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Doors(pub(crate) u8);

/// The bit that marks a Room; the low bits are its unlocked doors.
const ROOM: u8 = 0x80;
/// Both doors of a Room.
const BOTH: u8 = 0b11;

impl Hash for Doors {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if self.0 != 0 {
            self.0.hash(state);
        }
    }
}

impl Doors {
    /// The doors of a permanent of `card` entering the battlefield: the door it was cast as unlocked (CR 709.5d), none
    /// for a Room that was not cast, and no doors at all for any other card.
    pub(crate) fn entering(card: &CardDefinition, face: Option<u8>) -> Doors {
        if !card.is_room() {
            return Doors::default();
        }
        Doors(ROOM | face.filter(|door| *door < 2).map_or(0, |door| 1 << door))
    }

    /// Whether the permanent is a Room.
    pub const fn is_room(self) -> bool {
        self.0 & ROOM != 0
    }

    /// Whether `door` is unlocked.
    pub const fn unlocked(self, door: u8) -> bool {
        door < 2 && self.0 & (1 << door) != 0
    }

    /// Whether both doors are unlocked.
    pub const fn fully_unlocked(self) -> bool {
        self.0 & BOTH == BOTH
    }

    /// The face the permanent is in use as: the one unlocked door, or none for both or neither.
    pub(crate) const fn face(self) -> Option<u8> {
        match self.0 & BOTH {
            0b01 => Some(0),
            0b10 => Some(1),
            _ => None,
        }
    }

    /// The characteristics of a Room with these doors (CR 709.5c): the whole card when both are unlocked, the bare
    /// Room when none is, the face of the one unlocked door otherwise; `None` for a permanent that is not a Room.
    pub(crate) fn card(self, def: &CardDefinition) -> Option<&CardDefinition> {
        if !self.is_room() {
            return None;
        }
        Some(match self.0 & BOTH {
            BOTH => def,
            0 => &DK1C5_LOCKED,
            _ => def.face(self.face().unwrap_or(0)),
        })
    }
}

impl GameState {
    /// Which doors of the Room permanent `id` are unlocked, `None` for anything else: public information.
    pub fn dk1c5_doors(&self, id: ObjectId) -> Option<[bool; 2]> {
        let doors = self.object(id)?.permanent()?.doors;
        doors
            .is_room()
            .then(|| [doors.unlocked(0), doors.unlocked(1)])
    }
}

/// Unlocks `door` of the Room `object` (CR 709.5e): the door must be locked.
pub(crate) fn dk1c5_fold_unlocked(
    state: &mut GameState,
    object: ObjectId,
    door: u8,
) -> Result<(), EngineError> {
    let permanent = state
        .objects
        .get_mut(&object)
        .filter(|o| o.zone() == Zone::Battlefield)
        .and_then(|o| o.permanent.as_mut())
        .ok_or_else(|| invalid(format!("{object:?} is not a permanent")))?;
    let doors = permanent.doors;
    if !doors.is_room() || door > 1 || doors.unlocked(door) {
        return Err(invalid(format!(
            "{object:?} has no locked door {door} to unlock"
        )));
    }
    permanent.doors = Doors(doors.0 | 1 << door);
    permanent.face = permanent.doors.face();
    Ok(())
}

/// The locked doors of the Room permanents `player` controls, as (permanent, door).
pub(crate) fn dk1c5_locked(state: &GameState, player: PlayerId) -> Vec<(ObjectId, u8)> {
    state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| controller_of(state, *id) == Some(player))
        .flat_map(|id| {
            let doors = state.dk1c5_doors(id).unwrap_or([true, true]);
            (0..2u8)
                .filter(move |door| !doors[usize::from(*door)])
                .map(move |door| (id, door))
        })
        .collect()
}

/// The name and mana cost of `door` of the Room `id`.
pub(crate) fn dk1c5_door(state: &GameState, id: ObjectId, door: u8) -> Option<&CardDefinition> {
    let def = &state.object(id)?.def;
    def.faces.get(usize::from(door))
}

/// The locked doors `player` may unlock now as a special action (CR 116.2m): any time they could cast a sorcery, for
/// the door's mana cost.
pub(crate) fn dk1c5_unlockable(
    state: &GameState,
    player: PlayerId,
    pool: &ManaPool,
    sources: &[ManaOption],
    life: i32,
) -> Vec<FaceOption> {
    if !sorcery_timing(state, player) {
        return Vec::new();
    }
    dk1c5_locked(state, player)
        .into_iter()
        .filter_map(|(id, door)| {
            let face = dk1c5_door(state, id, door)?;
            let cost = face.mana_cost.clone().unwrap_or_default();
            can_afford(
                &cost,
                pool,
                &for_card(state, sources, id),
                life,
                &Spend::other(),
            )
            .then(|| FaceOption {
                card: id,
                face: door,
                name: face.name.clone(),
                cost: Some(cost),
                land: false,
            })
        })
        .collect()
}

/// The event of `player` unlocking `door` of `id`, while that door is locked and the Room theirs.
pub(crate) fn dk1c5_events(
    state: &GameState,
    player: PlayerId,
    id: ObjectId,
    door: u8,
) -> Option<Vec<Event>> {
    dk1c5_locked(state, player).contains(&(id, door)).then(|| {
        vec![Event::DoorUnlocked {
            object: id,
            door,
            player,
        }]
    })
}
