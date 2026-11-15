//! Goal: a Spirit (a fallen hero of Delves) is asked at priority only where its haunt actions matter (its team's upkeep and main phases, the boss side's beginning of combat and declare attackers step when it attacks the party, and any moment an object of the boss side is on the stack, each with an empty stack unless that object is the boss side's); its team's own spells and abilities never ask it and every other step is passed for at once; a Spirit that cannot act waits where it would be asked; a stop, Full Control and a yield decide for a Spirit as for any seat; a seat that is not a Spirit keeps the default windows.
use std::collections::BTreeSet;
use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::GameState;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::turn::Step;
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_protocol::{FullControlMode, Stop, StopSide, YieldUntil};
use mtg_testkit::bot::apply_answer;
use mtg_testkit::cards::{free_creature, instant, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use super::{Haunt, Verdict, Window, judge, verdict};
use crate::stops::Stops;
use crate::yields::Yields;

use Verdict::{Ask, Dwell, Pass};

const HERO: PlayerId = PlayerId(0);
const SPIRIT: PlayerId = PlayerId(1);
const BOSS: PlayerId = PlayerId(2);

/// Whose objects wait on the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Stack {
    /// Nothing.
    Empty,
    /// Objects of the party alone, the Spirit's own included.
    Party,
    /// An object of the boss side.
    Boss,
}

/// The restless Spirit of the content, which offers haunt actions.
fn haunting() -> Arc<CardDefinition> {
    mtg_campaign::content()
        .unwrap()
        .card("restless_spirit")
        .unwrap()
        .definition()
}

/// A Spirit emblem with no haunt action: the Spirit holds nothing it can do.
fn bare() -> Arc<CardDefinition> {
    Arc::new(emblem_definition("Haunt", &[]))
}

/// The party (a hero and a Spirit that falls at once) against the boss, under shared team turns from the party's
/// upkeep of turn 1. Each seat has a library; the hero and the boss hold a free instant and the boss a ready creature.
fn table(emblem: Arc<CardDefinition>) -> Scenario {
    let mut scenario = Scenario::new(3)
        .house_rules(HouseRules {
            teams: Some(Teams {
                mode: TeamMode::SharedTurns,
                seats: vec![0, 0, 1],
            }),
            ..HouseRules::default()
        })
        .at(1, 0, Step::Upkeep)
        .life(1, 0)
        .life(2, 40)
        .spirit(1, emblem)
        .hand(0, (0..4).map(|n| instant(&format!("Hero Zap {n}"), 0, &[])))
        .hand(2, [instant("Boss Zap", 0, &[])])
        .battlefield(2, [ready(free_creature("Brute", 3, 3))]);
    for seat in 0..3 {
        scenario = scenario.library(seat, (0..20).map(|_| mountain()));
    }
    scenario
}

/// One priority decision of the Spirit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Row {
    turn: u32,
    step: Step,
    stack: Stack,
    verdict: Verdict,
}

/// A spell a seat casts the first time it holds priority at that turn and step with an empty stack.
type Cast = (PlayerId, u32, Step);

fn stack_of(state: &GameState) -> Stack {
    let mut seen = Stack::Empty;
    for id in state.stack() {
        let boss = state
            .object(*id)
            .is_some_and(|object| object.controller() == BOSS);
        seen = seen.max(if boss { Stack::Boss } else { Stack::Party });
    }
    seen
}

