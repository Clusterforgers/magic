//! The opponents the controller attacked this turn, read from the turn's log of attacks (`Ref::AttackedThisTurn`), and
//! the opponents of named players (`Ref::OpponentsOf`).

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

    /// Whether a creature the controller attacked with this turn attacked one of the players `who` names, directly or
    /// through a planeswalker they controlled as it was attacked (`Condition::R123AttackedThisTurn`).
    pub(super) fn r123_attacked_this_turn(
        &self,
        who: &mtg_cards::refs::Ref,
    ) -> Result<bool, super::ResolveError> {
        let named = self.players(who)?;
        Ok(self.state.tallies.records.iter().any(|record| {
            record.event == TallyEvent::Attacked
                && record.player == Some(self.controller)
                && match record.other {
                    Some(Target::Player(player)) => named.contains(&player),
                    Some(Target::Object(_)) => record.other_shot.as_ref().is_some_and(|shot| {
                        shot.characteristics
                            .types
                            .contains(mtg_core::card::CardType::Planeswalker)
                            && named.contains(&shot.controller)
                    }),
                    None => false,
                }
        }))
    }

    /// The opponents of the players `inner` names, in turn order (`Ref::OpponentsOf`).
    pub(super) fn r5523_opponents_of(
        &self,
        inner: &mtg_cards::refs::Ref,
    ) -> Result<Vec<Target>, super::ResolveError> {
        let of = self.players(inner)?;
        let state = self.state;
        Ok(state
            .turn_order_from(state.turn().active())
            .into_iter()
            .filter(|player| of.iter().any(|named| state.is_opponent(*named, *player)))
            .map(Target::Player)
            .collect())
    }
}
