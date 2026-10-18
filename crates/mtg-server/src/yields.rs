//! Yields: a seat asks the server to pass priority for it until something happens.
//!
//! While a seat yields, the actor answers each of its `Priority` decisions with a pass and never sends them, through
//! the run of passes that serves auto-pass ([`crate::running::Running::skip_idle`]), so a pass, its rollback and its
//! batch are those of the seat's own click. A yield lives in the actor's memory only: it is not stored and a resumed
//! game starts without any.

use std::collections::BTreeSet;

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::GameState;
use mtg_protocol::{ErrorCode, ServerMessage, YieldUntil};

use crate::actor::{Fatal, GameActor, Phase, not_started};
use crate::running::Rejection;
use crate::store::EventStore;

/// One seat's yield and what it needs to tell when to end.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Active {
    until: YieldUntil,
    /// The turn it was set in; `EndOfTurn` and `MyTurn` end in a later one.
    turn: u32,
    /// The stack as the seat last passed on it, so that only an object added since counts as new.
    seen: BTreeSet<ObjectId>,
}

impl Active {
    fn new(until: YieldUntil, state: &GameState) -> Active {
        Active {
            until,
            turn: state.turn().number(),
            seen: state.stack().iter().copied().collect(),
        }
    }

    /// Whether the yield is over at a priority decision of `seat`, which `can_act` says it could use.
    fn ends_at(&self, state: &GameState, seat: PlayerId, can_act: bool) -> bool {
        let turn = state.turn();
        let reached = match self.until {
            YieldUntil::Resolve => state.stack().is_empty(),
            YieldUntil::EndOfTurn => turn.number() != self.turn,
            YieldUntil::MyTurn => turn.number() != self.turn && turn.active() == seat,
        };
        // An opponent's new object stops the yield only if the seat could respond to it.
        reached || (can_act && self.opponent_added(state, seat))
    }

    /// Whether the stack holds an object of another seat that the seat has not passed on yet.
    fn opponent_added(&self, state: &GameState, seat: PlayerId) -> bool {
        state.stack().iter().any(|id| {
            !self.seen.contains(id)
                && state
                    .object(*id)
                    .is_some_and(|object| object.controller() != seat)
        })
    }
}

/// The yields of every seat of one game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Yields {
    seats: Vec<Option<Active>>,
}

impl Yields {
    /// No seat yields.
    pub(crate) fn new(seats: usize) -> Yields {
        Yields {
            seats: vec![None; seats],
        }
    }

    /// What `seat` is yielding until, if it is.
    pub(crate) fn get(&self, seat: PlayerId) -> Option<YieldUntil> {
        self.seats
            .get(seat.index())
            .and_then(Option::as_ref)
            .map(|active| active.until)
    }

    /// Starts `seat`'s yield from the table as it is now, replacing any it had.
    ///
    /// The stack as it stands counts as seen: the seat asks while looking at it.
    pub(crate) fn start(&mut self, seat: PlayerId, until: YieldUntil, state: &GameState) {
        if let Some(slot) = self.seats.get_mut(seat.index()) {
            *slot = Some(Active::new(until, state));
        }
    }

    /// Ends `seat`'s yield; false if it had none.
    pub(crate) fn cancel(&mut self, seat: PlayerId) -> bool {
        self.seats
            .get_mut(seat.index())
            .is_some_and(|slot| slot.take().is_some())
    }

    /// Whether the server passes for `seat` at a priority decision, which `can_act` says it could use.
    ///
    /// A yield that has reached its end, or that a new opposing object stops, is over from here on and the
    /// answer is no: the decision goes to the seat. Otherwise the stack is noted as passed on.
    pub(crate) fn passes(&mut self, seat: PlayerId, state: &GameState, can_act: bool) -> bool {
        let Some(slot) = self.seats.get_mut(seat.index()) else {
            return false;
        };
        let Some(mut active) = slot.take() else {
            return false;
        };
        if active.ends_at(state, seat, can_act) {
            return false;
        }
        active.seen = state.stack().iter().copied().collect();
        *slot = Some(active);
        true
    }

    /// Ends the yield of every seat that has left the game, and of all of them once it is over.
    pub(crate) fn drop_finished(&mut self, state: &GameState) {
        let over = state.result().is_some();
        for (seat, slot) in self.seats.iter_mut().enumerate() {
            if over || !state.is_alive(PlayerId::from_index(seat)) {
                *slot = None;
            }
        }
    }

