//! The reference rule for when a table asks a seat for priority, for bot games to be compared with a server.
//!
//! A server decides this from the table, the seat's stops and its Full Control mode (auto-pass off is the `Held`
//! mode); [`Windows`] is the same rule written plainly and kept apart from the server, so that a bot game played on
//! an engine with it and the same game played through a server must write the same log. It judges a priority decision
//! as follows:
//! - A seat in `NextWindow` or `Held` is asked.
//! - A seat that holds a stop at the step, on the side of the turn it is in (`Own` for the seat's turns, `Opponent`
//!   for any other seat's), is asked, whether it can act or not.
//! - Otherwise it is asked only at a default window, and only if it can act: its own main phases, the declare
//!   attackers and declare blockers steps, and any moment an object of another seat is on the stack.
//! - Anywhere else it is passed for.
//!
//! A stop that is not kept goes once the game has left its step, and `NextWindow` ends when its seat passes.

use mtg_core::ids::PlayerId;
use mtg_engine::turn::Step;
use mtg_engine::{Decision, DecisionKind, Event, GameState};

/// Whose turns a stop is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The turns of the seat that holds the stop.
    Own,
    /// The turn of any other seat.
    Opponent,
}

/// A step at which a seat asks to be given priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stop {
    /// Whose turns the stop is for.
    pub side: Side,
    /// The step; one where a player gets priority.
    pub step: Step,
    /// Whether the stop stays once its step has passed.
    pub keep: bool,
}

/// How much of the game a seat plays by hand.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    /// The default windows and the seat's stops decide.
    #[default]
    Off,
    /// The seat is asked at every window until it has passed once, then the mode is `Off`.
    NextWindow,
    /// The seat is asked at every window.
    Held,
}

/// What one seat asked the table for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeatWindows {
    /// The stops, at most one for each side and step.
    pub stops: Vec<Stop>,
    /// The Full Control mode.
    pub mode: Mode,
}

impl SeatWindows {
    /// A seat in `mode` that holds no stop.
    pub fn in_mode(mode: Mode) -> SeatWindows {
        SeatWindows {
            stops: Vec::new(),
            mode,
        }
    }
}

/// The reference rule for a whole table, with the stops and modes as the game goes on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Windows {
    seats: Vec<SeatWindows>,
    /// Whose turn it is and in which step, as the events told it.
    at: (PlayerId, Step),
}

impl Windows {
    /// The rule for a table whose seats asked for `seats`, with the game as `state` shows it.
    pub fn new(seats: Vec<SeatWindows>, state: &GameState) -> Windows {
        let turn = state.turn();
        Windows {
            seats,
            at: (turn.active(), turn.step()),
        }
    }

    /// The mode `seat` is in; `Off` for a seat the table does not have.
    pub fn mode(&self, seat: PlayerId) -> Mode {
        self.seats.get(seat.index()).map_or(Mode::Off, |s| s.mode)
    }

    /// The stops `seat` holds now.
    pub fn stops(&self, seat: PlayerId) -> &[Stop] {
        self.seats
            .get(seat.index())
            .map_or(&[], |s| s.stops.as_slice())
    }

    /// Whether the table passes for the holder of `decision` instead of asking it.
    pub fn passes(&self, state: &GameState, decision: &Decision) -> bool {
        let DecisionKind::Priority(options) = &decision.kind else {
            return false;
        };
        let seat = decision.player;
        if self.mode(seat) != Mode::Off {
            return false;
        }
        let turn = state.turn();
        let own = turn.active() == seat;
        let side = if own { Side::Own } else { Side::Opponent };
        if self
            .stops(seat)
            .iter()
            .any(|stop| stop.side == side && stop.step == turn.step())
        {
            return false;
        }
        let object_to_answer = state.stack().iter().any(|id| {
            state
                .object(*id)
                .is_some_and(|object| object.controller() != seat)
        });
        let default_window = object_to_answer
            || matches!(turn.step(), Step::DeclareAttackers | Step::DeclareBlockers)
            || (own && matches!(turn.step(), Step::Main1 | Step::Main2));
        !(default_window && options.can_act)
    }

    /// Follows the game through `events`: a stop that is not kept goes when the step it is in is left.
    pub fn observe(&mut self, events: &[Event]) {
        for event in events {
            match event {
                Event::TurnBegan { active, .. } => {
                    self.leave();
                    self.at = (*active, Step::Untap);
                }
                Event::StepBegan { step } if *step != self.at.1 => {
                    self.leave();
                    self.at.1 = *step;
                }
                _ => {}
            }
        }
    }

    /// `seat` has passed priority itself: a `NextWindow` mode ends.
    pub fn passed(&mut self, seat: PlayerId) {
        if let Some(held) = self.seats.get_mut(seat.index())
            && held.mode == Mode::NextWindow
        {
            held.mode = Mode::Off;
        }
    }