/// Plays the table through `turns` turns: seats pass, the boss attacks the hero with everything it has, `casts` are
/// cast as scripted; returns what the table did with each priority decision of the Spirit under `stops` and `yields`.
fn walk(
    scenario: Scenario,
    who: PlayerId,
    stops: &Stops,
    yields: &Yields,
    casts: &[Cast],
    turns: u32,
) -> Vec<Row> {
    let mut engine = scenario.build().unwrap();
    let mut cast = BTreeSet::new();
    let mut rows = Vec::new();
    for _ in 0..3_000 {
        let state = engine.state();
        let (turn, step) = (state.turn().number(), state.turn().step());
        if turn > turns || state.result().is_some() {
            break;
        }
        let decision = engine.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(options) => {
                if decision.player == who {
                    rows.push(Row {
                        turn,
                        step,
                        stack: stack_of(state),
                        verdict: verdict(&decision, stops, &mut yields.clone(), state),
                    });
                }
                let scripted = casts.iter().position(|(seat, t, s)| {
                    *seat == decision.player
                        && (*t, *s) == (turn, step)
                        && state.stack().is_empty()
                        && !cast.contains(&(*seat, *t, *s))
                });
                match (scripted, options.castable.first()) {
                    (Some(at), Some(spell)) => {
                        cast.insert(casts[at]);
                        Answer::Priority(PriorityAction::CastSpell(*spell))
                    }
                    _ => Answer::Priority(PriorityAction::Pass),
                }
            }
            DecisionKind::PayMana(_) => Answer::PayMana(PayChoice::Auto),
            DecisionKind::DeclareAttackers(options) => Answer::DeclareAttackers(
                options
                    .attackers
                    .iter()
                    .filter_map(|a| a.defenders.first().map(|target| (a.object, *target)))
                    .collect(),
            ),
            _ => Driver::passive_answer(&decision),
        };
        apply_answer(&mut engine, &decision, answer).unwrap();
    }
    rows
}

fn plain(emblem: Arc<CardDefinition>, casts: &[Cast], turns: u32) -> Vec<Row> {
    walk(
        table(emblem),
        SPIRIT,
        &Stops::new(3),
        &Yields::new(3),
        casts,
        turns,
    )
}

/// The `(turn, step)` of the rows with `verdict`, in order.
fn spots(rows: &[Row], verdict: Verdict) -> Vec<(u32, Step)> {
    rows.iter()
        .filter(|row| row.verdict == verdict)
        .map(|row| (row.turn, row.step))
        .collect()
}

/// The party's turns are turns 1 and 3, the boss's turn 2.
#[test]
fn a_spirit_is_asked_in_its_teams_upkeep_and_main_phases_and_at_the_bosss_combat_and_nowhere_else()
{
    let rows = plain(haunting(), &[], 3);
    assert_eq!(
        spots(&rows, Ask),
        [
            (1, Step::Upkeep),
            (1, Step::Main1),
            (1, Step::Main2),
            (2, Step::BeginCombat),
            (2, Step::DeclareAttackers),
            (3, Step::Upkeep),
            (3, Step::Main1),
            (3, Step::Main2),
        ],
        "{rows:#?}"
    );
    assert!(spots(&rows, Dwell).is_empty(), "it can act: {rows:#?}");
    // The Spirit meets its own ectoplasm trigger in the upkeep and passes for it.
    let upkeep: Vec<_> = rows
        .iter()
        .filter(|row| row.step == Step::Upkeep && row.turn == 3)
        .map(|row| (row.stack, row.verdict))
        .collect();
    assert_eq!(upkeep, [(Stack::Party, Pass), (Stack::Empty, Ask)]);
    // Every other step of both turns is met and passed for.
    for step in [
        Step::Draw,
        Step::End,
        Step::DeclareBlockers,
        Step::CombatDamage,
    ] {
        assert!(
            rows.iter()
                .any(|row| row.step == step && row.verdict == Pass),
            "{step:?}: {rows:#?}"
        );
    }
}

