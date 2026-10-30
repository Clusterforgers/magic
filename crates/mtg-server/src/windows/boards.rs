//! Goal: over real boards, a seat that holds only mana abilities (a basic land, Sol Ring, a filter land that costs mana), a spell it cannot pay for or no play at all is never asked at a window and one that can play a land, cast a spell or activate a non-mana ability is, at each kind of window, with stops, Full Control and yields deciding as they do for any seat; and in seeded four-seat Commander games the table asks no priority window that is not a play, a stop, Full Control or a yield.
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::decision::{Decision, DecisionKind, PayChoice, PriorityAction, PriorityOptions};
use mtg_engine::turn::Step;
use mtg_engine::{Answer, Engine};
use mtg_protocol::{FullControlMode, Stop, YieldUntil};
use mtg_testkit::bot::apply_answer;
use mtg_testkit::cards::{forest, free_creature, instant, island, mountain, plains, swamp};
use mtg_testkit::{RandomBot, Scenario, ready};
use rstest::rstest;

use super::tests::{default_by_the_table, dwells_by_the_table, expected_verdict};
use super::{Verdict, Window, verdict};
use crate::stops::Stops;
use crate::yields::Yields;

const ME: PlayerId = PlayerId(1);

/// The windows a seat meets, by whose turn it is and what waits on the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spot {
    /// Its own first main phase, the stack empty.
    OwnMain,
    /// Its own first main phase with a spell of its own on the stack.
    OwnMainOwnSpell,
    /// Another seat's first main phase, the stack empty.
    FoeMain,
    /// Another seat's first main phase with that seat's spell on the stack.
    FoeMainFoeSpell,
    /// Another seat's end step.
    FoeEnd,
}

const SPOTS: [Spot; 5] = [
    Spot::OwnMain,
    Spot::OwnMainOwnSpell,
    Spot::FoeMain,
    Spot::FoeMainFoeSpell,
    Spot::FoeEnd,
];

/// A card of the testkit (the basic lands) or of the corpus, by name.
fn def(name: &str) -> Arc<CardDefinition> {
    match name {
        "Forest" => forest(),
        "Island" => island(),
        "Mountain" => mountain(),
        "Plains" => plains(),
        "Swamp" => swamp(),
        other => mtg_testkit::corpus::card(other).unwrap(),
    }
}

fn priority(decision: &Decision) -> &PriorityOptions {
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("priority was expected, got {decision:?}");
    };
    options
}

/// The card called `name` among `ids`.
fn named(engine: &Engine, ids: &[ObjectId], name: &str) -> ObjectId {
    *ids.iter()
        .find(|id| engine.state().object(**id).unwrap().card().name == name)
        .unwrap()
}

/// Casts `card` for its holder and answers the payment, if one is asked.
fn cast(engine: &mut Engine, card: ObjectId) {
    engine
        .apply(Answer::Priority(PriorityAction::CastSpell(card)))
        .unwrap();
    while matches!(
        engine.decision().map(|d| &d.kind),
        Some(DecisionKind::PayMana(_))
    ) {
        engine.apply(Answer::PayMana(PayChoice::Auto)).unwrap();
    }
}

fn pass(engine: &mut Engine) {
    engine
        .apply(Answer::Priority(PriorityAction::Pass))
        .unwrap();
}

