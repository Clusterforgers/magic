//! What happened since a player's last turn ended (`Pred::EnteredSinceLastTurn`, `Condition::CombatDamagedSinceLastTurn`):
//! the number of the latest turn each player finished, the turn each permanent entered in and the latest turn each player
//! was dealt combat damage in. The fold keeps it from the turn, zone and damage events.

use std::collections::BTreeMap;

use mtg_core::ids::PlayerId;

use crate::object::ObjectRef;
use crate::state::GameState;
use crate::zones::Zone;

/// The history kept for "since your last turn".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Since {
    /// The number of the latest turn each player finished.
    ended: BTreeMap<PlayerId, u32>,
    /// The turn each permanent on the battlefield entered in.
    entered: BTreeMap<ObjectRef, u32>,
    /// The latest turn each player was dealt combat damage in.
    hurt: BTreeMap<PlayerId, u32>,
}

impl Since {
    /// Whether the permanent entered after the latest turn of `player` ended; any entry counts before their first turn,
    /// and a permanent the game began with entered before every turn.
    pub(crate) fn entered_since(&self, permanent: ObjectRef, player: PlayerId) -> bool {
        let entered = self.entered.get(&permanent).copied().unwrap_or(0);
        self.ended.get(&player).is_none_or(|ended| entered > *ended)
    }

    /// Whether `player` was dealt combat damage after their latest turn ended (in any turn, before their first).
    pub(crate) fn hurt_since(&self, player: PlayerId) -> bool {
        let ended = self.ended.get(&player).copied().unwrap_or(0);
        self.hurt.get(&player).is_some_and(|turn| *turn > ended)
    }
}

impl GameState {
    /// A permanent entered the battlefield this turn.
    pub(crate) fn r379_entered(&mut self, permanent: ObjectRef) {
        let turn = self.turn.number;
        self.r379_since.entered.insert(permanent, turn);
    }

    /// A player was dealt combat damage this turn.
    pub(crate) fn r379_combat_hurt(&mut self, player: PlayerId) {
        let turn = self.turn.number;
        self.r379_since.hurt.insert(player, turn);
    }

    /// The turn `number` of `player` ended; the entries of permanents that have left are let go.
    pub(crate) fn r379_turn_ended(&mut self, player: PlayerId, number: u32) {
        self.r379_since.ended.insert(player, number);
        let on_battlefield: Vec<ObjectRef> = self
            .zone(Zone::Battlefield)
            .iter()
            .filter_map(|id| self.object(*id).map(crate::object::GameObject::reference))
            .collect();
        self.r379_since
            .entered
            .retain(|permanent, _| on_battlefield.contains(permanent));
    }
}

#[cfg(test)]
mod tests {
    //! Goal: an entry or combat damage counts only after the player's latest finished turn, and anything counts before
    //! their first turn.
    use rstest::rstest;

    use super::*;
    use mtg_core::ids::ObjectId;

    fn since(ended: Option<u32>, entered: u32, hurt: Option<u32>) -> Since {
        let player = PlayerId::from_index(0);
        let mut since = Since::default();
        if let Some(ended) = ended {
            since.ended.insert(player, ended);
        }
        since.entered.insert(permanent(), entered);
        if let Some(hurt) = hurt {
            since.hurt.insert(player, hurt);
        }
        since
    }

    fn permanent() -> ObjectRef {
        ObjectRef {
            id: ObjectId(7),
            counter: 1,
        }
    }

    #[rstest]
    #[case::after_the_last_turn(Some(4), 5, Some(5), true, true)]
    #[case::near_miss_during_the_last_turn(Some(4), 4, Some(4), false, false)]
    #[case::near_miss_before_the_last_turn(Some(4), 2, Some(3), false, false)]
    #[case::before_the_first_turn(None, 1, Some(1), true, true)]
    #[case::never_hurt(Some(4), 6, None, true, false)]
    fn only_what_followed_the_last_turn_counts(
        #[case] ended: Option<u32>,
        #[case] entered: u32,
        #[case] hurt: Option<u32>,
        #[case] entered_since: bool,
        #[case] hurt_since: bool,
    ) {
        let since = since(ended, entered, hurt);
        let player = PlayerId::from_index(0);
        assert_eq!(since.entered_since(permanent(), player), entered_since);
        assert_eq!(since.hurt_since(player), hurt_since);
    }
}
