//! Zones and the ordered containers that hold object ids.

use mtg_core::ids::{ObjectId, PlayerId};
use serde::{Deserialize, Serialize};

/// A game zone; per-player zones carry their owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Zone {
    /// A player's library; the last id is the top card.
    Library(PlayerId),
    /// A player's hand.
    Hand(PlayerId),
    /// The shared battlefield.
    Battlefield,
    /// A player's graveyard; the last id is the top card.
    Graveyard(PlayerId),
    /// The shared stack; the last id is the top object.
    Stack,
    /// The shared exile zone.
    Exile,
    /// A player's command zone (Commander extension point; unused by Normal).
    Command(PlayerId),
}

/// Where in an ordered zone an object is inserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Placement {
    /// On top (the end of the container).
    Top,
    /// On the bottom (the start of the container).
    Bottom,
}

/// Ordered id lists for every zone; the owning [`GameObject`](crate::state::GameObject) records its zone too.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Zones {
    library: Vec<Vec<ObjectId>>,
    hand: Vec<Vec<ObjectId>>,
    graveyard: Vec<Vec<ObjectId>>,
    command: Vec<Vec<ObjectId>>,
    battlefield: Vec<ObjectId>,
    stack: Vec<ObjectId>,
    exile: Vec<ObjectId>,
}

impl Zones {
    pub(crate) fn new(players: usize) -> Self {
        let per_player = || vec![Vec::new(); players];
        Zones {
            library: per_player(),
            hand: per_player(),
            graveyard: per_player(),
            command: per_player(),
            battlefield: Vec::new(),
            stack: Vec::new(),
            exile: Vec::new(),
        }
    }

    /// Contents of `zone` (empty for an unknown player).
    pub fn get(&self, zone: Zone) -> &[ObjectId] {
        match zone {
            Zone::Library(p) => by_player(&self.library, p),
            Zone::Hand(p) => by_player(&self.hand, p),
            Zone::Graveyard(p) => by_player(&self.graveyard, p),
            Zone::Command(p) => by_player(&self.command, p),
            Zone::Battlefield => &self.battlefield,
            Zone::Stack => &self.stack,
            Zone::Exile => &self.exile,
        }
    }

    fn get_mut(&mut self, zone: Zone) -> Option<&mut Vec<ObjectId>> {
        match zone {
            Zone::Library(p) => self.library.get_mut(p.index()),
            Zone::Hand(p) => self.hand.get_mut(p.index()),
            Zone::Graveyard(p) => self.graveyard.get_mut(p.index()),
            Zone::Command(p) => self.command.get_mut(p.index()),
            Zone::Battlefield => Some(&mut self.battlefield),
            Zone::Stack => Some(&mut self.stack),
            Zone::Exile => Some(&mut self.exile),
        }
    }

    /// Whether `zone` exists for the seats in this game.
    pub(crate) fn exists(&self, zone: Zone) -> bool {
        match zone {
            Zone::Library(p) => p.index() < self.library.len(),
            Zone::Hand(p) => p.index() < self.hand.len(),
            Zone::Graveyard(p) => p.index() < self.graveyard.len(),
            Zone::Command(p) => p.index() < self.command.len(),
            Zone::Battlefield | Zone::Stack | Zone::Exile => true,
        }
    }

    /// Inserts `id`; false if the zone does not exist.
    pub(crate) fn insert(&mut self, zone: Zone, id: ObjectId, placement: Placement) -> bool {
        match self.get_mut(zone) {
            Some(list) => {
                match placement {
                    Placement::Top => list.push(id),
                    Placement::Bottom => list.insert(0, id),
                }
                true
            }
            None => false,
        }
    }

    /// Removes `id`; false if it was not there.
    pub(crate) fn remove(&mut self, zone: Zone, id: ObjectId) -> bool {
        let Some(list) = self.get_mut(zone) else {
            return false;
        };
        match list.iter().position(|o| *o == id) {
            Some(pos) => {
                list.remove(pos);
                true
            }
            None => false,
        }
    }

    /// Replaces the contents of a per-player or shared zone wholesale.
    pub(crate) fn set(&mut self, zone: Zone, ids: Vec<ObjectId>) -> bool {
        match self.get_mut(zone) {
            Some(list) => {
                *list = ids;
                true
            }
            None => false,
        }
    }

    /// Every zone that exists for `players` seats, in a fixed order.
    pub fn all_zones(players: usize) -> impl Iterator<Item = Zone> {
        let per_player = (0..players).flat_map(|p| {
            let p = PlayerId::from_index(p);
            [
                Zone::Library(p),
                Zone::Hand(p),
                Zone::Graveyard(p),
                Zone::Command(p),
            ]
        });
        per_player.chain([Zone::Battlefield, Zone::Stack, Zone::Exile])
    }
}

fn by_player(lists: &[Vec<ObjectId>], player: PlayerId) -> &[ObjectId] {
    lists.get(player.index()).map_or(&[], Vec::as_slice)
}

#[cfg(test)]
mod tests {
    //! Goal: ordered zone containers insert, remove and order objects as the zone conventions promise.
    use super::*;
    use rstest::rstest;

    fn ids(zones: &Zones, zone: Zone) -> Vec<u64> {
        zones.get(zone).iter().map(|o| o.0).collect()
    }

    #[rstest]
    #[case(Zone::Library(PlayerId(1)))]
    #[case(Zone::Hand(PlayerId(0)))]
    #[case(Zone::Graveyard(PlayerId(1)))]
    #[case(Zone::Command(PlayerId(0)))]
    #[case(Zone::Battlefield)]
    #[case(Zone::Stack)]
    #[case(Zone::Exile)]
    fn top_is_the_end_and_bottom_the_start(#[case] zone: Zone) {
        let mut zones = Zones::new(2);
        assert!(zones.insert(zone, ObjectId(1), Placement::Top));
        assert!(zones.insert(zone, ObjectId(2), Placement::Top));
        assert!(zones.insert(zone, ObjectId(3), Placement::Bottom));
        assert_eq!(ids(&zones, zone), vec![3, 1, 2]);
        assert!(zones.remove(zone, ObjectId(1)));
        assert!(!zones.remove(zone, ObjectId(1)));
        assert_eq!(ids(&zones, zone), vec![3, 2]);
    }

    #[test]
    fn unknown_players_have_no_zones() {
        let mut zones = Zones::new(2);
        let ghost = Zone::Hand(PlayerId(5));
        assert_eq!(zones.get(ghost).len(), 0);
        assert!(!zones.insert(ghost, ObjectId(1), Placement::Top));
        assert!(!zones.remove(ghost, ObjectId(1)));
    }

    #[rstest]
    #[case(2, 11)]
    #[case(5, 23)]
    fn zone_enumeration_covers_every_container(#[case] players: usize, #[case] expected: usize) {
        assert_eq!(Zones::all_zones(players).count(), expected);
    }
}
