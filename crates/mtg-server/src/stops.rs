//! Stops and Full Control: what a seat asked the server to remember about when it wants priority.
//!
//! A stop is a step of a turn at which the seat wants to be given priority; Full Control is a mode in which it
//! wants priority at every step. Both live in the actor's memory per seat, like yields: they are not stored, they
//! survive a reconnect (`Welcome` states them) and a resumed game starts without any. They are only stored and
//! reported for now: nothing that decides when a seat is asked reads them.
//!
//! The rules of a stop are here: it names a step where a player gets priority (never untap or cleanup), a seat holds
//! each side and step once, and setting a held stop again replaces how long it lasts. A refusal is a small
//! `Error{BadMessage}` that leaves everything as it was, so a client never needs a resync after one.

use std::collections::BTreeMap;

use mtg_core::ids::PlayerId;
use mtg_engine::GameState;
use mtg_engine::turn::Step;
use mtg_protocol::{ErrorCode, FullControlMode, ServerMessage, Stop};
use thiserror::Error;

use crate::actor::{GameActor, Phase};
use crate::running::Rejection;
use crate::store::EventStore;

/// Why a stop was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub(crate) enum Refused {
    /// No player gets priority in the step, so a stop there could never be reached.
    #[error("no player gets priority in the {0:?} step, so it cannot hold a stop")]
    NoPriority(Step),
}

/// Whether `stop` can be held at all: players must get priority in its step.
fn check(stop: Stop) -> Result<(), Refused> {
    if stop.step.grants_priority() {
        Ok(())
    } else {
        Err(Refused::NoPriority(stop.step))
    }
}

/// What one seat holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Held {
    /// Each stop with whether it lasts: false until its step has passed once, true until it is cleared.
    stops: BTreeMap<Stop, bool>,
    full_control: FullControlMode,
}

/// The stops and the Full Control mode of every seat of one game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Stops {
    seats: Vec<Held>,
}

impl Stops {
    /// No seat holds a stop, and every seat is out of Full Control.
    pub(crate) fn new(seats: usize) -> Stops {
        Stops {
            seats: vec![Held::default(); seats],
        }
    }

    /// The stops `seat` holds, once each: own side first and each side in turn order.
    pub(crate) fn stops(&self, seat: PlayerId) -> Vec<Stop> {
        self.seats
            .get(seat.index())
            .map(|held| held.stops.keys().copied().collect())
            .unwrap_or_default()
    }

    /// `seat`'s Full Control mode.
    pub(crate) fn full_control(&self, seat: PlayerId) -> FullControlMode {
        self.seats
            .get(seat.index())
            .map(|held| held.full_control)
            .unwrap_or_default()
    }

    /// Gives `seat` the stop `stop`; one it already holds only has its `keep` replaced.
    pub(crate) fn set(&mut self, seat: PlayerId, stop: Stop, keep: bool) -> Result<(), Refused> {
        check(stop)?;
        if let Some(held) = self.seats.get_mut(seat.index()) {
            held.stops.insert(stop, keep);
        }
        Ok(())
    }

    /// Takes the stop `stop` from `seat`; a stop it does not hold is already gone.
    pub(crate) fn clear(&mut self, seat: PlayerId, stop: Stop) -> Result<(), Refused> {
        check(stop)?;
        if let Some(held) = self.seats.get_mut(seat.index()) {
            held.stops.remove(&stop);
        }
        Ok(())
    }

    /// Puts `seat` in Full Control mode `mode`.
    pub(crate) fn set_full_control(&mut self, seat: PlayerId, mode: FullControlMode) {
        if let Some(held) = self.seats.get_mut(seat.index()) {
            held.full_control = mode;
        }
    }

    /// Takes everything from every seat that has left the game, and from all of them once it is over.
    pub(crate) fn drop_finished(&mut self, state: &GameState) {
        let over = state.result().is_some();
        for (seat, held) in self.seats.iter_mut().enumerate() {
            if over || !state.is_alive(PlayerId::from_index(seat)) {
                *held = Held::default();
            }
        }
    }

