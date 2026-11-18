//! A seat's opening hand rule (a Delves boss's): its opening hand holds a land count in a range and it takes no
//! mulligan. A dealt hand outside the range is fixed from the top of the library: the bottom-most cards of the wrong
//! kind in the hand are swapped for the topmost cards of the right kind in the library, then the rest of the library is
//! shuffled with the game's seeded stream, so a game replays from its seed.

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};

use mtg_core::card::CardType;
use mtg_core::ids::{ObjectId, PlayerId};

use crate::state::GameState;

/// The land count a seat's opening hand must hold, both ends included.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OpeningLands {
    /// The fewest lands.
    pub least: u32,
    /// The most lands.
    pub most: u32,
}

impl OpeningLands {
    /// A Delves boss's hand: two to four lands.
    pub const BOSS: OpeningLands = OpeningLands { least: 2, most: 4 };
}

/// The opening hand rules of the seats that have one, fixed when the game is set up.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct OpeningRules(BTreeMap<PlayerId, OpeningLands>);

impl Hash for OpeningRules {
    fn hash<H: Hasher>(&self, state: &mut H) {
        if !self.0.is_empty() {
            self.0.hash(state);
        }
    }
}

impl OpeningRules {
    /// The rules of a game set up with these seats' ranges.
    pub(crate) fn of(seats: impl IntoIterator<Item = (PlayerId, OpeningLands)>) -> OpeningRules {
        OpeningRules(seats.into_iter().collect())
    }
}

impl GameState {
    /// The land count `player`'s opening hand must hold, when the seat has the rule; such a seat takes no mulligan.
    pub fn opening_lands(&self, player: PlayerId) -> Option<OpeningLands> {
        self.opening_rules.0.get(&player).copied()
    }
}

/// The hand `hand` (the top cards of `library`, which is listed top first) with its land count brought into `range`:
/// while it holds too few lands, its bottom-most nonland card is swapped for the topmost land left in the library, and
/// the other way round for too many; a library out of the right kind stops the fix. `None` when the hand is in range.
pub(crate) fn fixed(
    state: &GameState,
    range: OpeningLands,
    hand: &[ObjectId],
    library: &[ObjectId],
) -> Option<Vec<ObjectId>> {
    let is_land = |card: &ObjectId| {
        state
            .object(*card)
            .is_some_and(|object| object.base_types().contains(CardType::Land))
    };
    let lands = u32::try_from(hand.iter().filter(|card| is_land(card)).count()).unwrap_or(u32::MAX);
    let (short, want_land) = if lands < range.least {
        (range.least - lands, true)
    } else if lands > range.most {
        (lands - range.most, false)
    } else {
        return None;
    };
    let mut hand = hand.to_vec();
    let mut rest = library
        .iter()
        .copied()
        .filter(|card| !hand.contains(card) && is_land(card) == want_land)
        .collect::<Vec<_>>()
        .into_iter();
    for _ in 0..short {
        let Some(out) = hand.iter().rposition(|card| is_land(card) != want_land) else {
            break;
        };
        let Some(swap) = rest.next() else {
            break;
        };
        hand[out] = swap;
    }
    Some(hand)
}
