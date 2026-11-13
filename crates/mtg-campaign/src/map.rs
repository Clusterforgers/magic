//! The boss ladder: the theme's lieutenants in an order drawn from the run's seed, then its final boss, with a camp
//! row between two bosses where the party votes for one of two camps (treasure, shop, event or campfire).

use serde::{Deserialize, Serialize};

use crate::content::Theme;
use crate::rng::Stream;

/// What a room of the ladder holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum RoomKind {
    /// A boss fight.
    Boss,
    /// A relic pick for each hero.
    Treasure,
    /// Cards, relics and healing for gold.
    Shop,
    /// A story choice for each hero.
    Event,
    /// Rest (life) or reforge (a card pick).
    Campfire,
}

/// The kinds a camp may be.
const CAMPS: [RoomKind; 4] = [
    RoomKind::Treasure,
    RoomKind::Shop,
    RoomKind::Event,
    RoomKind::Campfire,
];

/// One room of the ladder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Node {
    /// Its index in the map.
    pub id: u8,
    /// Its row, from 0 at the first boss.
    pub row: u8,
    /// Its lane: 0 for a boss, 0 or 1 for a camp.
    pub lane: u8,
    /// What it holds.
    pub kind: RoomKind,
    /// The boss of a boss room.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boss: Option<String>,
    /// The rooms it leads to.
    pub next: Vec<u8>,
}

/// A generated ladder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FloorMap {
    /// The rooms, row by row; the final boss last.
    pub nodes: Vec<Node>,
}

impl FloorMap {
    /// The room with this id.
    pub fn node(&self, id: u8) -> Option<&Node> {
        self.nodes.get(usize::from(id))
    }

    /// The rooms the party may enter next from `at` (`None` before the first room).
    pub fn choices(&self, at: Option<u8>) -> Vec<u8> {
        match at {
            None => self
                .nodes
                .iter()
                .filter(|n| n.row == 0)
                .map(|n| n.id)
                .collect(),
            Some(id) => self.node(id).map(|n| n.next.clone()).unwrap_or_default(),
        }
    }

    /// The final boss room.
    pub fn last(&self) -> Option<&Node> {
        self.nodes.last()
    }
}

/// The ladder of `theme` for `seed`.
pub fn generate(theme: &Theme, seed: u64) -> FloorMap {
    let mut rng = Stream::new(seed, "map", 0, 0);
    let lieutenants: Vec<&str> = theme
        .bosses
        .iter()
        .filter(|b| !b.final_boss)
        .map(|b| b.id.as_str())
        .collect();
    let mut order = rng.pick(&lieutenants, lieutenants.len());
    order.extend(
        theme
            .bosses
            .iter()
            .filter(|b| b.final_boss)
            .map(|b| b.id.as_str()),
    );
    let mut nodes: Vec<Node> = Vec::new();
    let mut row = 0u8;
    for (at, boss) in order.iter().enumerate() {
        let id = u8::try_from(nodes.len()).unwrap_or(u8::MAX);
        // The camps of the row before lead here.
        for node in nodes.iter_mut().filter(|n| n.row + 1 == row) {
            node.next.push(id);
        }
        nodes.push(Node {
            id,
            row,
            lane: 0,
            kind: RoomKind::Boss,
            boss: Some((*boss).to_owned()),
            next: Vec::new(),
        });
        row += 1;
        if at + 1 == order.len() {
            break;
        }
        let camps = rng.pick(&CAMPS, 2);
        for (lane, kind) in camps.into_iter().enumerate() {
            let camp = u8::try_from(nodes.len()).unwrap_or(u8::MAX);
            if let Some(boss_node) = nodes.get_mut(usize::from(id)) {
                boss_node.next.push(camp);
            }
            nodes.push(Node {
                id: camp,
                row,
                lane: u8::try_from(lane).unwrap_or(0),
                kind,
                boss: None,
                next: Vec::new(),
            });
        }
        row += 1;
    }
    FloorMap { nodes }
}

#[cfg(test)]
mod tests {
    //! Goal: every ladder alternates bosses and camp pairs, meets each boss once with the final boss last, and is the
    //! same for the same seed.
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn ladders_follow_the_rules(seed in any::<u64>()) {
            let theme = crate::content().map_err(|e| TestCaseError::fail(e.to_string()))?.themes[0].clone();
            let map = generate(&theme, seed);
            prop_assert_eq!(&map, &generate(&theme, seed));
            let bosses: Vec<&str> = map.nodes.iter().filter_map(|n| n.boss.as_deref()).collect();
            prop_assert_eq!(bosses.len(), theme.bosses.len());
            let finals: Vec<&str> = theme.bosses.iter().filter(|b| b.final_boss).map(|b| b.id.as_str()).collect();
            prop_assert_eq!(map.last().and_then(|n| n.boss.as_deref()), finals.first().copied());
            let mut sorted = bosses.clone();
            sorted.sort_unstable();
            sorted.dedup();
            prop_assert_eq!(sorted.len(), bosses.len());
            for node in &map.nodes {
                prop_assert_eq!(node.kind == RoomKind::Boss, node.row % 2 == 0);
                prop_assert_eq!(map.node(node.id), Some(node));
                for next in &node.next {
                    prop_assert_eq!(map.node(*next).map(|n| n.row), Some(node.row + 1));
                }
                if node.id + 1 < u8::try_from(map.nodes.len()).unwrap_or(0) {
                    prop_assert!(!node.next.is_empty());
                }
                if node.kind == RoomKind::Boss && !node.next.is_empty() {
                    let kinds: Vec<RoomKind> = node.next.iter().filter_map(|id| map.node(*id)).map(|n| n.kind).collect();
                    prop_assert_eq!(kinds.len(), 2);
                    prop_assert_ne!(kinds.first(), kinds.get(1));
                }
            }
        }
    }
}