    /// Takes `new` as the stops and returns what each seat whose stops or mode differ from before is to be told.
    ///
    /// A seat is told its whole list of stops when the list it can see changes (replacing how long a stop lasts
    /// changes nothing it can see), then its mode when that changes; seats in seat order.
    pub(crate) fn replace(&mut self, new: Stops) -> Vec<(PlayerId, ServerMessage)> {
        let old = std::mem::replace(self, new);
        let seats = (0..self.seats.len()).map(PlayerId::from_index);
        let mut told = Vec::new();
        for seat in seats {
            let stops = self.stops(seat);
            if old.stops(seat) != stops {
                told.push((seat, ServerMessage::Stops { stops }));
            }
            let mode = self.full_control(seat);
            if old.full_control(seat) != mode {
                told.push((seat, ServerMessage::FullControl { mode }));
            }
        }
        told
    }
}

impl<S: EventStore> GameActor<S> {
    /// Sets or clears one of `seat`'s stops and tells the seat if the list it holds changed.
    ///
    /// Anything the rules refuse is answered with `Error{BadMessage}` and changes nothing. This takes no
    /// step of the game, so there is nothing to save and nothing to take back.
    pub(crate) fn set_stop(&mut self, seat: PlayerId, stop: Stop, on: bool, keep: bool) {
        if let Some(rejection) = self.out_of_game(seat) {
            self.refuse(seat, rejection);
            return;
        }
        let mut stops = self.stops.clone();
        let done = if on {
            stops.set(seat, stop, keep)
        } else {
            stops.clear(seat, stop)
        };
        match done {
            Ok(()) => self.commit_stops(stops),
            Err(refused) => {
                self.refuse(
                    seat,
                    Rejection::new(ErrorCode::BadMessage, refused.to_string()),
                );
            }
        }
    }

    /// Puts `seat` in Full Control mode `mode` and tells it if the mode changed.
    pub(crate) fn set_full_control(&mut self, seat: PlayerId, mode: FullControlMode) {
        if let Some(rejection) = self.out_of_game(seat) {
            self.refuse(seat, rejection);
            return;
        }
        let mut stops = self.stops.clone();
        stops.set_full_control(seat, mode);
        self.commit_stops(stops);
    }

    /// Takes `stops` as the seats' stops and modes and tells each seat whose own changed.
    fn commit_stops(&mut self, stops: Stops) {
        for (seat, message) in self.stops.replace(stops) {
            self.seats.send(seat, message);
        }
    }

