//! Controlling another player (CR 722): Mindslaver's "You control target player during that player's next turn" and
//! Opposition Agent's "You control your opponents while they're searching their libraries".
//!
//! A control is a fact of the state: `PlayerControlGained` schedules it, the turn of the controlled player that begins
//! next starts it, and the turn after that ends it, so a replay rebuilds it from the log alone. The engine asks every
//! decision of the controlled player as before; [`answerer`] names who answers it, and the engine shows the decision to
//! that player (`Engine::decision`). The controller sees what the controlled player sees (CR 722.3, `sees_as`).

use mtg_core::ids::PlayerId;
use serde::{Deserialize, Serialize};

use crate::decision::{Decision, DecisionKind};
use crate::state::GameState;
use crate::turn::ExtraTurn;
use crate::zones::Zone;

/// One player's control of another during the controlled player's next turn.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlayerControl {
    /// The player controlled.
    pub player: PlayerId,
    /// The player who controls them.
    pub by: PlayerId,
    /// Whether the controlled player takes an extra turn after that turn.
    pub extra_turn: bool,
    /// The number of the turn it applies in, once that turn has begun.
    pub turn: Option<u32>,
}

impl GameState {
    /// The controls of players that are scheduled or under way, oldest first.
    pub fn player_controls(&self) -> &[PlayerControl] {
        &self.player_controls
    }

    /// Who controls `player` now: the controller of the latest control under way this turn that is still in the game,
    /// or the player themselves (CR 722.1).
    pub fn player_controller(&self, player: PlayerId) -> PlayerId {
        self.player_controls
            .iter()
            .rev()
            .find(|control| {
                control.player == player
                    && control.turn == Some(self.turn.number)
                    && self.is_alive(control.by)
            })
            .map_or(player, |control| control.by)
    }

    /// The players whose cards `viewer` sees as their own: `viewer`, and each player they control now (CR 722.3).
    pub fn sees_as(&self, viewer: PlayerId) -> Vec<PlayerId> {
        let mut seen = vec![viewer];
        for control in &self.player_controls {
            if !seen.contains(&control.player) && self.player_controller(control.player) == viewer {
                seen.push(control.player);
            }
        }
        seen
    }

    /// Whether `viewer` sees the cards of `player` as `player` does: the same player, or one `viewer` controls now.
    pub fn sees_for(&self, viewer: PlayerId, player: PlayerId) -> bool {
        viewer == player || self.player_controller(player) == viewer
    }
}

/// Who answers `decision`: the player who controls its player now, or, for a choice among the cards of the player's
/// own library (a search), an opponent with a rule that controls players while they search (CR 722, Opposition Agent).
pub fn answerer(state: &GameState, decision: &Decision) -> PlayerId {
    let player = state.player_controller(decision.player);
    if player != decision.player {
        return player;
    }
    if searching_own_library(state, decision)
        && let Some(by) = state.search_controller(decision.player)
    {
        return by;
    }
    player
}

/// Whether the decision is a choice among cards of its player's own library.
fn searching_own_library(state: &GameState, decision: &Decision) -> bool {
    let DecisionKind::ChooseCards(options) = &decision.kind else {
        return false;
    };
    !options.from.is_empty()
        && options.from.iter().all(|id| {
            state
                .object(*id)
                .is_some_and(|object| object.zone() == Zone::Library(decision.player))
        })
}

/// Schedules the control (`PlayerControlGained`).
pub(crate) fn gain(state: &mut GameState, player: PlayerId, by: PlayerId, extra_turn: bool) {
    state.player_controls.push(PlayerControl {
        player,
        by,
        extra_turn,
        turn: None,
    });
}

/// A turn of `active` numbered `number` begins: the controls of earlier turns end and those waiting for this player's
/// next turn begin, each that asks for it adding the extra turn taken after this one.
pub(crate) fn turn_began(state: &mut GameState, number: u32, active: PlayerId) {
    if state.player_controls.is_empty() {
        return;
    }
    state
        .player_controls
        .retain(|control| control.turn.is_none_or(|turn| turn == number));
    let mut extra = 0;
    for control in &mut state.player_controls {
        if control.player == active && control.turn.is_none() {
            control.turn = Some(number);
            extra += usize::from(control.extra_turn);
        }
    }
    for _ in 0..extra {
        state.extra_turns.push(ExtraTurn {
            player: active,
            skip_untap: false,
            no_power_up: false,
        });
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a control begins with the controlled player's next turn and not before, ends when the turn after begins, the latest control of a player wins, a controller who left controls nobody, and an extra turn asked for follows the controlled turn.
    use super::*;
    use crate::fixtures::state;

    const A: PlayerId = PlayerId(0);
    const B: PlayerId = PlayerId(1);
    const C: PlayerId = PlayerId(2);

    fn game() -> GameState {
        state(vec![vec![], vec![], vec![]], vec![vec![], vec![], vec![]])
    }

    #[test]
    fn a_control_waits_for_the_players_next_turn_and_ends_after_it() {
        let mut game = game();
        game.turn.number = 4;
        gain(&mut game, B, A, false);
        assert_eq!(game.player_controller(B), B);
        turn_began(&mut game, 5, C);
        assert_eq!(game.player_controller(B), B);
        game.turn.number = 6;
        turn_began(&mut game, 6, B);
        assert_eq!(game.player_controller(B), A);
        assert_eq!(game.sees_as(A), vec![A, B]);
        assert!(game.sees_for(A, B) && !game.sees_for(C, B));
        game.turn.number = 7;
        turn_began(&mut game, 7, C);
        assert_eq!(game.player_controller(B), B);
        assert_eq!(game.player_controls().len(), 0);
    }

    #[test]
    fn the_latest_control_wins_and_an_extra_turn_follows() {
        let mut game = game();
        gain(&mut game, B, A, false);
        gain(&mut game, B, C, true);
        game.turn.number = 2;
        turn_began(&mut game, 2, B);
        assert_eq!(game.player_controller(B), C);
        assert_eq!(game.extra_turns.len(), 1);
        assert_eq!(game.extra_turns[0].player, B);
    }
}
