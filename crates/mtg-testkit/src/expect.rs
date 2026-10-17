//! Assertions on a [`Driver`] that return errors instead of panicking.

use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::player::Player;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;

use crate::driver::Driver;
use crate::script_error::ScriptError;

impl Driver {
    /// Asserts a player's life total.
    pub fn expect_life(&self, player: usize, life: i32) -> Result<(), ScriptError> {
        let actual = self
            .state()
            .player(PlayerId::from_index(player))
            .map(Player::life);
        expectation(actual == Some(life), || {
            format!("player {player} life is {actual:?}, expected {life}")
        })
    }

    /// Asserts the names in a zone, ignoring order.
    pub fn expect_zone(&self, zone: Zone, names: &[&str]) -> Result<(), ScriptError> {
        let mut actual: Vec<&str> = self
            .state()
            .zone(zone)
            .iter()
            .map(|id| self.name_of(*id))
            .collect();
        let mut expected = names.to_vec();
        actual.sort_unstable();
        expected.sort_unstable();
        expectation(actual == expected, || {
            format!("{zone:?} holds {actual:?}, expected {expected:?}")
        })
    }

    /// Asserts the current step.
    pub fn expect_step(&self, step: Step) -> Result<(), ScriptError> {
        let actual = self.state().turn().step();
        expectation(actual == step, || {
            format!("step is {actual:?}, expected {step:?}")
        })
    }

    /// Asserts a decision of `kind` is pending for `player`.
    pub fn expect_decision(&self, player: usize, kind: &str) -> Result<(), ScriptError> {
        let decision = self.decision()?;
        expectation(
            decision.player == PlayerId::from_index(player) && decision.kind.name() == kind,
            || {
                format!(
                    "pending {} for {:?}, expected {kind} for player {player}",
                    decision.kind.name(),
                    decision.player
                )
            },
        )
    }

    /// Asserts some event in the whole log satisfies `matches`.
    pub fn expect_event(
        &self,
        what: &str,
        matches: impl Fn(&Event) -> bool,
    ) -> Result<(), ScriptError> {
        expectation(self.engine.events().iter().any(matches), || {
            format!("no event matching {what}")
        })
    }

    /// Asserts no event in the whole log satisfies `matches`.
    pub fn expect_no_event(
        &self,
        what: &str,
        matches: impl Fn(&Event) -> bool,
    ) -> Result<(), ScriptError> {
        expectation(!self.engine.events().iter().any(matches), || {
            format!("unexpected event matching {what}")
        })
    }
}

fn expectation(holds: bool, message: impl FnOnce() -> String) -> Result<(), ScriptError> {
    if holds {
        Ok(())
    } else {
        Err(ScriptError::Expectation(message()))
    }
}
