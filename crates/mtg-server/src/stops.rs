//! Stops and Full Control: what a seat asked the server to remember about when it wants priority.
//!
//! A stop is a step of a turn at which the seat wants to be given priority; Full Control is a mode in which it
//! wants priority at every step. Both live in the actor's memory per seat, like yields: they are not stored, they
//! survive a reconnect (`Welcome` states them) and a resumed game starts with the server's default mode and no
//! stop. [`crate::windows`] reads them to decide when a seat is asked. Turning auto-pass off is the `Held` mode,
//! so the seat's auto-pass flag is not kept apart from the mode.
//!
//! The rules of a stop are here: it names a step where a player gets priority (never untap or cleanup), a seat holds
//! each side and step once, setting a held stop again replaces how long it lasts, and a one-turn stop goes once the
//! game has left its step. A refusal is a small `Error{BadMessage}` that leaves everything as it was, so a client
//! never needs a resync after one.

use std::collections::BTreeMap;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::events::ResolutionOutcome;
use mtg_engine::turn::Step;
use mtg_engine::{Event, GameState};
use mtg_protocol::{ErrorCode, FullControlMode, ServerMessage, Stop, StopSide};
use thiserror::Error;

use crate::actor::{Fatal, GameActor, Phase};
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
    /// Each side and step with whether its stop lasts: false until the step has passed once, true until cleared.
    ///
    /// Keyed by the place alone, so a seat holds one stop there whatever its `keep`.
    stops: BTreeMap<(StopSide, Step), bool>,
    full_control: FullControlMode,
    /// Whether a spell or ability of the seat with clauses left to the table resolved and the seat has not passed
    /// priority since: it is asked at its next priority window, as at a stop, to apply them.
    by_hand: bool,
}

/// The stops and the Full Control mode of every seat of one game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Stops {
    seats: Vec<Held>,
}

/// Where the game is: whose turn it is and in which step.
pub(crate) type Place = (PlayerId, Step);

/// Where `state` stands.
pub(crate) fn place(state: &GameState) -> Place {
    let turn = state.turn();
    (turn.active(), turn.step())
}

/// The objects on the stack of `state` whose rules text holds clauses the table is owed when they resolve
/// ([`mtg_cards::CardDefinition::manual_owed`], the flag `ObjectView::manual_owed` tells clients), with their
/// controllers: what [`Stops::resolved`] needs to know once one of them has resolved and left.
pub(crate) fn by_hand_on_stack(state: &GameState) -> Vec<(ObjectId, PlayerId)> {
    state
        .stack()
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().manual_owed())
        .map(|object| (object.id(), object.controller()))
        .collect()
}

impl Stops {
    /// No seat holds a stop, and every seat is out of Full Control.
    #[cfg(test)]
    pub(crate) fn new(seats: usize) -> Stops {
        Stops::with_mode(seats, FullControlMode::Off)
    }

    /// No seat holds a stop, and every seat is in Full Control mode `mode` (the server's default).
    pub(crate) fn with_mode(seats: usize, mode: FullControlMode) -> Stops {
        Stops {
            seats: vec![
                Held {
                    full_control: mode,
                    ..Held::default()
                };
                seats
            ],
        }
    }

    /// Whether `seat` holds a stop at `step` of a turn on `side`, one-turn or lasting.
    pub(crate) fn holds(&self, seat: PlayerId, side: StopSide, step: Step) -> bool {
        self.seats
            .get(seat.index())
            .is_some_and(|held| held.stops.contains_key(&(side, step)))
    }

    /// Whether the server passes for `seat` by its usual rules: the seat is in any mode but `Held`.
    ///
    /// This is the seat's auto-pass setting: turning it off is the `Held` mode.
    pub(crate) fn auto_pass(&self, seat: PlayerId) -> bool {
        self.seats
            .get(seat.index())
            .is_some_and(|held| held.full_control != FullControlMode::Held)
    }

