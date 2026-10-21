//! Windows: when the table asks a seat for priority and when it passes for it.
//!
//! [`verdict`] decides at each priority decision of a seat, from the table (the step, whose turn it is, the
//! stack), the seat's stops, its Full Control mode, its yield and whether it can act, whether the seat is asked, passed
//! for at once or passed for after a dwell. The server's auto-pass is the mode: a seat that turned auto-pass off is in
//! `Held`. The rules, in the order they are judged:
//! 1. Full Control (`Held`, `NextWindow`) asks at every window, so a seat in it is never passed for by a yield or by
//!    the default windows.
//! 2. A running yield passes (its own end conditions are in [`crate::yields`]); a stop of the seat at a window ends
//!    it and the seat is asked, except in the step of the window the yield was asked at.
//! 3. A stop of the seat at the window asks, whether the seat can act or not. `Own` stops are for the seat's own
//!    turns, `Opponent` stops for the turn of any other seat.
//! 4. The default windows (Arena's table) ask when the seat can act: the seat's own precombat and postcombat main
//!    phase, the declare attackers and declare blockers steps of any turn, and any moment an object of another seat
//!    is on the stack. Where the seat cannot act the table passes for it after a dwell ([`Verdict::Dwell`]), so the
//!    pace does not show that it holds nothing, at every default window but one: the seat's own main phase with an
//!    empty stack, where what a seat holds shows anyway (a land or a spell is played in plain sight) and it is passed
//!    for at once.
//! 5. Everywhere else (upkeep, draw, beginning of combat, the damage steps, end of combat, an end step, another
//!    seat's main phase with an empty stack) the seat is passed for at once, whatever it holds, and so is a seat that
//!    yields: a yield is the seat's own choice to hurry.

use mtg_core::ids::PlayerId;
use mtg_engine::turn::Step;
use mtg_engine::{Decision, DecisionKind, GameState};
use mtg_protocol::{FullControlMode, StopSide};

use crate::stops::Stops;
use crate::yields::Yields;

/// Where the table stands at one priority decision of one seat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Window {
    /// The step the game is in.
    pub(crate) step: Step,
    /// Whether it is the seat's own turn.
    pub(crate) own_turn: bool,
    /// Whether the stack holds an object, of any seat.
    pub(crate) stack: bool,
    /// Whether the stack holds an object of another seat.
    pub(crate) others_object: bool,
}

impl Window {
    /// The window `seat` meets in `state`.
    pub(crate) fn at(state: &GameState, seat: PlayerId) -> Window {
        let turn = state.turn();
        Window {
            step: turn.step(),
            own_turn: turn.active() == seat,
            stack: !state.stack().is_empty(),
            others_object: state.stack().iter().any(|id| {
                state
                    .object(*id)
                    .is_some_and(|object| object.controller() != seat)
            }),
        }
    }

    /// Which side of the seat's stops this window is on.
    pub(crate) fn side(self) -> StopSide {
        if self.own_turn {
            StopSide::Own
        } else {
            StopSide::Opponent
        }
    }

    /// Whether this is one of the default windows (rule 4).
    pub(crate) fn is_default(self) -> bool {
        self.others_object
            || matches!(self.step, Step::DeclareAttackers | Step::DeclareBlockers)
            || (self.own_turn && self.step.is_main())
    }

    /// Whether a seat that cannot act waits here ([`Verdict::Dwell`]) instead of being passed for at once: at every
    /// default window but the seat's own main phase with an empty stack, where what a seat holds shows anyway.
    pub(crate) fn dwells(self) -> bool {
        self.is_default() && !(self.own_turn && self.step.is_main() && !self.stack)
    }
}

/// Whether the seat is asked at `window` rather than passed for.
///
/// `stop` says the seat holds a stop at this side and step, `mode` is its Full Control mode and `yielding` that its
/// yield is still passing here (it has not ended and no stop interrupts it).
pub(crate) fn asks(
    window: Window,
    can_act: bool,
    stop: bool,
    mode: FullControlMode,
    yielding: bool,
) -> bool {
    mode != FullControlMode::Off || (!yielding && (stop || (window.is_default() && can_act)))
}

