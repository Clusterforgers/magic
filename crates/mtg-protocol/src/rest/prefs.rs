//! Body of the account preference endpoints of `mtg-server`: `GET /api/me/prefs` and `PUT /api/me/prefs`.

use mtg_engine::turn::Step;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::message::{AutoChoices, AutoYield, Stop, StopSide};

/// The most default stops an account keeps.
pub const MAX_DEFAULT_STOPS: usize = 40;

/// The most abilities an account always yields to.
pub const MAX_AUTO_YIELDS: usize = 200;

/// The pinned stops of an account that saved none: its own declare attackers step and its postcombat main phase.
#[must_use]
pub fn default_stops() -> Vec<Stop> {
    [Step::DeclareAttackers, Step::Main2]
        .into_iter()
        .map(|step| Stop {
            side: StopSide::Own,
            step,
            keep: true,
        })
        .collect()
}

/// The pace preferences of an account, kept on the server and applied to every game it plays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct UserPrefs {
    /// The lasting stops a new game starts with.
    pub default_stops: Vec<Stop>,
    /// Whether the account's seats end their own turns by hand.
    pub end_turn_by_hand: bool,
    /// The decisions the server answers for the account's seats; off when absent.
    #[serde(default)]
    pub auto_choices: AutoChoices,
    /// The abilities the account's seats always yield to; sorted, each once.
    #[serde(default)]
    pub auto_yields: Vec<AutoYield>,
    /// How the hand dock orders the hand in every game; "as drawn" when absent.
    #[serde(default)]
    pub hand_sort: HandSort,
}

impl Default for UserPrefs {
    fn default() -> Self {
        Self {
            default_stops: default_stops(),
            end_turn_by_hand: false,
            auto_choices: AutoChoices::default(),
            auto_yields: Vec::new(),
            hand_sort: HandSort::default(),
        }
    }
}

/// How the hand dock orders a hand the player did not drag: by the order drawn, by mana value, or by groups of card
/// types in the account's order (each group by mana value, then name).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
pub enum HandSortBy {
    /// The order the cards came into the hand.
    #[default]
    Drawn,
    /// Mana value, then name.
    ManaValue,
    /// The groups of `HandSort::groups`, in that order.
    Groups,
}

/// A group of the hand by card type; a card of several types joins the first of its groups in the account's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
pub enum HandGroup {
    /// Lands.
    Land,
    /// Creatures.
    Creature,
    /// Artifacts.
    Artifact,
    /// Enchantments.
    Enchantment,
    /// Planeswalkers.
    Planeswalker,
    /// Battles.
    Battle,
    /// Instants.
    Instant,
    /// Sorceries.
    Sorcery,
}

/// Every hand group in the order an account starts with.
pub const HAND_GROUPS: [HandGroup; 8] = [
    HandGroup::Land,
    HandGroup::Creature,
    HandGroup::Artifact,
    HandGroup::Enchantment,
    HandGroup::Planeswalker,
    HandGroup::Battle,
    HandGroup::Instant,
    HandGroup::Sorcery,
];

/// The account's auto sort of the hand, the same in every game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct HandSort {
    /// What the sort orders by.
    pub by: HandSortBy,
    /// The groups in the account's order, each once; a group left out follows the named ones in the starting order.
    pub groups: Vec<HandGroup>,
}

impl Default for HandSort {
    fn default() -> Self {
        Self {
            by: HandSortBy::Drawn,
            groups: HAND_GROUPS.to_vec(),
        }
    }
}