#[test]
fn a_spell_of_the_party_never_asks_the_spirit_in_any_step() {
    let casts = [
        (HERO, 1, Step::Main1),
        (HERO, 1, Step::Main2),
        (HERO, 2, Step::BeginCombat),
        (HERO, 2, Step::DeclareAttackers),
    ];
    let rows = plain(haunting(), &casts, 2);
    let with_party_spell: Vec<_> = rows
        .iter()
        .filter(|row| row.stack == Stack::Party)
        .map(|row| row.step)
        .collect();
    for step in [
        Step::Main1,
        Step::Main2,
        Step::BeginCombat,
        Step::DeclareAttackers,
    ] {
        assert!(
            with_party_spell.contains(&step),
            "the Spirit meets a spell in {step:?}: {rows:#?}"
        );
    }
    assert!(
        rows.iter()
            .filter(|row| row.stack == Stack::Party && row.step != Step::Upkeep)
            .all(|row| row.verdict == Pass),
        "{rows:#?}"
    );
    // The windows with an empty stack are the same as without the spells.
    assert_eq!(
        spots(&rows, Ask),
        spots(&plain(haunting(), &[], 2), Ask),
        "{rows:#?}"
    );
}

#[rstest]
#[case::the_party_main_phase(Step::Main1, 1)]
#[case::the_party_second_main_phase(Step::Main2, 1)]
#[case::the_bosss_main_phase(Step::Main1, 2)]
#[case::the_bosss_end_step(Step::End, 2)]
#[case::the_bosss_declare_attackers(Step::DeclareAttackers, 2)]
fn a_spell_of_the_boss_asks_the_spirit_in_any_step(#[case] step: Step, #[case] turn: u32) {
    let rows = plain(haunting(), &[(BOSS, turn, step)], 2);
    let answered: Vec<_> = rows
        .iter()
        .filter(|row| row.stack == Stack::Boss)
        .map(|row| (row.turn, row.step, row.verdict))
        .collect();
    assert_eq!(answered, [(turn, step, Ask)], "{rows:#?}");
}

#[test]
fn a_spirit_that_cannot_act_waits_where_it_would_be_asked_and_never_at_its_own_main_phase_with_an_empty_stack()
 {
    let rows = plain(bare(), &[(BOSS, 2, Step::Main1)], 2);
    assert_eq!(
        spots(&rows, Dwell),
        [
            (1, Step::Upkeep),
            (2, Step::Main1),
            (2, Step::BeginCombat),
            (2, Step::DeclareAttackers),
        ],
        "{rows:#?}"
    );
    assert!(spots(&rows, Ask).is_empty(), "{rows:#?}");
    for row in &rows {
        if row.step.is_main() && row.turn == 1 {
            assert_eq!(row.verdict, Pass, "{row:?}");
        }
    }
}

#[rstest]
#[case::the_bosss_end_step(StopSide::Opponent, Step::End, 2)]
#[case::the_bosss_draw_step(StopSide::Opponent, Step::Draw, 2)]
#[case::the_partys_end_step(StopSide::Own, Step::End, 1)]
#[case::the_partys_beginning_of_combat(StopSide::Own, Step::BeginCombat, 1)]
#[case::the_bosss_declare_blockers(StopSide::Opponent, Step::DeclareBlockers, 2)]
fn a_stop_of_the_spirit_asks_it_there_whether_it_can_act_or_not(
    #[case] side: StopSide,
    #[case] step: Step,
    #[case] turn: u32,
    #[values(true, false)] can_act: bool,
) {
    let mut stops = Stops::new(3);
    stops
        .set(
            SPIRIT,
            Stop {
                side,
                step,
                keep: true,
            },
        )
        .unwrap();
    let emblem = if can_act { haunting() } else { bare() };
    let rows = walk(table(emblem), SPIRIT, &stops, &Yields::new(3), &[], 2);
    let stopped: Vec<_> = rows
        .iter()
        .filter(|row| (row.turn, row.step) == (turn, step))
        .map(|row| row.verdict)
        .collect();
    assert!(
        !stopped.is_empty() && stopped.iter().all(|v| *v == Ask),
        "{rows:#?}"
    );
}