/// What the table does at a priority window of a seat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// The seat is sent the decision.
    Ask,
    /// The server passes for the seat at once.
    Pass,
    /// The server passes for the seat after a dwell ([`crate::dwell`]): the window is a default one, other than the
    /// seat's own main phase with an empty stack, that the seat cannot use, and a pass at once would show that it holds
    /// nothing.
    Dwell,
}

/// The verdict at `window`; the facts are those of [`asks`].
pub(crate) fn judge(
    window: Window,
    can_act: bool,
    stop: bool,
    mode: FullControlMode,
    yielding: bool,
) -> Verdict {
    if asks(window, can_act, stop, mode, yielding) {
        Verdict::Ask
    } else if window.dwells() && !yielding {
        // Not asked at a window that waits, with no yield: the seat is in no Full Control, holds no stop and cannot act.
        Verdict::Dwell
    } else {
        Verdict::Pass
    }
}

/// What the table does with `decision`: ask its seat, pass for it at once or pass for it after a dwell.
///
/// Any decision that is not a priority decision is asked. A yield that this window ends, or that a stop interrupts,
/// is dropped from `yields` and the rest of the rules decide.
pub(crate) fn verdict(
    decision: &Decision,
    stops: &Stops,
    yields: &mut Yields,
    state: &GameState,
) -> Verdict {
    let DecisionKind::Priority(options) = &decision.kind else {
        return Verdict::Ask;
    };
    let seat = decision.player;
    let window = Window::at(state, seat);
    let mode = stops.full_control(seat);
    let stop = stops.holds(seat, window.side(), window.step);
    // A seat in Full Control holds no yield; the yield is left alone for the rules below it.
    let yielding = mode == FullControlMode::Off && yields.passes(seat, state, stop);
    judge(window, options.can_act, stop, mode, yielding)
}

/// Whether the server passes for the holder of `decision` instead of asking it, at once or after a dwell.
#[cfg(test)]
pub(crate) fn passes_for(
    decision: &Decision,
    stops: &Stops,
    yields: &mut Yields,
    state: &GameState,
) -> bool {
    verdict(decision, stops, yields, state) != Verdict::Ask
}

#[cfg(test)]
mod tests {
    //! Goal: the verdict (ask, pass at once, pass after a dwell) is the rule table for every combination of whose turn, step, stack, stop, Full Control mode, yield and ability to act; a seat that cannot act waits at every default window but its own main phase with an empty stack, a stop interrupts a yield and Full Control ignores it; windows are read from the table alone, for the seat's own side and objects.
    use super::*;
    use mtg_engine::decision::{PayChoice, PriorityAction, PriorityOptions};
    use mtg_engine::{Answer, Engine};
    use mtg_protocol::YieldUntil;
    use mtg_testkit::Scenario;
    use mtg_testkit::cards::instant;
    use rstest::rstest;

    const ME: PlayerId = PlayerId(1);
    const FOE: PlayerId = PlayerId(0);
    const MODES: [FullControlMode; 3] = [
        FullControlMode::Off,
        FullControlMode::NextWindow,
        FullControlMode::Held,
    ];

    /// The default windows, spelled out by hand: the steps of a turn that are one whoever's turn it is, and the main
    /// phases that are one only on the seat's own turn.
    fn default_by_the_table(step: Step, own_turn: bool, others_object: bool) -> bool {
        const EVERY_TURN: [Step; 2] = [Step::DeclareAttackers, Step::DeclareBlockers];
        const OWN_TURN_ONLY: [Step; 2] = [Step::Main1, Step::Main2];
        others_object || EVERY_TURN.contains(&step) || (own_turn && OWN_TURN_ONLY.contains(&step))
    }

    /// The windows where a seat that cannot act waits, spelled out by hand: every default window but the seat's own
    /// main phase with an empty stack.
    fn dwells_by_the_table(step: Step, own_turn: bool, stack: bool, others_object: bool) -> bool {
        let own_main_with_an_empty_stack =
            own_turn && [Step::Main1, Step::Main2].contains(&step) && !stack;
        default_by_the_table(step, own_turn, others_object) && !own_main_with_an_empty_stack
    }