/// The game at `spot`, at the priority decision of seat `ME`, who holds `hand` and `board`.
fn at(spot: Spot, hand: &[&str], board: &[&str]) -> Engine {
    let (active, step) = match spot {
        Spot::OwnMain | Spot::OwnMainOwnSpell => (1, Step::Main1),
        Spot::FoeMain | Spot::FoeMainFoeSpell => (0, Step::Main1),
        Spot::FoeEnd => (0, Step::End),
    };
    let mut mine: Vec<_> = hand.iter().map(|name| def(name)).collect();
    if spot == Spot::OwnMainOwnSpell {
        mine.push(free_creature("Spare Bear", 2, 2));
    }
    let mut engine = Scenario::new(2)
        .at(2, active, step)
        .hand(1, mine)
        .hand(0, [instant("Foe Zap", 0, &[])])
        .battlefield(1, board.iter().map(|name| ready(def(name))))
        .build()
        .unwrap();
    match spot {
        Spot::OwnMain => {}
        Spot::OwnMainOwnSpell => {
            let spare = named(
                &engine,
                &priority(engine.decision().unwrap()).castable,
                "Spare Bear",
            );
            cast(&mut engine, spare);
        }
        Spot::FoeMain | Spot::FoeEnd => pass(&mut engine),
        Spot::FoeMainFoeSpell => {
            let zap = priority(engine.decision().unwrap()).castable[0];
            cast(&mut engine, zap);
            pass(&mut engine);
        }
    }
    assert_eq!(engine.decision().unwrap().player, ME, "{spot:?}");
    engine
}

use Verdict::{Ask, Dwell, Pass};

/// Seat states against the windows, in the order of [`SPOTS`]: with no stop, no Full Control and no yield the table asks
/// exactly the seat that has a play; it passes for the others, at once or after a dwell where a pass at once would show
/// that they hold nothing.
#[rstest]
#[case::nothing(&[], &[], [Pass, Dwell, Pass, Dwell, Pass])]
#[case::a_basic_land(&[], &["Forest"], [Pass, Dwell, Pass, Dwell, Pass])]
#[case::sol_ring(&[], &["Sol Ring"], [Pass, Dwell, Pass, Dwell, Pass])]
#[case::lands_and_mana_rocks(&[], &["Forest", "Sol Ring", "Arcane Signet"], [Pass, Dwell, Pass, Dwell, Pass])]
#[case::a_filter_land_alone(&[], &["Mystic Gate"], [Pass, Dwell, Pass, Dwell, Pass])]
#[case::a_filter_land_with_a_land_to_pay_it(&[], &["Mystic Gate", "Plains"], [Pass, Dwell, Pass, Dwell, Pass])]
#[case::a_spell_with_no_mana(&["Lightning Bolt"], &[], [Pass, Dwell, Pass, Dwell, Pass])]
#[case::a_spell_of_the_wrong_colour(&["Lightning Bolt"], &["Island"], [Pass, Dwell, Pass, Dwell, Pass])]
#[case::a_land_to_play(&["Forest"], &[], [Ask, Dwell, Pass, Dwell, Pass])]
#[case::an_instant_it_can_pay(&["Lightning Bolt"], &["Mountain"], [Ask, Ask, Pass, Ask, Pass])]
#[case::an_ability_at_instant_speed(&[], &["Perpetual Timepiece"], [Ask, Ask, Pass, Ask, Pass])]
#[case::an_ability_at_sorcery_speed(&[], &["Bonesplitter", "Llanowar Elves", "Plains"], [Ask, Dwell, Pass, Dwell, Pass])]
fn a_seat_is_asked_exactly_where_it_has_a_play(
    #[case] hand: &[&str],
    #[case] board: &[&str],
    #[case] expected: [Verdict; 5],
) {
    for (spot, expected) in SPOTS.into_iter().zip(expected) {
        let engine = at(spot, hand, board);
        let got = verdict(
            engine.decision().unwrap(),
            &Stops::new(2),
            &mut Yields::new(2),
            engine.state(),
        );
        assert_eq!(got, expected, "{spot:?}: hand {hand:?}, board {board:?}");
    }
}

