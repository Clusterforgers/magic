//! The opponents the controller attacked this turn, read from the turn's log of attacks (`Ref::AttackedThisTurn`).

use mtg_cards::value::TallyEvent;
use mtg_core::ids::PlayerId;

use super::Scope;
use crate::choose::Target;

impl Scope<'_> {
    /// The opponents of the controller that a creature the controller attacked with this turn attacked, directly or
    /// through a planeswalker they controlled as it was attacked, in turn order.
    pub(super) fn r5523_attacked_this_turn(&self) -> Vec<PlayerId> {
        let attacked: Vec<PlayerId> = self
            .state
            .tallies
            .records
            .iter()
            .filter(|record| {
                record.event == TallyEvent::Attacked && record.player == Some(self.controller)
            })
            .filter_map(|record| match record.other {
                Some(Target::Player(player)) => Some(player),
                Some(Target::Object(_)) => record.other_shot.as_ref().map(|shot| shot.controller),
                None => None,
            })
            .collect();
        self.state
            .turn_order_from(self.state.turn().active())
            .into_iter()
            .filter(|player| {
                self.state.is_opponent(self.controller, *player) && attacked.contains(player)
            })
            .collect()
    }
}
