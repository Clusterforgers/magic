//! A requirement to attack a planeswalker: "During target opponent's next turn, creatures that player controls attack ~
//! if able." (`Duration::TheirNextTurn`) and "Up to one target creature an opponent controls attacks ~ during its
//! controller's next turn if able." (`Duration::DuringItsControllersNextTurn`).

use super::{Effect, Ref, Writer};
use crate::modify::{Duration, GameRule};

impl Writer<'_> {
    /// The sentence of a rule that makes creatures attack this planeswalker during a player's next turn.
    pub(super) fn r5501_attack_at(&mut self, effect: &Effect) -> Option<String> {
        let Effect::Rule {
            rule:
                GameRule::MustAttackAt {
                    who,
                    of,
                    at: Ref::This,
                },
            duration,
        } = effect
        else {
            return None;
        };
        match (who, of, duration) {
            (None, Some(players), Duration::TheirNextTurn) => {
                let player = self.player(players)?;
                // The player named first is "that player" after it.
                let that = if player.starts_with("target ") {
                    "that player".to_owned()
                } else {
                    return None;
                };
                Some(format!(
                    "during {player}'s next turn, creatures {that} controls attack ~ if able"
                ))
            }
            (Some(creatures), None, Duration::DuringItsControllersNextTurn) => {
                let creature = self.object(creatures)?;
                Some(format!(
                    "{creature} attacks ~ during its controller's next turn if able"
                ))
            }
            _ => None,
        }
    }
}