    /// The refusal for a seat that is out of the game, or for any seat once the game is over.
    ///
    /// A game that has not started has no such seat: stops set in the lobby are held for the game.
    fn out_of_game(&self, seat: PlayerId) -> Option<Rejection> {
        let Phase::Running(running) = &self.phase else {
            return None;
        };
        let out = running.engine.result().is_some() || !running.engine.state().is_alive(seat);
        out.then(|| {
            let reason = "you are not in a game that is still being played";
            Rejection::new(ErrorCode::BadMessage, reason)
        })
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a seat holds each stop once, only in a step where a player gets priority, a refusal changes nothing, seats are independent, the list is told in a fixed order and only when the seat can see it change, and a seat that left the game or a finished game holds nothing.
    use super::*;
    use mtg_engine::Engine;
    use mtg_protocol::StopSide;
    use mtg_testkit::Scenario;
    use rstest::rstest;

    const ME: PlayerId = PlayerId(1);
    const FOE: PlayerId = PlayerId(0);
    const SIDES: [StopSide; 2] = [StopSide::Own, StopSide::Opponent];
    const MODES: [FullControlMode; 3] = [
        FullControlMode::Off,
        FullControlMode::NextWindow,
        FullControlMode::Held,
    ];

    fn stop(side: StopSide, step: Step) -> Stop {
        Stop { side, step }
    }

    fn own(step: Step) -> Stop {
        stop(StopSide::Own, step)
    }

    fn opponent(step: Step) -> Stop {
        stop(StopSide::Opponent, step)
    }

    /// The `keep` flag stored for `stop`, if the seat holds it.
    fn kept(stops: &Stops, seat: PlayerId, stop: Stop) -> Option<bool> {
        stops.seats[seat.index()].stops.get(&stop).copied()
    }

    #[test]
    fn a_stop_is_held_in_exactly_the_steps_where_a_player_gets_priority_on_either_side() {
        for side in SIDES {
            for step in Step::ALL {
                let mut stops = Stops::new(2);
                let set = stops.set(ME, stop(side, step), false);
                if step.grants_priority() {
                    assert_eq!(set, Ok(()), "{side:?} {step:?}");
                    assert_eq!(stops.stops(ME), [stop(side, step)]);
                } else {
                    assert_eq!(set, Err(Refused::NoPriority(step)), "{side:?} {step:?}");
                    assert_eq!(stops, Stops::new(2), "a refusal changes nothing");
                }
            }
        }
    }

    #[test]
    fn untap_and_cleanup_are_refused_whether_the_seat_sets_or_clears_them() {
        for side in SIDES {
            for step in [Step::Untap, Step::Cleanup] {
                let mut stops = Stops::new(2);
                stops.set(ME, own(Step::Main1), true).unwrap();
                let before = stops.clone();
                assert_eq!(
                    stops.set(ME, stop(side, step), true),
                    Err(Refused::NoPriority(step))
                );
                assert_eq!(
                    stops.clear(ME, stop(side, step)),
                    Err(Refused::NoPriority(step))
                );
                assert_eq!(stops, before);
            }
        }
    }

    #[test]
    fn a_refusal_says_which_step_has_no_priority() {
        let text = Refused::NoPriority(Step::Untap).to_string();
        assert!(
            text.contains("Untap") && text.contains("priority"),
            "{text}"
        );
    }

    #[rstest]
    #[case::one_turn_then_lasting(false, true)]
    #[case::lasting_then_one_turn(true, false)]
    #[case::the_same_again(true, true)]
    fn a_stop_set_twice_is_one_stop_whose_keep_is_the_latest(
        #[case] first: bool,
        #[case] second: bool,
    ) {
        let mut stops = Stops::new(2);
        stops.set(ME, opponent(Step::End), first).unwrap();
        stops.set(ME, opponent(Step::End), second).unwrap();
        assert_eq!(stops.stops(ME), [opponent(Step::End)]);
        assert_eq!(kept(&stops, ME, opponent(Step::End)), Some(second));
    }

    #[test]
    fn the_same_step_on_the_two_sides_is_two_stops() {
        let mut stops = Stops::new(2);
        stops.set(ME, own(Step::Upkeep), false).unwrap();
        stops.set(ME, opponent(Step::Upkeep), true).unwrap();
        assert_eq!(stops.stops(ME), [own(Step::Upkeep), opponent(Step::Upkeep)]);
        stops.clear(ME, own(Step::Upkeep)).unwrap();
        assert_eq!(stops.stops(ME), [opponent(Step::Upkeep)]);
    }

    #[test]
    fn clearing_removes_a_stop_whatever_its_keep_and_clearing_nothing_is_fine() {
        let mut stops = Stops::new(2);
        stops.set(ME, own(Step::Main1), true).unwrap();
        stops.set(ME, own(Step::Main2), false).unwrap();
        stops.clear(ME, own(Step::Main1)).unwrap();
        stops.clear(ME, own(Step::Main2)).unwrap();
        assert_eq!(stops, Stops::new(2));
        let empty = stops.clone();
        stops.clear(ME, own(Step::Draw)).unwrap();
        assert_eq!(stops, empty, "a stop that is not held is already clear");
    }

    #[test]
    fn the_list_is_in_a_fixed_order_whatever_order_the_stops_were_set_in() {
        let wanted = [
            own(Step::Upkeep),
            own(Step::Main1),
            own(Step::End),
            opponent(Step::Draw),
            opponent(Step::DeclareBlockers),
            opponent(Step::Main2),
        ];
        let mut scrambled = wanted;
        scrambled.reverse();
        scrambled.swap(0, 3);
        let mut stops = Stops::new(2);
        for item in scrambled {
            stops.set(ME, item, false).unwrap();
        }
        assert_eq!(stops.stops(ME), wanted);
    }

    #[test]
    fn each_seat_holds_its_own_stops_and_mode() {
        let mut stops = Stops::new(3);
        stops.set(ME, own(Step::Main1), false).unwrap();
        stops.set_full_control(ME, FullControlMode::Held);
        assert_eq!(stops.stops(FOE), []);
        assert_eq!(stops.full_control(FOE), FullControlMode::Off);
        assert_eq!(stops.stops(PlayerId(2)), []);
        assert_eq!(stops.full_control(ME), FullControlMode::Held);
    }

    #[test]
    fn a_seat_the_table_does_not_have_holds_nothing_and_breaks_nothing() {
        let mut stops = Stops::new(2);
        let nobody = PlayerId(5);
        assert_eq!(stops.set(nobody, own(Step::Main1), true), Ok(()));
        assert_eq!(stops.clear(nobody, own(Step::Main1)), Ok(()));
        stops.set_full_control(nobody, FullControlMode::Held);
        assert_eq!(stops.stops(nobody), []);
        assert_eq!(stops.full_control(nobody), FullControlMode::Off);
        assert_eq!(stops, Stops::new(2));
    }

    #[test]
    fn full_control_takes_each_mode_and_starts_off() {
        let mut stops = Stops::new(2);
        assert_eq!(stops.full_control(ME), FullControlMode::Off);
        for mode in MODES {
            stops.set_full_control(ME, mode);
            assert_eq!(stops.full_control(ME), mode);
        }
    }

    #[test]
    fn replacing_names_each_seat_whose_list_or_mode_changed_and_what_it_holds_now() {
        let mut old = Stops::new(3);
        old.set(FOE, own(Step::Upkeep), false).unwrap();
        old.set(ME, own(Step::Main1), true).unwrap();
        old.set_full_control(ME, FullControlMode::Held);

        let mut new = old.clone();
        new.clear(FOE, own(Step::Upkeep)).unwrap();
        new.set(ME, opponent(Step::End), false).unwrap();
        new.set_full_control(ME, FullControlMode::Off);
        new.set_full_control(PlayerId(2), FullControlMode::NextWindow);

        let mut current = old.clone();
        assert_eq!(
            current.replace(new.clone()),
            [
                (FOE, ServerMessage::Stops { stops: Vec::new() }),
                (
                    ME,
                    ServerMessage::Stops {
                        stops: vec![own(Step::Main1), opponent(Step::End)]
                    }
                ),
                (
                    ME,
                    ServerMessage::FullControl {
                        mode: FullControlMode::Off
                    }
                ),
                (
                    PlayerId(2),
                    ServerMessage::FullControl {
                        mode: FullControlMode::NextWindow
                    }
                ),
            ]
        );
        assert_eq!(current, new);
        assert_eq!(current.replace(new), [], "the same stops change nothing");
    }

    #[test]
    fn replacing_how_long_a_stop_lasts_tells_nobody_since_the_list_is_the_same() {
        let mut old = Stops::new(2);
        old.set(ME, own(Step::Main1), false).unwrap();
        let mut new = old.clone();
        new.set(ME, own(Step::Main1), true).unwrap();
        assert_eq!(old.replace(new), []);
        assert_eq!(
            kept(&old, ME, own(Step::Main1)),
            Some(true),
            "but it is kept"
        );
    }

    #[test]
    fn a_seat_that_left_and_a_finished_game_hold_nothing() {
        let setup = Scenario::new(3).setup().unwrap();
        let mut engine = Engine::new(setup).unwrap();
        let mut stops = Stops::new(3);
        for seat in 0..3 {
            stops.set(PlayerId(seat), own(Step::Main1), true).unwrap();
            stops.set_full_control(PlayerId(seat), FullControlMode::Held);
        }
        stops.drop_finished(engine.state());
        assert_eq!(stops.stops(PlayerId(0)), [own(Step::Main1)]);

        engine.concede(PlayerId(2)).unwrap();
        stops.drop_finished(engine.state());
        assert_eq!(stops.stops(PlayerId(2)), [], "the seat that left");
        assert_eq!(stops.full_control(PlayerId(2)), FullControlMode::Off);
        assert_eq!(stops.stops(PlayerId(0)), [own(Step::Main1)]);
        assert_eq!(stops.full_control(PlayerId(1)), FullControlMode::Held);

        engine.concede(PlayerId(1)).unwrap();
        assert!(engine.state().result().is_some(), "the game is over");
        stops.drop_finished(engine.state());
        assert_eq!(stops, Stops::new(3), "so is everyone's");
    }
}