    /// What the stack holds at a window.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Stack {
        /// Nothing.
        Empty,
        /// Objects of the seat alone.
        Own,
        /// An object of another seat.
        Others,
    }

    impl Stack {
        const ALL: [Stack; 3] = [Stack::Empty, Stack::Own, Stack::Others];

        /// The window at `step` of a turn that is the seat's own or not, with this on the stack.
        fn window(self, step: Step, own_turn: bool) -> Window {
            Window {
                step,
                own_turn,
                stack: self != Stack::Empty,
                others_object: self == Stack::Others,
            }
        }
    }

    /// The verdict written as the rules read, one early answer per rule, instead of the one boolean expression of
    /// [`asks`]. A row of the table is a list of independent facts, so the bools stay bools.
    #[allow(clippy::fn_params_excessive_bools)]
    fn expected(
        default: bool,
        stop: bool,
        mode: FullControlMode,
        yielding: bool,
        can_act: bool,
    ) -> bool {
        if mode != FullControlMode::Off {
            return true; // Full Control asks at every window
        }
        if yielding {
            return false; // a running yield that no stop interrupts passes
        }
        if stop {
            return true; // a stop asks, whether the seat can act or not
        }
        default && can_act // a default window asks a seat that can act; nowhere else is a window
    }

    /// What the table does, written as the rules read: Full Control asks, a yield is the seat's choice to hurry and
    /// passes at once, a stop asks, a default window asks a seat that can act and makes one that cannot wait unless it
    /// is the seat's own main phase with an empty stack, and the rest of the game passes at once.
    #[allow(clippy::fn_params_excessive_bools)]
    fn expected_verdict(
        default: bool,
        dwells: bool,
        stop: bool,
        mode: FullControlMode,
        yielding: bool,
        can_act: bool,
    ) -> Verdict {
        if mode != FullControlMode::Off {
            return Verdict::Ask;
        }
        if yielding {
            return Verdict::Pass;
        }
        if stop || (default && can_act) {
            return Verdict::Ask;
        }
        if dwells {
            Verdict::Dwell
        } else {
            Verdict::Pass
        }
    }

    #[test]
    fn every_combination_of_turn_step_stack_stop_mode_yield_and_ability_gives_the_table_verdict() {
        let mut cases = 0;
        for own_turn in [false, true] {
            for step in Step::ALL {
                for stack in Stack::ALL {
                    let window = stack.window(step, own_turn);
                    let default = default_by_the_table(step, own_turn, window.others_object);
                    let dwells =
                        dwells_by_the_table(step, own_turn, window.stack, window.others_object);
                    assert_eq!(window.is_default(), default, "{window:?}");
                    assert_eq!(window.dwells(), dwells, "{window:?}");
                    for stop in [false, true] {
                        for mode in MODES {
                            for yielding in [false, true] {
                                for can_act in [false, true] {
                                    cases += 1;
                                    assert_eq!(
                                        asks(window, can_act, stop, mode, yielding),
                                        expected(default, stop, mode, yielding, can_act),
                                        "{window:?} stop {stop} {mode:?} yield {yielding} can act {can_act}"
                                    );
                                    assert_eq!(
                                        judge(window, can_act, stop, mode, yielding),
                                        expected_verdict(
                                            default, dwells, stop, mode, yielding, can_act
                                        ),
                                        "{window:?} stop {stop} {mode:?} yield {yielding} can act {can_act}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 2 * 13 * 3 * 2 * 3 * 2 * 2);
    }

    #[test]
    fn the_default_windows_are_exactly_the_mains_of_the_seat_the_combat_declarations_and_an_object_to_answer()
     {
        let own = |step| default_by_the_table(step, true, false);
        let other = |step| default_by_the_table(step, false, false);
        let own_windows: Vec<Step> = Step::ALL.into_iter().filter(|s| own(*s)).collect();
        let other_windows: Vec<Step> = Step::ALL.into_iter().filter(|s| other(*s)).collect();
        assert_eq!(
            own_windows,
            [
                Step::Main1,
                Step::DeclareAttackers,
                Step::DeclareBlockers,
                Step::Main2
            ]
        );
        assert_eq!(
            other_windows,
            [Step::DeclareAttackers, Step::DeclareBlockers]
        );
        // An object of another seat on the stack makes every step one, on either turn.
        assert!(
            Step::ALL
                .into_iter()
                .all(|step| default_by_the_table(step, true, true)
                    && default_by_the_table(step, false, true))
        );
    }

    #[test]
    fn the_side_of_a_window_follows_whose_turn_it_is() {
        for (own_turn, side) in [(true, StopSide::Own), (false, StopSide::Opponent)] {
            let window = Stack::Empty.window(Step::End, own_turn);
            assert_eq!(window.side(), side);
        }
    }

    /// Turn 1, seat 0's first main phase: seat 0 holds `spells` free instants and seat 1 one.
    fn duel(spells: usize) -> Engine {
        let zaps = (0..spells).map(|n| instant(&format!("Foe Zap {n}"), 0, &[]));
        let setup = Scenario::new(2)
            .hand(0, zaps)
            .hand(1, [instant("My Zap", 0, &[])])
            .setup()
            .unwrap();
        Engine::new(setup).unwrap()
    }

    fn priority(player: PlayerId, can_act: bool) -> Decision {
        Decision {
            player,
            kind: DecisionKind::Priority(Box::new(PriorityOptions {
                playable_lands: Vec::new(),
                castable: Vec::new(),
                mana_abilities: Vec::new(),
                abilities: Vec::new(),
                suspendable: Vec::new(),
                can_act,
                can_pass: true,
            })),
        }
    }

    #[test]
    fn a_window_is_read_from_the_stack_for_the_seat_that_meets_it() {
        let mut engine = duel(1);
        let quiet = Window::at(engine.state(), ME);
        assert_eq!(
            quiet,
            Window {
                step: Step::Main1,
                own_turn: false,
                stack: false,
                others_object: false
            }
        );
        assert!(
            !quiet.is_default(),
            "another seat's main phase, nothing to answer"
        );
        assert!(Window::at(engine.state(), FOE).own_turn);

        let decision = engine.decision().unwrap().clone();
        let DecisionKind::Priority(options) = decision.kind else {
            panic!("seat 0 holds priority");
        };
        engine
            .apply(Answer::Priority(PriorityAction::CastSpell(
                options.castable[0],
            )))
            .unwrap();
        engine
            .apply(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))
            .unwrap();
        assert_eq!(engine.state().stack().len(), 1);
        assert!(
            Window::at(engine.state(), ME).others_object,
            "seat 0's spell waits for seat 1"
        );
        let own = Window::at(engine.state(), FOE);
        assert!(
            own.stack && !own.others_object,
            "and is not another seat's object to seat 0, though it is on the stack"
        );
    }

    #[test]
    fn a_decision_that_is_not_priority_is_never_passed_for() {
        let engine = duel(0);
        let blockers = Decision {
            player: ME,
            kind: DecisionKind::DeclareBlockers(mtg_engine::decision::BlockOptions {
                blockers: Vec::new(),
                min_blockers: Vec::new(),
                required: Vec::new(),
            }),
        };
        let stops = Stops::new(2);
        let mut yields = Yields::new(2);
        yields.start(ME, YieldUntil::EndOfTurn, engine.state());
        assert!(!passes_for(&blockers, &stops, &mut yields, engine.state()));
        assert_eq!(yields.get(ME), Some(YieldUntil::EndOfTurn));
    }

    #[test]
    fn a_stop_ends_a_yield_at_its_window_and_a_yield_without_one_goes_on_passing() {
        let engine = duel(0);
        // Seat 1 meets the first main phase of seat 0: an opponent's step, no default window.
        let here = Window::at(engine.state(), ME);
        let decision = priority(ME, true);
        for stop in [false, true] {
            let mut stops = Stops::new(2);
            if stop {
                let held = mtg_protocol::Stop {
                    side: here.side(),
                    step: here.step,
                    keep: true,
                };
                stops.set(ME, held).unwrap();
            }
            let mut yields = Yields::new(2);
            yields.start(ME, YieldUntil::EndOfTurn, engine.state());
            assert_eq!(
                passes_for(&decision, &stops, &mut yields, engine.state()),
                !stop,
                "stop {stop}"
            );
            assert_eq!(yields.get(ME).is_some(), !stop, "the stop ends the yield");
        }
    }

    #[test]
    fn a_stop_does_not_end_a_yield_asked_at_that_very_window() {
        let engine = duel(0);
        let here = Window::at(engine.state(), ME);
        let mut stops = Stops::new(2);
        let held = mtg_protocol::Stop {
            side: here.side(),
            step: here.step,
            keep: true,
        };
        stops.set(ME, held).unwrap();
        let mut yields = Yields::new(2);
        yields.start_facing(ME, YieldUntil::EndOfTurn, engine.state());
        assert!(passes_for(
            &priority(ME, true),
            &stops,
            &mut yields,
            engine.state()
        ));
        assert_eq!(yields.get(ME), Some(YieldUntil::EndOfTurn));
    }

    #[test]
    fn full_control_asks_and_leaves_a_yield_it_does_not_hold_alone() {
        let engine = duel(0);
        let decision = priority(ME, false);
        for mode in [FullControlMode::NextWindow, FullControlMode::Held] {
            let mut stops = Stops::new(2);
            stops.set_full_control(ME, mode);
            let mut yields = Yields::new(2);
            yields.start(ME, YieldUntil::EndOfTurn, engine.state());
            assert!(!passes_for(&decision, &stops, &mut yields, engine.state()));
            assert_eq!(yields.get(ME), Some(YieldUntil::EndOfTurn), "{mode:?}");
        }
    }

    /// A window of each kind, as the seat meets it: the verdict for a seat that cannot act, and for one that can.
    #[rstest]
    #[case::own_main_with_an_empty_stack(
        Step::Main1,
        true,
        Stack::Empty,
        Verdict::Pass,
        Verdict::Ask
    )]
    #[case::own_second_main_with_an_empty_stack(
        Step::Main2,
        true,
        Stack::Empty,
        Verdict::Pass,
        Verdict::Ask
    )]
    #[case::own_main_with_its_own_spell(
        Step::Main1,
        true,
        Stack::Own,
        Verdict::Dwell,
        Verdict::Ask
    )]
    #[case::own_second_main_with_an_opponents_spell(
        Step::Main2,
        true,
        Stack::Others,
        Verdict::Dwell,
        Verdict::Ask
    )]
    #[case::declare_attackers_on_its_own_turn(
        Step::DeclareAttackers,
        true,
        Stack::Empty,
        Verdict::Dwell,
        Verdict::Ask
    )]
    #[case::declare_attackers_on_another_turn(
        Step::DeclareAttackers,
        false,
        Stack::Empty,
        Verdict::Dwell,
        Verdict::Ask
    )]
    #[case::declare_blockers_on_its_own_turn(
        Step::DeclareBlockers,
        true,
        Stack::Empty,
        Verdict::Dwell,
        Verdict::Ask
    )]
    #[case::declare_blockers_on_another_turn(
        Step::DeclareBlockers,
        false,
        Stack::Empty,
        Verdict::Dwell,
        Verdict::Ask
    )]
    #[case::an_opponents_spell_in_its_main_phase(
        Step::Main1,
        false,
        Stack::Others,
        Verdict::Dwell,
        Verdict::Ask
    )]
    #[case::an_opponents_spell_in_an_end_step(
        Step::End,
        false,
        Stack::Others,
        Verdict::Dwell,
        Verdict::Ask
    )]
    #[case::an_opponents_spell_in_its_own_upkeep(
        Step::Upkeep,
        true,
        Stack::Others,
        Verdict::Dwell,
        Verdict::Ask
    )]
    #[case::an_opponents_main_with_an_empty_stack(
        Step::Main1,
        false,
        Stack::Empty,
        Verdict::Pass,
        Verdict::Pass
    )]
    #[case::its_own_spell_alone_in_an_end_step(
        Step::End,
        true,
        Stack::Own,
        Verdict::Pass,
        Verdict::Pass
    )]
    #[case::its_own_spell_alone_in_combat(
        Step::BeginCombat,
        true,
        Stack::Own,
        Verdict::Pass,
        Verdict::Pass
    )]
    #[case::upkeep(Step::Upkeep, true, Stack::Empty, Verdict::Pass, Verdict::Pass)]
    #[case::draw(Step::Draw, false, Stack::Empty, Verdict::Pass, Verdict::Pass)]
    #[case::combat_damage(Step::CombatDamage, true, Stack::Empty, Verdict::Pass, Verdict::Pass)]
    #[case::an_end_step(Step::End, false, Stack::Empty, Verdict::Pass, Verdict::Pass)]
    fn each_kind_of_window_gets_its_verdict(
        #[case] step: Step,
        #[case] own_turn: bool,
        #[case] stack: Stack,
        #[case] cannot_act: Verdict,
        #[case] can_act: Verdict,
    ) {
        let window = stack.window(step, own_turn);
        let judged = |can_act| judge(window, can_act, false, FullControlMode::Off, false);
        assert_eq!(judged(false), cannot_act, "a seat that cannot act");
        assert_eq!(judged(true), can_act, "a seat that can");
    }

    /// Turn 1, seat 0's first main phase after it cast its free instant: the spell waits on the stack.
    fn spell_on_the_stack() -> Engine {
        let mut engine = duel(1);
        let decision = engine.decision().unwrap().clone();
        let DecisionKind::Priority(options) = decision.kind else {
            panic!("seat 0 holds priority");
        };
        engine
            .apply(Answer::Priority(PriorityAction::CastSpell(
                options.castable[0],
            )))
            .unwrap();
        engine.apply(Answer::PayMana(PayChoice::Auto)).unwrap();
        engine
    }

    #[test]
    fn the_own_main_phase_passes_at_once_until_a_spell_waits_on_the_stack() {
        let empty = duel(0);
        let spell = spell_on_the_stack();
        let verdict_in = |engine: &Engine, seat, can_act| {
            verdict(
                &priority(seat, can_act),
                &Stops::new(2),
                &mut Yields::new(2),
                engine.state(),
            )
        };
        // Nothing on the stack: seat 0 asks only if it can act; seat 1 meets an ordinary step of seat 0's turn.
        assert_eq!(verdict_in(&empty, FOE, false), Verdict::Pass);
        assert_eq!(verdict_in(&empty, FOE, true), Verdict::Ask);
        assert_eq!(verdict_in(&empty, ME, false), Verdict::Pass);
        assert_eq!(verdict_in(&empty, ME, true), Verdict::Pass);
        // Seat 0's spell waits: its own main phase and the other seat's window are both response windows.
        assert_eq!(verdict_in(&spell, FOE, false), Verdict::Dwell);
        assert_eq!(verdict_in(&spell, FOE, true), Verdict::Ask);
        assert_eq!(verdict_in(&spell, ME, false), Verdict::Dwell);
        assert_eq!(verdict_in(&spell, ME, true), Verdict::Ask);
    }

    #[test]
    fn a_stop_full_control_and_a_yield_are_the_seats_own_choices_and_never_wait() {
        let engine = spell_on_the_stack();
        let state = engine.state();
        // Seat 0 cannot act at a window where it would wait.
        let verdict_for = |stops: &Stops, yields: &mut Yields| {
            verdict(&priority(FOE, false), stops, yields, state)
        };
        let none = Stops::new(2);
        assert_eq!(
            verdict_for(&none, &mut Yields::new(2)),
            Verdict::Dwell,
            "nothing of its own asked for"
        );

        let mut held = Stops::new(2);
        held.set(
            FOE,
            mtg_protocol::Stop {
                side: StopSide::Own,
                step: Step::Main1,
                keep: false,
            },
        )
        .unwrap();
        assert_eq!(
            verdict_for(&held, &mut Yields::new(2)),
            Verdict::Ask,
            "a stop is the seat's own to answer"
        );
        for mode in [FullControlMode::NextWindow, FullControlMode::Held] {
            let mut full = Stops::new(2);
            full.set_full_control(FOE, mode);
            assert_eq!(
                verdict_for(&full, &mut Yields::new(2)),
                Verdict::Ask,
                "Full Control is the seat's own to answer ({mode:?})"
            );
        }
        let mut yields = Yields::new(2);
        yields.start(FOE, YieldUntil::EndOfTurn, state);
        assert_eq!(
            verdict_for(&none, &mut yields),
            Verdict::Pass,
            "a yield is the seat's choice to hurry"
        );
    }

    #[test]
    fn a_decision_that_is_not_priority_is_asked_whatever_the_seat_holds() {
        let engine = duel(0);
        let blockers = Decision {
            player: FOE,
            kind: DecisionKind::DeclareBlockers(mtg_engine::decision::BlockOptions {
                blockers: Vec::new(),
                min_blockers: Vec::new(),
                required: Vec::new(),
            }),
        };
        let verdict = verdict(
            &blockers,
            &Stops::new(2),
            &mut Yields::new(2),
            engine.state(),
        );
        assert_eq!(verdict, Verdict::Ask);
    }

    #[test]
    fn a_seat_that_can_act_is_asked_only_at_a_default_window_or_a_stop() {
        let engine = duel(0);
        let stops = Stops::new(2);
        let mut yields = Yields::new(2);
        // Seat 1 at seat 0's first main phase: no window, whatever it holds.
        for can_act in [false, true] {
            let decision = priority(ME, can_act);
            assert!(passes_for(&decision, &stops, &mut yields, engine.state()));
        }
        // Seat 0 in its own first main phase: a window, which asks only a seat that can act.
        assert!(passes_for(
            &priority(FOE, false),
            &stops,
            &mut yields,
            engine.state()
        ));
        assert!(!passes_for(
            &priority(FOE, true),
            &stops,
            &mut yields,
            engine.state()
        ));
    }

    #[test]
    fn a_triggered_ability_on_the_stack_is_an_object_of_its_controller_to_every_other_seat() {
        let drifter = mtg_testkit::trigger_cards::watcher(
            "Drifter",
            mtg_testkit::trigger_cards::when::this_enters(),
            vec![mtg_testkit::cards::draw(1)],
        );
        let setup = Scenario::new(2).hand(0, [drifter]).setup().unwrap();
        let mut engine = Engine::new(setup).unwrap();
        let decision = engine.decision().unwrap().clone();
        let DecisionKind::Priority(options) = decision.kind else {
            panic!("seat 0 holds priority");
        };
        engine
            .apply(Answer::Priority(PriorityAction::CastSpell(
                options.castable[0],
            )))
            .unwrap();
        engine
            .apply(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))
            .unwrap();
        for _ in 0..3 {
            engine
                .apply(Answer::Priority(PriorityAction::Pass))
                .unwrap();
        }
        // The creature resolved; its trigger waits on the stack for seat 1, who holds priority after seat 0 passed.
        let stack = engine.state().stack();
        assert_eq!(stack.len(), 1);
        let ability = engine.state().object(stack[0]).unwrap();
        assert_eq!(ability.kind(), mtg_engine::object::ObjectKind::Ability);
        assert_eq!(engine.decision().unwrap().player, ME);

        let theirs = Window::at(engine.state(), ME);
        assert!(theirs.others_object && theirs.stack, "{theirs:?}");
        assert!(theirs.is_default() && theirs.dwells());
        let mine = Window::at(engine.state(), FOE);
        assert!(mine.stack && !mine.others_object, "{mine:?}");
        // A seat that cannot answer it waits a dwell, one that can is asked: the trigger is a window like a spell.
        let mode = FullControlMode::Off;
        assert_eq!(judge(theirs, false, false, mode, false), Verdict::Dwell);
        assert_eq!(judge(theirs, true, false, mode, false), Verdict::Ask);
    }
}