#[rstest]
#[case::next_window(FullControlMode::NextWindow)]
#[case::held(FullControlMode::Held)]
fn full_control_asks_a_spirit_at_every_window(#[case] mode: FullControlMode) {
    let mut stops = Stops::new(3);
    stops.set_full_control(SPIRIT, mode);
    let rows = walk(table(haunting()), SPIRIT, &stops, &Yields::new(3), &[], 2);
    assert!(
        rows.len() > 10 && rows.iter().all(|row| row.verdict == Ask),
        "{rows:#?}"
    );
}

#[test]
fn a_yield_hurries_a_spirit_past_the_windows_where_it_would_be_asked() {
    let engine = table(haunting()).build().unwrap();
    let mut yields = Yields::new(3);
    yields.start(SPIRIT, YieldUntil::EndOfTurn, engine.state());
    let rows = walk(table(haunting()), SPIRIT, &Stops::new(3), &yields, &[], 1);
    assert!(rows.iter().all(|row| row.verdict == Pass), "{rows:#?}");
}

/// What `Window::is_default` says for a Spirit at each step of a turn, written by hand: with an empty stack the steps
/// of its team's turn that ask it and those of the boss's turn; with an object of the boss on the stack every step.
#[rstest]
#[case::own_turn(true, false, &[Step::Upkeep, Step::Main1, Step::Main2])]
#[case::boss_turn_without_an_attack(false, false, &[Step::BeginCombat])]
#[case::boss_turn_with_an_attack(false, true, &[Step::BeginCombat, Step::DeclareAttackers])]
fn the_spirits_windows_with_an_empty_stack_are_the_haunt_steps(
    #[case] own_turn: bool,
    #[case] attacked: bool,
    #[case] asked: &[Step],
) {
    for step in Step::ALL {
        for (stack, boss) in [(false, false), (true, false), (true, true)] {
            let window = Window {
                step,
                own_turn,
                stack,
                others_object: stack,
                haunt: Some(Haunt {
                    foes_object: boss,
                    foe_turn: !own_turn,
                    attacked,
                }),
            };
            let expected = boss || (!stack && asked.contains(&step));
            assert_eq!(
                window.is_default(),
                expected,
                "{step:?} own {own_turn} stack {stack} boss {boss}"
            );
        }
    }
}

/// The verdict of a Spirit at a window it is asked at, by what it can do, and at one it is not.
#[rstest]
#[case::asked_and_can_act(Step::Main1, true, true, Ask)]
#[case::asked_and_cannot_act_in_its_main_phase(Step::Main1, true, false, Pass)]
#[case::asked_and_cannot_act_in_its_upkeep(Step::Upkeep, true, false, Dwell)]
#[case::not_a_window_and_can_act(Step::End, true, true, Pass)]
#[case::not_a_window_and_cannot_act(Step::End, true, false, Pass)]
fn the_verdict_of_a_spirit_follows_the_table(
    #[case] step: Step,
    #[case] own_turn: bool,
    #[case] can_act: bool,
    #[case] expected: Verdict,
) {
    let window = Window {
        step,
        own_turn,
        stack: false,
        others_object: false,
        haunt: Some(Haunt {
            foes_object: false,
            foe_turn: !own_turn,
            attacked: false,
        }),
    };
    assert_eq!(
        judge(window, can_act, false, FullControlMode::Off, false),
        expected
    );
}

#[test]
fn a_living_hero_keeps_the_default_windows_beside_the_spirit() {
    let state = table(haunting()).build().unwrap();
    let state = state.state();
    assert_eq!(Window::at(state, HERO).haunt, None);
    assert_eq!(Window::at(state, BOSS).haunt, None);
    assert!(Window::at(state, SPIRIT).haunt.is_some());
    let hero = walk(
        table(haunting()),
        HERO,
        &Stops::new(3),
        &Yields::new(3),
        &[],
        2,
    );
    assert_eq!(
        spots(&hero, Ask),
        [
            (1, Step::Main1),
            (1, Step::Main2),
            (2, Step::DeclareAttackers),
            (2, Step::DeclareBlockers),
        ],
        "{hero:#?}"
    );
}