/// A seat with no play and one with a play, at every window, under every stop, Full Control mode and yield: the verdict
/// is the rule table of the windows, read from the `can_act` the engine states.
#[rstest]
#[case::no_play(&[], &["Forest", "Mystic Gate", "Sol Ring"], false)]
#[case::a_play(&["Lightning Bolt"], &["Mountain"], true)]
fn stops_full_control_and_yields_decide_the_same_for_every_seat(
    #[case] hand: &[&str],
    #[case] board: &[&str],
    #[case] plays: bool,
) {
    for spot in SPOTS {
        let engine = at(spot, hand, board);
        let decision = engine.decision().unwrap();
        let can_act = priority(decision).can_act;
        assert_eq!(can_act, plays, "{spot:?}");
        let window = Window::at(engine.state(), ME);
        let (step, own, others) = (window.step, window.own_turn, window.others_object);
        let default = default_by_the_table(step, own, others);
        let dwells = dwells_by_the_table(step, own, window.stack, others);
        for stop in [false, true] {
            for mode in [
                FullControlMode::Off,
                FullControlMode::NextWindow,
                FullControlMode::Held,
            ] {
                for yielding in [false, true] {
                    let mut stops = Stops::new(2);
                    stops.set_full_control(ME, mode);
                    if stop {
                        let held = Stop {
                            side: window.side(),
                            step,
                            keep: true,
                        };
                        stops.set(ME, held).unwrap();
                    }
                    let mut yields = Yields::new(2);
                    if yielding {
                        yields.start_facing(ME, YieldUntil::EndOfTurn, engine.state());
                    }
                    assert_eq!(
                        verdict(decision, &stops, &mut yields, engine.state()),
                        expected_verdict(default, dwells, stop, mode, yielding, can_act),
                        "{spot:?} stop {stop} {mode:?} yield {yielding} can act {can_act}"
                    );
                }
            }
        }
    }
}

/// What a seeded game asked of its seats.
#[derive(Debug, Default)]
struct Asked {
    /// Priority windows the table sent a seat.
    windows: usize,
    /// Priority windows it passed for.
    passed: usize,
    /// Windows asked of a seat that could not play: none may be.
    idle: usize,
    /// Windows asked of a seat whose only plays were abilities that are not mana abilities.
    abilities_only: usize,
}

/// Plays a four-seat Commander game of the corpus for `limit` decisions with the table's windows: no stops, Full Control
/// off, no yield.
fn commander_game(seed: u64, limit: usize) -> Asked {
    let mut engine = Engine::new(mtg_testkit::commander_fuzz_setup(4, seed).unwrap()).unwrap();
    let mut bots: Vec<RandomBot> = (0..4).map(|seat| RandomBot::new(seed, seat)).collect();
    let (stops, mut yields) = (Stops::new(4), Yields::new(4));
    let mut asked = Asked::default();
    for _ in 0..limit {
        let Some(decision) = engine.decision().cloned() else {
            break;
        };
        let seat = decision.player.index();
        let answer = match &decision.kind {
            DecisionKind::Priority(options) => {
                if verdict(&decision, &stops, &mut yields, engine.state()) == Verdict::Ask {
                    asked.windows += 1;
                    asked.idle += usize::from(!options.can_act);
                    asked.abilities_only += usize::from(
                        options.playable_lands.is_empty()
                            && options.castable.is_empty()
                            && !options.abilities.is_empty(),
                    );
                    bots[seat].choose_in(&decision, engine.state())
                } else {
                    asked.passed += 1;
                    Answer::Priority(PriorityAction::Pass)
                }
            }
            _ => bots[seat].choose_in(&decision, engine.state()),
        };
        apply_answer(&mut engine, &decision, answer).unwrap();
    }
    asked
}

#[rstest]
fn a_table_of_bots_is_asked_no_priority_window_that_is_not_a_play(#[values(1, 2, 3)] seed: u64) {
    let asked = commander_game(seed, 3_000);
    assert_eq!(asked.idle, 0, "{asked:?}");
    assert!(asked.windows > 20, "the seats play: {asked:?}");
    assert!(asked.abilities_only > 0, "boards with abilities: {asked:?}");
    assert!(
        asked.passed > asked.windows,
        "the table passes for idle seats: {asked:?}"
    );
}