    /// The stops `seat` holds, once each with its `keep`: own side first and each side in turn order.
    pub(crate) fn stops(&self, seat: PlayerId) -> Vec<Stop> {
        self.seats
            .get(seat.index())
            .map(|held| {
                held.stops
                    .iter()
                    .map(|(&(side, step), &keep)| Stop { side, step, keep })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `seat`'s Full Control mode.
    pub(crate) fn full_control(&self, seat: PlayerId) -> FullControlMode {
        self.seats
            .get(seat.index())
            .map(|held| held.full_control)
            .unwrap_or_default()
    }

    /// Gives `seat` the stop `stop`; one it already holds at that side and step only has its `keep` replaced.
    pub(crate) fn set(&mut self, seat: PlayerId, stop: Stop) -> Result<(), Refused> {
        check(stop)?;
        if let Some(held) = self.seats.get_mut(seat.index()) {
            held.stops.insert((stop.side, stop.step), stop.keep);
        }
        Ok(())
    }

    /// Takes the stop at the side and step of `stop` from `seat`, whatever its `keep`; one it does not hold is
    /// already gone.
    pub(crate) fn clear(&mut self, seat: PlayerId, stop: Stop) -> Result<(), Refused> {
        check(stop)?;
        if let Some(held) = self.seats.get_mut(seat.index()) {
            held.stops.remove(&(stop.side, stop.step));
        }
        Ok(())
    }

    /// Puts `seat` in Full Control mode `mode`.
    pub(crate) fn set_full_control(&mut self, seat: PlayerId, mode: FullControlMode) {
        if let Some(held) = self.seats.get_mut(seat.index()) {
            held.full_control = mode;
        }
    }

    /// Takes every stop from `seat` and puts it in Full Control mode `mode` (the server's default),
    /// for a seat that changes hands.
    pub(crate) fn reset(&mut self, seat: PlayerId, mode: FullControlMode) {
        if let Some(held) = self.seats.get_mut(seat.index()) {
            *held = Held {
                full_control: mode,
                ..Held::default()
            };
        }
    }

    /// Clears the one-turn stops of the steps that `events` left, applied with the game at `at`; lasting stops stay.
    ///
    /// A step is left when the next one begins, so a step the game entered and left in one go (declare attackers
    /// with nobody to attack) counts, and one it never entered does not.
    pub(crate) fn left(&mut self, mut at: Place, events: &[Event]) {
        for event in events {
            match event {
                Event::TurnBegan { active, .. } => {
                    self.over(at);
                    at = (*active, Step::Untap);
                }
                Event::StepBegan { step } if *step != at.1 => {
                    self.over(at);
                    at.1 = *step;
                }
                _ => {}
            }
        }
    }

    /// Removes every seat's one-turn stop at the side that `place` is for that seat, and the step.
    fn over(&mut self, (active, step): Place) {
        for (seat, held) in self.seats.iter_mut().enumerate() {
            let side = if PlayerId::from_index(seat) == active {
                StopSide::Own
            } else {
                StopSide::Opponent
            };
            if held.stops.get(&(side, step)) == Some(&false) {
                held.stops.remove(&(side, step));
            }
        }
    }

    /// Whether `seat` owes the table the clauses of a spell or ability of its own that just resolved: it is asked at
    /// its next priority window, as at a stop, until it passes.
    pub(crate) fn by_hand(&self, seat: PlayerId) -> bool {
        self.seats
            .get(seat.index())
            .is_some_and(|held| held.by_hand)
    }

    /// Follows `events`, applied with `on_stack` ([`by_hand_on_stack`]) on the stack: a seat that passes priority
    /// owes nothing, and the controller of one of those objects that resolves owes its clauses.
    pub(crate) fn resolved(&mut self, on_stack: &[(ObjectId, PlayerId)], events: &[Event]) {
        for event in events {
            let (seat, owes) = match event {
                Event::PriorityPassed { player } => (*player, false),
                Event::SpellResolved { object }
                | Event::ResolutionEnded {
                    object,
                    outcome: ResolutionOutcome::Resolved,
                } => match on_stack.iter().find(|(id, _)| id == object) {
                    Some(&(_, controller)) => (controller, true),
                    None => continue,
                },
                _ => continue,
            };
            if let Some(held) = self.seats.get_mut(seat.index()) {
                held.by_hand = owes;
            }
        }
    }

    /// `seat` has passed priority: a `NextWindow` mode ends.
    pub(crate) fn passed(&mut self, seat: PlayerId) {
        if let Some(held) = self.seats.get_mut(seat.index())
            && held.full_control == FullControlMode::NextWindow
        {
            held.full_control = FullControlMode::Off;
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
    /// A seat is told its whole list of stops when the list changes (a stop that only changes its `keep` changes
    /// the list), then its mode when that changes; seats in seat order.
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
    /// Sets (`on`) or clears `stop` for `seat`, and tells the seat if the list it holds changed, `keep` included.
    ///
    /// Anything the rules refuse is answered with `Error{BadMessage}` and changes nothing. This takes no
    /// step of the game, so there is nothing to save and nothing to take back, except for a seat that waits for its
    /// dwell at a window the new stop makes one to ask: that one is sent its decision at once, as a unit with the
    /// stops (an empty step), so a failed save keeps them as they were.
    pub(crate) async fn set_stop(
        &mut self,
        seat: PlayerId,
        stop: Stop,
        on: bool,
    ) -> Result<(), Fatal> {
        if let Some(rejection) = self.out_of_game(seat) {
            self.refuse(seat, rejection);
            return Ok(());
        }
        let mut stops = self.stops.clone();
        let done = if on {
            stops.set(seat, stop)
        } else {
            stops.clear(seat, stop)
        };
        match done {
            Ok(()) => self.take_stops(seat, stops).await,
            Err(refused) => {
                let rejection = Rejection::new(ErrorCode::BadMessage, refused.to_string());
                self.refuse(seat, rejection);
                Ok(())
            }
        }
    }

    /// Takes `stops` as the seats' stops and modes. A seat that waits for its dwell is sent its decision at once if
    /// they now ask it: the decision is issued again, as a unit with the stops.
    async fn take_stops(&mut self, seat: PlayerId, stops: Stops) -> Result<(), Fatal> {
        if let Phase::Running(running) = &mut self.phase
            && running.dwell_ends(seat, &stops, &self.yields)
        {
            return match running.restart_decision(seat) {
                Ok(step) => {
                    let yields = self.yields.clone();
                    self.publish_with(step, yields, stops).await
                }
                Err(rejection) => {
                    self.refuse(seat, rejection);
                    Ok(())
                }
            };
        }
        self.commit_stops(stops);
        Ok(())
    }

    /// Puts `seat` in Full Control mode `mode` and tells it if the mode changed.
    pub(crate) async fn set_full_control(
        &mut self,
        seat: PlayerId,
        mode: FullControlMode,
    ) -> Result<(), Fatal> {
        if let Some(rejection) = self.out_of_game(seat) {
            self.refuse(seat, rejection);
            return Ok(());
        }
        self.put_in_mode(seat, mode).await
    }

    /// Sets the seat's auto-pass: off is the `Held` mode, on is `Off` unless the seat is in a mode that already
    /// lets the server pass for it (`NextWindow` stays).
    ///
    /// A seat that is out of the game, or a game that is over, has nothing to set and is not answered.
    pub(crate) async fn set_auto_pass(
        &mut self,
        seat: PlayerId,
        enabled: bool,
    ) -> Result<(), Fatal> {
        if self.out_of_game(seat).is_some() {
            return Ok(());
        }
        let mode = match (enabled, self.stops.full_control(seat)) {
            (false, _) => FullControlMode::Held,
            (true, FullControlMode::Held) => FullControlMode::Off,
            (true, other) => other,
        };
        self.put_in_mode(seat, mode).await
    }

    /// Takes `mode` for `seat`; a mode it already has changes nothing and says nothing.
    ///
    /// Turning Full Control on ends a running yield, and a seat that waited for its dwell is sent its decision at
    /// once. Turning it off while the seat faces a priority decision that the usual rules now pass for passes at once,
    /// as a started yield does (after its dwell, at an idle default window): the passes, the new mode and the save are
    /// one unit, so a failed save keeps the mode as it was.
    async fn put_in_mode(&mut self, seat: PlayerId, mode: FullControlMode) -> Result<(), Fatal> {
        if self.stops.full_control(seat) == mode {
            return Ok(());
        }
        let mut stops = self.stops.clone();
        stops.set_full_control(seat, mode);
        let mut yields = self.yields.clone();
        if mode != FullControlMode::Off {
            yields.cancel(seat);
        }
        if let Phase::Running(running) = &mut self.phase
            && ((mode == FullControlMode::Off && running.passes_pending(seat, &stops, &yields))
                || running.dwell_ends(seat, &stops, &yields))
        {
            return match running.restart_decision(seat) {
                Ok(step) => self.publish_with(step, yields, stops).await,
                Err(rejection) => {
                    self.refuse(seat, rejection);
                    Ok(())
                }
            };
        }
        self.commit_yields(yields);
        self.commit_stops(stops);
        Ok(())
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
    //! Goal: a seat holds one stop per side and step with the `keep` it was last given, only in a step where a player gets priority, a refusal changes nothing, seats are independent, the list is told in a fixed order whenever it changes (a changed `keep` too) and not otherwise, a one-turn stop goes once the game has left its step (and a lasting one never does), a pass ends `NextWindow` and no other mode, auto-pass is every mode but `Held`, a seat that left the game or a finished game holds nothing, and a seat that changes hands in the lobby starts from the defaults again; a seat owes the clauses of its own object that resolved with clauses left to the table until it passes, and is never told so. and the objects it holds a seat for are those the wire flags (`ObjectView::manual_owed`), for a spell and for an ability.
    use super::*;
    use mtg_engine::Engine;
    use mtg_protocol::ObjectView;
    use mtg_testkit::cards::{by_hand, free_creature, healer};
    use mtg_testkit::{Driver, Scenario, ready};
    use rstest::rstest;

    const ME: PlayerId = PlayerId(1);
    const FOE: PlayerId = PlayerId(0);
    const SIDES: [StopSide; 2] = [StopSide::Own, StopSide::Opponent];
    const MODES: [FullControlMode; 3] = [
        FullControlMode::Off,
        FullControlMode::NextWindow,
        FullControlMode::Held,
    ];

    fn stop(side: StopSide, step: Step, keep: bool) -> Stop {
        Stop { side, step, keep }
    }

    fn own(step: Step, keep: bool) -> Stop {
        stop(StopSide::Own, step, keep)
    }

    fn opponent(step: Step, keep: bool) -> Stop {
        stop(StopSide::Opponent, step, keep)
    }

    #[test]
    fn a_stop_is_held_in_exactly_the_steps_where_a_player_gets_priority_on_either_side() {
        for side in SIDES {
            for step in Step::ALL {
                for keep in [false, true] {
                    let mut stops = Stops::new(2);
                    let set = stops.set(ME, stop(side, step, keep));
                    if step.grants_priority() {
                        assert_eq!(set, Ok(()), "{side:?} {step:?}");
                        assert_eq!(stops.stops(ME), [stop(side, step, keep)]);
                    } else {
                        assert_eq!(set, Err(Refused::NoPriority(step)), "{side:?} {step:?}");
                        assert_eq!(stops, Stops::new(2), "a refusal changes nothing");
                    }
                }
            }
        }
    }

    #[test]
    fn untap_and_cleanup_are_refused_whether_the_seat_sets_or_clears_them() {
        for side in SIDES {
            for step in [Step::Untap, Step::Cleanup] {
                let mut stops = Stops::new(2);
                stops.set(ME, own(Step::Main1, true)).unwrap();
                let before = stops.clone();
                assert_eq!(
                    stops.set(ME, stop(side, step, true)),
                    Err(Refused::NoPriority(step))
                );
                assert_eq!(
                    stops.clear(ME, stop(side, step, true)),
                    Err(Refused::NoPriority(step))
                );
                assert_eq!(stops, before);
            }
        }
    }

    #[rstest]
    #[case::off(FullControlMode::Off)]
    #[case::next_window(FullControlMode::NextWindow)]
    #[case::held(FullControlMode::Held)]
    fn a_seat_that_changes_hands_starts_from_the_default_mode_with_no_stop(
        #[case] default: FullControlMode,
    ) {
        let mut stops = Stops::with_mode(3, default);
        for seat in [FOE, ME] {
            stops.set(seat, own(Step::Main1, true)).unwrap();
            stops.set(seat, opponent(Step::End, false)).unwrap();
            stops.set_full_control(seat, FullControlMode::Held);
        }
        let others = stops.clone();

        stops.reset(ME, default);
        assert_eq!(stops.stops(ME), []);
        assert_eq!(stops.full_control(ME), default);
        // The seat before it and the seat past the table are untouched.
        assert_eq!(stops.stops(FOE), others.stops(FOE));
        assert_eq!(stops.full_control(FOE), FullControlMode::Held);
        stops.reset(PlayerId(9), default);
        assert_eq!(stops.stops(FOE), others.stops(FOE));
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
        stops.set(ME, opponent(Step::End, first)).unwrap();
        stops.set(ME, opponent(Step::End, second)).unwrap();
        assert_eq!(stops.stops(ME), [opponent(Step::End, second)]);
    }

    #[test]
    fn the_same_step_on_the_two_sides_is_two_stops() {
        let mut stops = Stops::new(2);
        stops.set(ME, own(Step::Upkeep, false)).unwrap();
        stops.set(ME, opponent(Step::Upkeep, true)).unwrap();
        assert_eq!(
            stops.stops(ME),
            [own(Step::Upkeep, false), opponent(Step::Upkeep, true)]
        );
        stops.clear(ME, own(Step::Upkeep, false)).unwrap();
        assert_eq!(stops.stops(ME), [opponent(Step::Upkeep, true)]);
    }

    #[test]
    fn clearing_removes_a_stop_whatever_keep_the_request_names_and_clearing_nothing_is_fine() {
        let mut stops = Stops::new(2);
        stops.set(ME, own(Step::Main1, true)).unwrap();
        stops.set(ME, own(Step::Main2, false)).unwrap();
        // The request's `keep` means nothing when clearing: it need not match the one the stop was set with.
        stops.clear(ME, own(Step::Main1, false)).unwrap();
        stops.clear(ME, own(Step::Main2, true)).unwrap();
        assert_eq!(stops, Stops::new(2));
        let empty = stops.clone();
        stops.clear(ME, own(Step::Draw, false)).unwrap();
        assert_eq!(stops, empty, "a stop that is not held is already clear");
    }

    #[test]
    fn the_list_is_in_a_fixed_order_whatever_order_or_keep_the_stops_were_set_with() {
        let wanted = [
            own(Step::Upkeep, true),
            own(Step::Main1, false),
            own(Step::End, true),
            opponent(Step::Draw, false),
            opponent(Step::DeclareBlockers, true),
            opponent(Step::Main2, false),
        ];
        let mut scrambled = wanted;
        scrambled.reverse();
        scrambled.swap(0, 3);
        let mut stops = Stops::new(2);
        for item in scrambled {
            stops.set(ME, item).unwrap();
        }
        assert_eq!(stops.stops(ME), wanted);
        // Changing a keep moves nothing.
        stops.set(ME, own(Step::Main1, true)).unwrap();
        stops.set(ME, opponent(Step::Draw, true)).unwrap();
        let places: Vec<_> = stops.stops(ME).iter().map(|s| (s.side, s.step)).collect();
        let wanted_places: Vec<_> = wanted.iter().map(|s| (s.side, s.step)).collect();
        assert_eq!(places, wanted_places);
    }

    #[test]
    fn each_seat_holds_its_own_stops_and_mode() {
        let mut stops = Stops::new(3);
        stops.set(ME, own(Step::Main1, false)).unwrap();
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
        assert_eq!(stops.set(nobody, own(Step::Main1, true)), Ok(()));
        assert_eq!(stops.clear(nobody, own(Step::Main1, true)), Ok(()));
        stops.set_full_control(nobody, FullControlMode::Held);
        assert_eq!(stops.stops(nobody), []);
        assert_eq!(stops.full_control(nobody), FullControlMode::Off);
        assert_eq!(stops, Stops::new(2));
    }

    fn resolution(object: u64, outcome: ResolutionOutcome) -> Event {
        Event::ResolutionEnded {
            object: ObjectId(object),
            outcome,
        }
    }

    fn passed(player: PlayerId) -> Event {
        Event::PriorityPassed { player }
    }

    #[rstest]
    #[case::a_spell(vec![Event::SpellResolved { object: ObjectId(7) }], [false, true])]
    #[case::an_ability(vec![resolution(7, ResolutionOutcome::Resolved)], [false, true])]
    #[case::a_foe_object(vec![resolution(8, ResolutionOutcome::Resolved)], [true, false])]
    #[case::fizzled(vec![resolution(7, ResolutionOutcome::Fizzled)], [false, false])]
    #[case::countered(vec![resolution(7, ResolutionOutcome::Countered)], [false, false])]
    #[case::not_by_hand(vec![resolution(9, ResolutionOutcome::Resolved)], [false, false])]
    #[case::pass_after(vec![resolution(7, ResolutionOutcome::Resolved), passed(ME)], [false, false])]
    #[case::pass_before(vec![passed(ME), resolution(7, ResolutionOutcome::Resolved)], [false, true])]
    #[case::foe_passes(vec![resolution(7, ResolutionOutcome::Resolved), passed(FOE)], [false, true])]
    fn a_seat_owes_the_clauses_of_its_object_that_resolved_until_it_passes(
        #[case] events: Vec<Event>,
        #[case] owed: [bool; 2],
    ) {
        // Objects 7 (the seat's) and 8 (the foe's) hold clauses left to the table; 9 holds none and is not listed.
        let on_stack = [(ObjectId(7), ME), (ObjectId(8), FOE)];
        let mut stops = Stops::new(2);
        stops.resolved(&on_stack, &events);
        assert_eq!([stops.by_hand(FOE), stops.by_hand(ME)], owed);
        // Nobody is told: the list and the mode are unchanged.
        assert_eq!(stops.clone().replace(Stops::new(2)), []);
        stops.resolved(&[], &[passed(FOE), passed(ME)]);
        assert_eq!([stops.by_hand(FOE), stops.by_hand(ME)], [false, false]);
    }

    #[test]
    fn a_seat_that_changes_hands_owes_nothing_and_one_the_table_lacks_never_does() {
        let mut stops = Stops::new(2);
        let spell = Event::SpellResolved {
            object: ObjectId(7),
        };
        stops.resolved(&[(ObjectId(7), ME), (ObjectId(7), PlayerId(5))], &[spell]);
        assert!(stops.by_hand(ME));
        stops.reset(ME, FullControlMode::Off);
        assert!(!stops.by_hand(ME));
        assert!(!stops.by_hand(PlayerId(5)));
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
        old.set(FOE, own(Step::Upkeep, false)).unwrap();
        old.set(ME, own(Step::Main1, true)).unwrap();
        old.set_full_control(ME, FullControlMode::Held);

        let mut new = old.clone();
        new.clear(FOE, own(Step::Upkeep, false)).unwrap();
        new.set(ME, opponent(Step::End, false)).unwrap();
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
                        stops: vec![own(Step::Main1, true), opponent(Step::End, false)]
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

    #[rstest]
    #[case::one_turn_to_lasting(false, true)]
    #[case::lasting_to_one_turn(true, false)]
    fn replacing_how_long_a_stop_lasts_tells_the_seat_the_whole_list_with_the_new_keep(
        #[case] before: bool,
        #[case] after: bool,
    ) {
        let mut old = Stops::new(2);
        old.set(ME, own(Step::Main1, before)).unwrap();
        old.set(ME, opponent(Step::End, before)).unwrap();
        let mut new = old.clone();
        new.set(ME, own(Step::Main1, after)).unwrap();
        assert_eq!(
            old.replace(new),
            [(
                ME,
                ServerMessage::Stops {
                    stops: vec![own(Step::Main1, after), opponent(Step::End, before)]
                }
            )],
            "the seat is told the list, in the same order, with the one keep changed"
        );
    }

    #[test]
    fn setting_a_stop_again_with_the_keep_it_has_changes_nothing_to_tell() {
        let mut old = Stops::new(2);
        old.set(ME, own(Step::Main1, true)).unwrap();
        let mut new = old.clone();
        new.set(ME, own(Step::Main1, true)).unwrap();
        assert_eq!(old.replace(new), []);
    }

    #[test]
    fn a_seat_that_left_and_a_finished_game_hold_nothing() {
        let setup = Scenario::new(3).setup().unwrap();
        let mut engine = Engine::new(setup).unwrap();
        let mut stops = Stops::new(3);
        for seat in 0..3 {
            stops.set(PlayerId(seat), own(Step::Main1, true)).unwrap();
            stops.set_full_control(PlayerId(seat), FullControlMode::Held);
        }
        stops.drop_finished(engine.state());
        assert_eq!(stops.stops(PlayerId(0)), [own(Step::Main1, true)]);

        engine.concede(PlayerId(2)).unwrap();
        stops.drop_finished(engine.state());
        assert_eq!(stops.stops(PlayerId(2)), [], "the seat that left");
        assert_eq!(stops.full_control(PlayerId(2)), FullControlMode::Off);
        assert_eq!(stops.stops(PlayerId(0)), [own(Step::Main1, true)]);
        assert_eq!(stops.full_control(PlayerId(1)), FullControlMode::Held);

        engine.concede(PlayerId(1)).unwrap();
        assert!(engine.state().result().is_some(), "the game is over");
        stops.drop_finished(engine.state());
        assert_eq!(stops, Stops::new(3), "so is everyone's");
    }

    #[rstest]
    #[case::off(FullControlMode::Off, true)]
    #[case::next_window(FullControlMode::NextWindow, true)]
    #[case::held(FullControlMode::Held, false)]
    fn auto_pass_is_on_in_every_mode_but_held_and_every_seat_starts_in_the_mode_it_is_given(
        #[case] mode: FullControlMode,
        #[case] auto_pass: bool,
    ) {
        let mut stops = Stops::with_mode(3, mode);
        for seat in 0..3 {
            assert_eq!(stops.full_control(PlayerId(seat)), mode);
            assert_eq!(stops.auto_pass(PlayerId(seat)), auto_pass);
        }
        assert!(
            !stops.auto_pass(PlayerId(7)),
            "a seat the table does not have is passed for by nobody"
        );
        stops.set_full_control(ME, FullControlMode::Held);
        assert!(!stops.auto_pass(ME));
        assert_eq!(stops.auto_pass(FOE), auto_pass, "only the seat's own");
    }

    #[test]
    fn a_stop_is_held_at_its_side_and_step_only_whatever_its_keep() {
        let mut stops = Stops::new(2);
        stops.set(ME, own(Step::Main1, false)).unwrap();
        stops.set(ME, opponent(Step::End, true)).unwrap();
        for side in SIDES {
            for step in Step::ALL {
                let held = matches!(
                    (side, step),
                    (StopSide::Own, Step::Main1) | (StopSide::Opponent, Step::End)
                );
                assert_eq!(stops.holds(ME, side, step), held, "{side:?} {step:?}");
                assert!(!stops.holds(FOE, side, step), "another seat holds none");
            }
        }
    }

    fn began(step: Step) -> Event {
        Event::StepBegan { step }
    }

    fn turn(number: u32, active: PlayerId) -> [Event; 2] {
        [
            Event::TurnBegan {
                number,
                active,
                extra: false,
            },
            Event::StepBegan { step: Step::Untap },
        ]
    }

    /// Seat 1 holds a one-turn and a lasting stop at `step` on `side`; the game is at `at` and `events` happen.
    fn after(side: StopSide, step: Step, at: Place, events: &[Event]) -> (Vec<Stop>, Vec<Stop>) {
        let mut stops = Stops::new(2);
        stops.set(ME, stop(side, step, false)).unwrap();
        let other = if step == Step::End {
            Step::Main2
        } else {
            Step::End
        };
        stops.set(ME, stop(side, other, true)).unwrap();
        stops.left(at, events);
        let kept = stops.stops(ME);
        let lasting = kept.iter().copied().filter(|s| s.keep).collect();
        let one_turn = kept.into_iter().filter(|s| !s.keep).collect();
        (one_turn, lasting)
    }

    #[rstest]
    #[case::the_next_step_begins(StopSide::Own, Step::Main1, (ME, Step::Main1), vec![began(Step::BeginCombat)], true)]
    #[case::a_step_left_on_the_other_side_leaves_it(StopSide::Opponent, Step::Main1, (ME, Step::Main1), vec![began(Step::BeginCombat)], false)]
    #[case::the_other_sides_turn_is_not_this_one(StopSide::Own, Step::Main1, (FOE, Step::Main1), vec![began(Step::BeginCombat)], false)]
    #[case::the_opponents_step_on_the_opponents_turn(StopSide::Opponent, Step::End, (FOE, Step::End), vec![began(Step::Cleanup)], true)]
    #[case::another_step_of_the_turn_is_left(StopSide::Own, Step::Main1, (ME, Step::Draw), vec![began(Step::Main1)], false)]
    #[case::the_step_is_still_running(StopSide::Own, Step::Main1, (ME, Step::Main1), vec![], false)]
    #[case::a_new_turn_ends_the_step(StopSide::Own, Step::End, (ME, Step::End), turn(4, FOE).to_vec(), true)]
    fn a_one_turn_stop_goes_when_its_step_is_left_and_not_before(
        #[case] side: StopSide,
        #[case] step: Step,
        #[case] at: Place,
        #[case] events: Vec<Event>,
        #[case] gone: bool,
    ) {
        let (one_turn, lasting) = after(side, step, at, &events);
        assert_eq!(one_turn.is_empty(), gone, "{one_turn:?}");
        assert_eq!(lasting.len(), 1, "a lasting stop always stays");
    }

    #[test]
    fn a_step_entered_and_left_in_one_go_is_passed_and_one_never_entered_is_not() {
        // Declare attackers with nobody to attack with: the game enters the step and leaves it without priority.
        let events = [began(Step::DeclareAttackers), began(Step::EndCombat)];
        let (kept, _) = after(
            StopSide::Own,
            Step::DeclareAttackers,
            (ME, Step::BeginCombat),
            &events,
        );
        assert_eq!(kept, [], "passed through");
        // The same turn skips the blockers step altogether, so a stop there waits for a turn that has one.
        let (kept, _) = after(
            StopSide::Own,
            Step::DeclareBlockers,
            (ME, Step::BeginCombat),
            &events,
        );
        assert_eq!(kept, [own(Step::DeclareBlockers, false)], "never entered");
    }

    #[test]
    fn a_stop_set_while_its_step_runs_covers_that_step_and_one_set_for_a_step_already_past_waits_for_the_next_turn()
     {
        let mut stops = Stops::new(2);
        // Set in seat 1's own first main phase: it covers this main phase and goes when the phase is left.
        stops.set(ME, own(Step::Main1, false)).unwrap();
        // Set after seat 1's own upkeep passed: it waits for the next turn of seat 1.
        stops.set(ME, own(Step::Upkeep, false)).unwrap();
        stops.left((ME, Step::Main1), &[began(Step::BeginCombat)]);
        assert_eq!(stops.stops(ME), [own(Step::Upkeep, false)]);
        // A whole turn of each seat goes by: seat 1's upkeep comes round again and ends the stop at it.
        let mut events = Vec::new();
        events.extend(turn(3, FOE));
        events.extend(turn(4, ME));
        events.push(began(Step::Upkeep));
        events.push(began(Step::Draw));
        stops.left((ME, Step::End), &events);
        assert_eq!(stops.stops(ME), []);
    }

    #[test]
    fn several_turns_clear_each_one_turn_stop_once_and_keep_the_lasting_ones() {
        let setup = Scenario::new(2).setup().unwrap();
        let mut engine = Engine::new(setup).unwrap();
        let mut stops = Stops::new(2);
        let lasting = own(Step::Main2, true);
        stops.set(ME, lasting).unwrap();
        stops.set(ME, opponent(Step::End, false)).unwrap();
        stops.set(ME, own(Step::End, false)).unwrap();
        stops.set(FOE, own(Step::End, false)).unwrap();

        let mut cleared = Vec::new();
        while engine.state().turn().number() < 4 {
            let at = place(engine.state());
            let before = (stops.stops(ME), stops.stops(FOE));
            let events = engine
                .apply(mtg_engine::Answer::Priority(
                    mtg_engine::decision::PriorityAction::Pass,
                ))
                .unwrap();
            stops.left(at, &events);
            if (stops.stops(ME), stops.stops(FOE)) != before {
                cleared.push(at);
            }
        }
        // Seat 0 plays turns 1 and 3, seat 1 turn 2. Seat 0's end step in turn 1 ends seat 1's `Opponent` stop and
        // seat 0's own `Own` stop together; seat 1's end step in turn 2 ends its `Own` stop. The lasting stop stays.
        assert_eq!(cleared, [(FOE, Step::End), (ME, Step::End)]);
        assert_eq!(stops.stops(ME), [lasting]);
        assert_eq!(stops.stops(FOE), []);
    }

    #[test]
    fn passing_ends_a_next_window_and_no_other_mode() {
        for (before, after) in [
            (FullControlMode::Off, FullControlMode::Off),
            (FullControlMode::NextWindow, FullControlMode::Off),
            (FullControlMode::Held, FullControlMode::Held),
        ] {
            let mut stops = Stops::new(2);
            stops.set_full_control(ME, before);
            stops.set_full_control(FOE, FullControlMode::NextWindow);
            stops.passed(ME);
            assert_eq!(stops.full_control(ME), after, "{before:?}");
            assert_eq!(
                stops.full_control(FOE),
                FullControlMode::NextWindow,
                "another seat's pass is not this seat's"
            );
        }
        // A seat the table does not have passes into thin air.
        Stops::new(2).passed(PlayerId(9));
    }

    /// Puts a spell of seat 0 with the clause `text` on the stack, or with `ability` an ability of its permanent, and
    /// returns the driver and the object.
    fn on_the_stack(text: &str, ability: bool) -> (Driver, ObjectId) {
        let scenario = if ability {
            Scenario::new(2).battlefield(0, [ready(by_hand(healer("Held", 0), text))])
        } else {
            Scenario::new(2).hand(0, [by_hand(free_creature("Held", 1, 1), text)])
        };
        let mut driver = Driver::from_scenario(scenario).unwrap();
        if ability {
            driver.begin_activation("Held").unwrap();
        } else {
            driver.cast("Held").unwrap();
        }
        let top = *driver.state().stack().last().unwrap();
        (driver, top)
    }

    #[rstest]
    #[case::mana_ability("{T}: Add {C}.", false)]
    #[case::activated_ability("{2}, {T}: Draw a card.", false)]
    #[case::loyalty_abilities("+1: Draw a card.\n\u{2212}2: Destroy target creature.", false)]
    #[case::a_trigger("Whenever this creature dies, each opponent loses 1 life.", true)]
    #[case::a_trigger_among_standing_clauses(
        "{T}: Add {C}.\nWhen ~ enters, you gain 2 life.",
        true
    )]
    fn the_seat_is_held_for_the_objects_the_wire_flags(
        #[case] text: &str,
        #[case] owed: bool,
        #[values(false, true)] ability: bool,
    ) {
        let (driver, top) = on_the_stack(text, ability);
        let held = by_hand_on_stack(driver.state()).contains(&(top, FOE));
        assert_eq!(held, owed, "{text}");
        for viewer in [FOE, ME, PlayerId(2)] {
            let shown = ObjectView::of(driver.state(), viewer, top).unwrap();
            assert_eq!(
                shown.manual_owed, held,
                "{viewer:?} reads what the server holds the seat by"
            );
        }
    }
}