    /// The game leaves the step it is in: every seat's stops that were for it and are not kept go.
    fn leave(&mut self) {
        let (active, step) = self.at;
        for (index, held) in self.seats.iter_mut().enumerate() {
            let side = if PlayerId::from_index(index) == active {
                Side::Own
            } else {
                Side::Opponent
            };
            held.stops
                .retain(|stop| stop.keep || stop.side != side || stop.step != step);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the reference rule asks and passes exactly as its module doc says, over the turn, the step, the stack, stops, modes and the ability to act, and it follows the game through events to clear stops and end `NextWindow`.
    use super::*;
    use crate::Scenario;
    use crate::cards::instant;
    use mtg_engine::Engine;
    use mtg_engine::decision::{PayChoice, PriorityAction, PriorityOptions};
    use mtg_engine::{Answer, Decision};
    use rstest::rstest;

    const ME: PlayerId = PlayerId(1);
    const FOE: PlayerId = PlayerId(0);

    fn priority(player: PlayerId, can_act: bool) -> Decision {
        Decision {
            player,
            kind: DecisionKind::Priority(Box::new(PriorityOptions {
                playable_lands: Vec::new(),
                castable: Vec::new(),
                mana_abilities: Vec::new(),
                abilities: Vec::new(),
                suspendable: Vec::new(),
                foretellable: Vec::new(),
                can_act,
                can_pass: true,
            })),
        }
    }

    fn engine() -> Engine {
        let setup = Scenario::new(2)
            .hand(0, [instant("Zap", 0, &[])])
            .hand(1, [instant("Zap", 0, &[])])
            .setup()
            .unwrap();
        Engine::new(setup).unwrap()
    }

    fn rule(seats: Vec<SeatWindows>, engine: &Engine) -> Windows {
        Windows::new(seats, engine.state())
    }

    #[rstest]
    #[case::own_main_that_can_act(FOE, true, false)]
    #[case::own_main_that_cannot_act(FOE, false, true)]
    #[case::another_seats_main_that_can_act(ME, true, true)]
    #[case::another_seats_main_that_cannot_act(ME, false, true)]
    fn a_seat_with_no_stop_is_asked_at_a_default_window_only_if_it_can_act(
        #[case] seat: PlayerId,
        #[case] can_act: bool,
        #[case] passes: bool,
    ) {
        // Turn 1, seat 0's first main phase: a default window for seat 0 and none for seat 1.
        let engine = engine();
        let windows = rule(vec![SeatWindows::default(); 2], &engine);
        assert_eq!(
            windows.passes(engine.state(), &priority(seat, can_act)),
            passes
        );
    }

    #[rstest]
    #[case::next_window(Mode::NextWindow)]
    #[case::held(Mode::Held)]
    fn full_control_asks_whatever_the_window_and_the_hand(#[case] mode: Mode) {
        let engine = engine();
        let windows = rule(
            vec![SeatWindows::in_mode(mode), SeatWindows::in_mode(mode)],
            &engine,
        );
        for seat in [FOE, ME] {
            for can_act in [false, true] {
                assert!(!windows.passes(engine.state(), &priority(seat, can_act)));
            }
        }
    }

    #[test]
    fn a_stop_asks_at_its_side_and_step_whether_the_seat_can_act_or_not() {
        let engine = engine();
        let stop = |side| Stop {
            side,
            step: Step::Main1,
            keep: true,
        };
        for (side, asked) in [(Side::Opponent, true), (Side::Own, false)] {
            let mut seat = SeatWindows::default();
            seat.stops.push(stop(side));
            let windows = rule(vec![SeatWindows::default(), seat], &engine);
            for can_act in [false, true] {
                assert_eq!(
                    windows.passes(engine.state(), &priority(ME, can_act)),
                    !asked,
                    "{side:?} {can_act}"
                );
            }
        }
    }

    #[test]
    fn an_object_of_another_seat_on_the_stack_is_a_window_for_a_seat_that_can_act() {
        let mut engine = engine();
        let DecisionKind::Priority(options) = engine.decision().unwrap().kind.clone() else {
            panic!("seat 0 holds priority");
        };
        engine
            .apply(Answer::Priority(PriorityAction::CastSpell(
                options.castable[0],
            )))
            .unwrap();
        engine.apply(Answer::PayMana(PayChoice::Auto)).unwrap();
        let windows = rule(vec![SeatWindows::default(); 2], &engine);
        assert!(!windows.passes(engine.state(), &priority(ME, true)));
        assert!(windows.passes(engine.state(), &priority(ME, false)));
    }

    #[test]
    fn a_decision_that_is_not_priority_is_never_passed_for() {
        let engine = engine();
        let windows = rule(vec![SeatWindows::default(); 2], &engine);
        let blocks = Decision {
            player: ME,
            kind: DecisionKind::DeclareBlockers(mtg_engine::decision::BlockOptions {
                blockers: Vec::new(),
                min_blockers: Vec::new(),
                required: Vec::new(),
            }),
        };
        assert!(!windows.passes(engine.state(), &blocks));
    }

    #[test]
    fn following_a_game_clears_one_turn_stops_once_their_step_is_left_and_ends_next_window_at_a_pass()
     {
        let mut engine = engine();
        let mut seat = SeatWindows::in_mode(Mode::NextWindow);
        seat.stops.push(Stop {
            side: Side::Opponent,
            step: Step::Main1,
            keep: false,
        });
        seat.stops.push(Stop {
            side: Side::Opponent,
            step: Step::End,
            keep: true,
        });
        let mut windows = rule(vec![SeatWindows::default(), seat], &engine);

        let pass = Answer::Priority(PriorityAction::Pass);
        // Seat 0 passes in its first main phase and seat 1 after it: the phase is left.
        let events = engine.apply(pass.clone()).unwrap();
        windows.observe(&events);
        assert_eq!(windows.stops(ME).len(), 2, "the step is still running");
        let events = engine.apply(pass.clone()).unwrap();
        windows.observe(&events);
        windows.passed(ME);
        assert_eq!(windows.mode(ME), Mode::Off);
        assert_eq!(
            windows.stops(ME),
            [Stop {
                side: Side::Opponent,
                step: Step::End,
                keep: true
            }]
        );
    }
}