    /// Takes `new` as the yields and returns each seat whose yield differs from before, with what it is now.
    pub(crate) fn replace(&mut self, new: Yields) -> Vec<(PlayerId, Option<YieldUntil>)> {
        let old = std::mem::replace(self, new);
        (0..self.seats.len())
            .map(PlayerId::from_index)
            .filter(|seat| old.get(*seat) != self.get(*seat))
            .map(|seat| (seat, self.get(seat)))
            .collect()
    }
}

/// What a seat is told when its yield changes.
pub(crate) fn notice(until: Option<YieldUntil>) -> ServerMessage {
    ServerMessage::Yielding { until }
}

impl<S: EventStore> GameActor<S> {
    /// Starts `seat`'s yield; if the seat faces a priority decision it is passed at once.
    pub(crate) async fn set_yield(
        &mut self,
        seat: PlayerId,
        until: YieldUntil,
    ) -> Result<(), Fatal> {
        let Phase::Running(running) = &mut self.phase else {
            self.refuse(seat, not_started());
            return Ok(());
        };
        let state = running.engine.state();
        if running.engine.result().is_some() || !state.is_alive(seat) {
            let reason = "you are not in a game that is still being played";
            self.refuse(seat, Rejection::new(ErrorCode::BadMessage, reason));
            return Ok(());
        }
        let mut yields = self.yields.clone();
        yields.start(seat, until, state);
        if !running.asks_priority_of(seat) {
            self.commit_yields(yields);
            return Ok(());
        }
        // The decision is sent again with a new id if the yield ended before passing, so the seat is never left
        // without a prompt it still owes an answer to.
        match running.restart_decision(seat) {
            Ok(step) => self.publish_with(step, yields).await,
            Err(rejection) => {
                self.refuse(seat, rejection);
                Ok(())
            }
        }
    }

    /// Ends `seat`'s yield; the seat is told if it had one.
    pub(crate) fn cancel_yield(&mut self, seat: PlayerId) {
        let mut yields = self.yields.clone();
        if yields.cancel(seat) {
            self.commit_yields(yields);
        }
    }

