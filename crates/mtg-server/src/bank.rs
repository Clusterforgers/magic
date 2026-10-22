//! The timer bank: the extensions a seat earns by not using its clocks and spends when a clock runs out.
//!
//! A seat that finishes [`BANK_TURNS`] turns of its own in a row without a clock running out on it earns one
//! extension, up to [`BANK_MAX`]. The bank is public and depends on the clocks alone, never on the cards. It is
//! kept in memory with the running game and starts empty when a game is resumed.

use mtg_core::ids::PlayerId;
use mtg_engine::GameState;
use mtg_protocol::TableView;

use crate::pace::{BANK_MAX, BANK_TURNS};

/// One seat's extensions and the turns it needs for the next.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Account {
    /// Extensions held.
    held: u8,
    /// Own turns finished in a row without a clock running out since the last extension.
    clean: u8,
    /// Whether a clock ran out during the turn the seat is playing now, so the turn does not count.
    dirty: bool,
}

/// The extensions of every seat and the turn last seen, which tells when a turn ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Banks {
    /// Whether extensions are earned at all (`MTG_EXTENSION_SECS` is not 0).
    on: bool,
    accounts: Vec<Account>,
    /// Number and active seat of the turn last seen.
    turn: Option<(u32, PlayerId)>,
}

impl Banks {
    /// Empty banks for `seats` seats, earning only when `on`.
    pub(crate) fn new(seats: usize, on: bool) -> Banks {
        Banks {
            on,
            accounts: vec![Account::default(); seats],
            turn: None,
        }
    }

    /// The extensions `seat` holds.
    pub(crate) fn held(&self, seat: PlayerId) -> u8 {
        self.accounts.get(seat.index()).map_or(0, |a| a.held)
    }

    /// Writes every seat's extensions into the players of `table`.
    pub(crate) fn stamp(&self, table: &mut TableView) {
        for player in &mut table.players {
            player.bank = self.held(player.seat);
        }
    }

    /// Notes the turn `state` is in: when it is a new one, the seat whose turn just ended is credited with it.
    ///
    /// Call after every change of the engine, so no turn goes by unseen.
    pub(crate) fn watch(&mut self, state: &GameState) {
        let turn = state.turn();
        let now = (turn.number(), turn.active());
        if let Some((number, seat)) = self.turn.replace(now)
            && number != now.0
        {
            self.finished(seat, state.is_alive(seat));
        }
    }

    /// `seat` finished a turn of its own; a seat that has left the game earns nothing more.
    pub(crate) fn finished(&mut self, seat: PlayerId, alive: bool) {
        let on = self.on;
        let Some(account) = self.accounts.get_mut(seat.index()) else {
            return;
        };
        if !std::mem::take(&mut account.dirty) && alive && on {
            account.clean += 1;
            if account.clean >= BANK_TURNS {
                account.clean = 0;
                account.held = (account.held + 1).min(BANK_MAX);
            }
        }
    }

    /// A clock of `seat` reached its deadline while `active` was playing: the run of clean turns starts over, and the
    /// turn the seat is playing does not count.
    pub(crate) fn ran_out(&mut self, seat: PlayerId, active: Option<PlayerId>) {
        if let Some(account) = self.accounts.get_mut(seat.index()) {
            account.clean = 0;
            account.dirty |= active == Some(seat);
        }
    }

    /// Takes one extension from `seat`; `false` when it holds none.
    pub(crate) fn spend(&mut self, seat: PlayerId) -> bool {
        match self.accounts.get_mut(seat.index()) {
            Some(account) if account.held > 0 => {
                account.held -= 1;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a seat earns one extension per three own turns finished without a clock running out, holds at most three, loses the run to any clock that runs out and earns nothing with the bank off.
    use super::*;
    use rstest::rstest;

    const SEAT: PlayerId = PlayerId(1);
    const OTHER: PlayerId = PlayerId(0);

    /// What happens to the seat in a test.
    #[derive(Debug, Clone, Copy)]
    enum Step {
        /// The seat finishes a turn of its own.
        Turn,
        /// The seat finishes a turn of its own after leaving the game.
        TurnGone,
        /// A clock of the seat runs out during its own turn.
        RanOutOwn,
        /// A clock of the seat runs out during another seat's turn.
        RanOutElsewhere,
        /// The seat spends an extension.
        Spend,
    }

    fn play(on: bool, steps: &[Step]) -> (Banks, Vec<bool>) {
        let mut banks = Banks::new(2, on);
        let mut spent = Vec::new();
        for step in steps {
            match step {
                Step::Turn => banks.finished(SEAT, true),
                Step::TurnGone => banks.finished(SEAT, false),
                Step::RanOutOwn => banks.ran_out(SEAT, Some(SEAT)),
                Step::RanOutElsewhere => banks.ran_out(SEAT, Some(OTHER)),
                Step::Spend => spent.push(banks.spend(SEAT)),
            }
        }
        (banks, spent)
    }

    use Step::{RanOutElsewhere, RanOutOwn, Spend, Turn, TurnGone};

    #[rstest]
    #[case::none_yet(&[], 0)]
    #[case::two_turns_are_not_enough(&[Turn, Turn], 0)]
    #[case::three_turns_earn_one(&[Turn, Turn, Turn], 1)]
    #[case::six_earn_two(&[Turn, Turn, Turn, Turn, Turn, Turn], 2)]
    #[case::five_earn_one(&[Turn, Turn, Turn, Turn, Turn], 1)]
    #[case::the_bank_holds_three(&[Turn; 12], 3)]
    #[case::never_more_than_three(&[Turn; 30], 3)]
    #[case::a_clock_in_the_turn_voids_it(&[Turn, Turn, RanOutOwn, Turn], 0)]
    #[case::the_run_starts_over(&[Turn, Turn, RanOutOwn, Turn, Turn, Turn, Turn], 1)]
    #[case::a_clock_in_another_turn_breaks_the_run(&[Turn, Turn, RanOutElsewhere, Turn, Turn], 0)]
    #[case::but_the_next_own_turn_counts(&[Turn, Turn, RanOutElsewhere, Turn, Turn, Turn], 1)]
    #[case::a_seat_that_left_earns_nothing(&[Turn, Turn, TurnGone], 0)]
    #[case::spending_keeps_the_run(&[Turn, Turn, Turn, Spend, Turn], 0)]
    fn extensions_earned(#[case] steps: &[Step], #[case] held: u8) {
        let (banks, _) = play(true, steps);
        assert_eq!(banks.held(SEAT), held);
        assert_eq!(banks.held(OTHER), 0, "the other seat's bank is its own");
    }

    #[test]
    fn a_bank_off_earns_nothing() {
        let (banks, _) = play(false, &[Turn; 12]);
        assert_eq!(banks.held(SEAT), 0);
    }

    #[test]
    fn extensions_are_spent_one_at_a_time_until_none_is_left() {
        let mut steps = vec![Turn; 9];
        steps.extend([Spend, Spend, Spend, Spend]);
        let (banks, spent) = play(true, &steps);
        assert_eq!(spent, [true, true, true, false]);
        assert_eq!(banks.held(SEAT), 0);
    }

    #[test]
    fn a_seat_outside_the_table_holds_nothing() {
        let mut banks = Banks::new(2, true);
        banks.finished(PlayerId(7), true);
        banks.ran_out(PlayerId(7), None);
        assert!(!banks.spend(PlayerId(7)));
        assert_eq!(banks.held(PlayerId(7)), 0);
    }
}