    /// Takes `yields` as the seats' yields and tells each seat whose yield changed.
    fn commit_yields(&mut self, yields: Yields) {
        for (seat, until) in self.yields.replace(yields) {
            self.seats.send(seat, notice(until));
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: a yield passes at every priority window until the thing it waits for happens and ends at exactly that window; an opponent's new object ends it only when the seat can respond, objects the seat already passed on never do, other decisions leave it alone, and it never outlives the seat or the game.
    use super::*;
    use crate::pace::passes_for;
    use mtg_engine::decision::{
        BlockOptions, DecisionKind, PayChoice, PriorityAction, PriorityOptions,
    };
    use mtg_engine::{Answer, Decision, Engine};
    use mtg_testkit::Scenario;
    use mtg_testkit::cards::instant;
    use rstest::rstest;

    const ME: PlayerId = PlayerId(1);
    const FOE: PlayerId = PlayerId(0);
    const ALL: [YieldUntil; 3] = [
        YieldUntil::Resolve,
        YieldUntil::EndOfTurn,
        YieldUntil::MyTurn,
    ];

    /// Turn 1, seat 0's first main phase: seat 0 holds `spells` free instants and seat 1 one, so either can cast
    /// at any time, even over a spell on the stack.
    fn table(spells: usize) -> Engine {
        let zaps = (0..spells).map(|n| instant(&format!("Foe Zap {n}"), 0, &[]));
        let setup = Scenario::new(2)
            .hand(0, zaps)
            .hand(1, [instant("My Zap", 0, &[])])
            .setup()
            .unwrap();
        Engine::new(setup).unwrap()
    }

    /// The holder of priority casts the first spell it can and pays for it.
    fn cast(engine: &mut Engine) {
        let decision = engine.decision().unwrap().clone();
        let DecisionKind::Priority(options) = decision.kind else {
            panic!("expected priority, got {decision:?}");
        };
        let spell = options.castable[0];
        engine
            .apply(Answer::Priority(PriorityAction::CastSpell(spell)))
            .unwrap();
        engine.apply(Answer::PayMana(PayChoice::Auto)).unwrap();
    }

    fn pass(engine: &mut Engine) {
        engine
            .apply(Answer::Priority(PriorityAction::Pass))
            .unwrap();
    }

    /// Passes (or declares nothing) for whoever is asked until the next window of `seat` that opens when
    /// `stop` holds; returns how many windows of `seat` went by.
    fn run_to(
        engine: &mut Engine,
        yields: &mut Yields,
        seat: PlayerId,
        stop: impl Fn(&GameState) -> bool,
    ) -> usize {
        let mut windows = 0;
        for _ in 0..500 {
            let decision = engine.decision().expect("the game goes on").clone();
            if decision.player == seat && stop(engine.state()) {
                return windows;
            }
            if decision.player == seat {
                assert!(
                    yields.passes(seat, engine.state(), false),
                    "the yield ended after {windows} windows, before the stop"
                );
                windows += 1;
            }
            let answer = match &decision.kind {
                DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
                DecisionKind::DeclareAttackers(_) => Answer::DeclareAttackers(Vec::new()),
                DecisionKind::DeclareBlockers(_) => Answer::DeclareBlockers(Vec::new()),
                other => panic!("unexpected {other:?}"),
            };
            engine.apply(answer).unwrap();
        }
        panic!("the stop never came");
    }

    #[rstest]
    #[case::resolve(YieldUntil::Resolve)]
    #[case::end_of_turn(YieldUntil::EndOfTurn)]
    #[case::my_turn(YieldUntil::MyTurn)]
    fn a_seat_starts_without_a_yield_and_a_started_one_passes_over_the_stack_it_saw(
        #[case] until: YieldUntil,
    ) {
        let mut engine = table(1);
        cast(&mut engine);
        let mut yields = Yields::new(2);
        assert_eq!(yields.get(ME), None);
        assert!(
            !yields.passes(ME, engine.state(), true),
            "no yield, no pass"
        );

        yields.start(ME, until, engine.state());
        assert_eq!(yields.get(ME), Some(until));
        assert!(
            yields.passes(ME, engine.state(), true),
            "the spell on the stack was there when the seat asked"
        );
        assert_eq!(yields.get(FOE), None, "another seat is not affected");
    }

    #[test]
    fn resolve_ends_when_the_stack_is_empty_and_not_before() {
        let mut engine = table(1);
        cast(&mut engine);
        let mut yields = Yields::new(2);
        yields.start(ME, YieldUntil::Resolve, engine.state());
        assert!(yields.passes(ME, engine.state(), false));

        // Both pass: the spell resolves and seat 0 holds priority again with an empty stack.
        pass(&mut engine);
        pass(&mut engine);
        assert_eq!(engine.state().stack(), []);
        assert!(!yields.passes(ME, engine.state(), false));
        assert_eq!(yields.get(ME), None, "and the yield is over");
    }

    #[test]
    fn resolve_asked_over_an_empty_stack_ends_at_once() {
        let engine = table(0);
        let mut yields = Yields::new(2);
        yields.start(ME, YieldUntil::Resolve, engine.state());
        assert!(!yields.passes(ME, engine.state(), false));
        assert_eq!(yields.get(ME), None);
    }

    #[test]
    fn end_of_turn_passes_through_the_turn_and_ends_when_the_next_one_begins() {
        let mut engine = table(0);
        let mut yields = Yields::new(2);
        yields.start(ME, YieldUntil::EndOfTurn, engine.state());
        let turn = engine.state().turn().number();

        let windows = run_to(&mut engine, &mut yields, ME, |state| {
            state.turn().number() != turn
        });
        assert!(windows >= 5, "seat 1 was passed for in {windows} windows");
        assert_eq!(engine.state().turn().number(), turn + 1);
        // The first window of the next turn is the seat's own, and it ends the yield.
        assert!(!yields.passes(ME, engine.state(), false));
        assert_eq!(yields.get(ME), None);
    }

    #[rstest]
    #[case::asked_on_the_opponents_turn(FOE, 2)]
    #[case::asked_on_its_own_turn(ME, 3)]
    fn my_turn_ends_when_the_next_turn_of_the_seat_begins(
        #[case] first_turn_of: PlayerId,
        #[case] ends_in_turn: u32,
    ) {
        let setup = Scenario::new(2)
            .at(1, first_turn_of.index(), mtg_engine::turn::Step::Main1)
            .setup()
            .unwrap();
        let mut engine = Engine::new(setup).unwrap();
        let mut yields = Yields::new(2);
        yields.start(ME, YieldUntil::MyTurn, engine.state());

        // Whoever's turn it is when asked, the yield outlasts that turn and the opponent's, and no earlier one.
        let windows = run_to(&mut engine, &mut yields, ME, |state| {
            state.turn().number() != 1 && state.turn().active() == ME
        });
        assert!(windows > 0);
        assert_eq!(engine.state().turn().number(), ends_in_turn);
        assert!(!yields.passes(ME, engine.state(), false));
    }

    #[rstest]
    #[case::could_respond_and_it_is_new(true, false, true)]
    #[case::could_not_respond(false, false, false)]
    #[case::already_passed_on(true, true, false)]
    fn an_opponents_object_ends_the_yield_only_when_it_is_new_and_the_seat_can_respond(
        #[case] can_act: bool,
        #[case] passed_on_it: bool,
        #[case] ends: bool,
    ) {
        for until in ALL {
            let mut engine = table(1);
            let mut yields = Yields::new(2);
            yields.start(ME, until, engine.state());
            cast(&mut engine);
            pass(&mut engine); // seat 0 passes with its spell on the stack; seat 1 is asked
            let state = engine.state();
            assert_eq!(state.stack().len(), 1);
            assert_eq!(engine.decision().map(|d| d.player), Some(ME));
            if passed_on_it {
                // It was there when the seat last passed (before a longer yield replaced the first).
                yields.start(ME, until, state);
            }
            assert_eq!(
                !yields.passes(ME, state, can_act),
                ends,
                "{until:?} can_act {can_act} passed_on_it {passed_on_it}"
            );
        }
    }

    #[test]
    fn the_seats_own_objects_never_stop_it() {
        // Seat 1 is the active player and casts its own instant; a yield that had not seen it still passes.
        let setup = Scenario::new(2)
            .at(1, 1, mtg_engine::turn::Step::Main1)
            .hand(1, [instant("Mine", 0, &[])])
            .setup()
            .unwrap();
        let mut engine = Engine::new(setup).unwrap();
        let mut yields = Yields::new(2);
        yields.start(ME, YieldUntil::EndOfTurn, engine.state());
        cast(&mut engine);
        assert_eq!(engine.state().stack().len(), 1);
        assert!(yields.passes(ME, engine.state(), true));
    }

    #[test]
    fn what_was_passed_on_is_remembered_and_what_left_the_stack_forgotten() {
        let mut engine = table(2);
        cast(&mut engine);
        cast(&mut engine);
        assert_eq!(engine.state().stack().len(), 2);
        let mut yields = Yields::new(2);
        yields.start(ME, YieldUntil::EndOfTurn, engine.state());
        assert!(yields.passes(ME, engine.state(), true));
        assert_eq!(yields.seats[ME.index()].as_ref().unwrap().seen.len(), 2);

        // Both spells resolve; the next pass notes an empty stack.
        for _ in 0..4 {
            pass(&mut engine);
        }
        assert_eq!(engine.state().stack(), []);
        assert!(yields.passes(ME, engine.state(), true));
        assert!(yields.seats[ME.index()].as_ref().unwrap().seen.is_empty());
    }

    #[test]
    fn comparing_yields_names_each_seat_that_changed_and_what_it_holds_now() {
        let engine = table(0);
        let mut old = Yields::new(3);
        old.start(PlayerId(0), YieldUntil::Resolve, engine.state());
        old.start(PlayerId(1), YieldUntil::MyTurn, engine.state());

        let mut new = old.clone();
        assert!(new.cancel(PlayerId(0)));
        assert!(!new.cancel(PlayerId(0)), "nothing left to cancel");
        new.start(PlayerId(1), YieldUntil::EndOfTurn, engine.state());
        new.start(PlayerId(2), YieldUntil::Resolve, engine.state());

        let mut current = old.clone();
        assert_eq!(
            current.replace(new),
            [
                (PlayerId(0), None),
                (PlayerId(1), Some(YieldUntil::EndOfTurn)),
                (PlayerId(2), Some(YieldUntil::Resolve)),
            ]
        );
        let same = current.clone();
        assert_eq!(current.replace(same), [], "the same yields change nothing");
    }

    #[test]
    fn a_seat_that_left_and_a_finished_game_have_no_yield() {
        let setup = Scenario::new(3).setup().unwrap();
        let mut engine = Engine::new(setup).unwrap();
        let mut yields = Yields::new(3);
        for seat in 0..3 {
            yields.start(PlayerId(seat), YieldUntil::Resolve, engine.state());
        }
        yields.drop_finished(engine.state());
        assert_eq!(yields.get(PlayerId(0)), Some(YieldUntil::Resolve));

        engine.concede(PlayerId(2)).unwrap();
        yields.drop_finished(engine.state());
        assert_eq!(yields.get(PlayerId(2)), None, "the seat that left");
        assert_eq!(yields.get(PlayerId(0)), Some(YieldUntil::Resolve));
        assert_eq!(yields.get(PlayerId(1)), Some(YieldUntil::Resolve));

        engine.concede(PlayerId(1)).unwrap();
        assert!(engine.state().result().is_some(), "the game is over");
        yields.drop_finished(engine.state());
        assert_eq!(yields.get(PlayerId(0)), None, "so is everyone's yield");
    }

    #[test]
    fn an_unknown_seat_never_yields() {
        let engine = table(0);
        let mut yields = Yields::new(2);
        yields.start(PlayerId(5), YieldUntil::Resolve, engine.state());
        assert_eq!(yields.get(PlayerId(5)), None);
        assert!(!yields.cancel(PlayerId(5)));
        assert!(!yields.passes(PlayerId(5), engine.state(), true));
    }

    fn priority(player: PlayerId, can_act: bool) -> Decision {
        Decision {
            player,
            kind: DecisionKind::Priority(PriorityOptions {
                playable_lands: Vec::new(),
                castable: Vec::new(),
                mana_abilities: Vec::new(),
                can_act,
                can_pass: true,
            }),
        }
    }

    /// What the actor does with a priority decision of seat 1, whose yield is `until`, on the table of
    /// `table(0)` (an empty stack), and whether the yield is still there afterwards.
    #[rstest]
    #[case::passes_for_a_seat_that_could_act(YieldUntil::EndOfTurn, true, false, true, true)]
    #[case::passes_for_an_idle_seat_without_auto_pass(
        YieldUntil::EndOfTurn,
        false,
        false,
        true,
        true
    )]
    #[case::an_ended_yield_hands_an_idle_seat_to_auto_pass(
        YieldUntil::Resolve,
        false,
        true,
        true,
        false
    )]
    #[case::an_ended_yield_asks_an_idle_seat_without_auto_pass(
        YieldUntil::Resolve,
        false,
        false,
        false,
        false
    )]
    #[case::an_ended_yield_asks_a_seat_that_could_act(
        YieldUntil::Resolve,
        true,
        true,
        false,
        false
    )]
    fn a_priority_decision_is_passed_for_by_a_yield_or_by_auto_pass(
        #[case] until: YieldUntil,
        #[case] can_act: bool,
        #[case] auto_pass: bool,
        #[case] passed: bool,
        #[case] still_yielding: bool,
    ) {
        let engine = table(0);
        let mut yields = Yields::new(2);
        yields.start(ME, until, engine.state());
        let flags = [false, auto_pass];
        let decision = priority(ME, can_act);
        assert_eq!(
            passes_for(&decision, &flags, &mut yields, engine.state()),
            passed
        );
        assert_eq!(yields.get(ME).is_some(), still_yielding);
    }

    #[test]
    fn a_decision_that_is_not_priority_is_never_passed_for_and_leaves_the_yield_alone() {
        let engine = table(0);
        let mut yields = Yields::new(2);
        yields.start(ME, YieldUntil::Resolve, engine.state());
        let decision = Decision {
            player: ME,
            kind: DecisionKind::DeclareBlockers(BlockOptions {
                blockers: Vec::new(),
            }),
        };
        assert!(!passes_for(
            &decision,
            &[true, true],
            &mut yields,
            engine.state()
        ));
        assert_eq!(
            yields.get(ME),
            Some(YieldUntil::Resolve),
            "even one that would have ended it at a priority window"
        );
    }
}
