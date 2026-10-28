//! Goal: a crowd of creatures that an attack tax holds back (Propaganda) still attacks where the attack is free, within the mana its player surely has, at two to four seats; a table whose tokens double every round stops growing at the crowded-table limit instead of running away; and the watchdog names the seed, the turn and the last answers of a game that passes its caps.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::panic::{AssertUnwindSafe, catch_unwind};

use mtg_core::ids::ObjectId;
use mtg_core::mana::ManaCost;
use mtg_engine::combat::AttackTarget;
use mtg_engine::decision::{
    Answer, AttackOptions, AttackerOption, Decision, DecisionKind, PriorityAction,
};
use mtg_engine::turn::Step;
use mtg_engine::{Engine, GameSetup};
use mtg_testkit::cards::{forest, free_creature, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::watchdog::{MAX_GAME_ANSWERS, MAX_PERMANENTS, MAX_TURN_ANSWERS};
use mtg_testkit::{PlayOptions, RandomBot, Scenario, Watchdog, play_game_with, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::pid;

/// Seat 0 may attack with `size` creatures, each at every other seat; seat 1 taxes its attackers {2} each (Propaganda).
fn crowd(seats: usize, size: u64, mana: u32) -> Decision {
    let defenders: Vec<AttackTarget> = (1..seats)
        .map(|seat| AttackTarget::Player(pid(seat)))
        .collect();
    let tax = vec![(defenders[0], ManaCost::parse("{2}").unwrap())];
    let attackers = (0..size)
        .map(|n| AttackerOption {
            object: ObjectId(1_000 + n),
            defenders: defenders.clone(),
            tax: tax.clone(),
            life: Vec::new(),
        })
        .collect();
    let options = AttackOptions {
        attackers,
        required: Vec::new(),
        mana,
    };
    Decision {
        player: pid(0),
        kind: DecisionKind::DeclareAttackers(options),
    }
}

#[rstest]
fn a_crowd_held_back_by_a_tax_attacks_where_the_attack_is_free(
    #[values(2, 3, 4)] seats: usize,
    #[values(400, 3_000)] size: u64,
    #[values(0, 4, 40)] mana: u32,
    #[values(1, 2)] seed: u64,
) {
    let decision = crowd(seats, size, mana);
    let Answer::DeclareAttackers(declared) = RandomBot::new(seed, 0).choose(&decision) else {
        panic!("not an attack");
    };
    let DecisionKind::DeclareAttackers(options) = &decision.kind else {
        panic!("not an attack");
    };
    assert!(
        options.cost_of(&declared).mana_value() <= mana,
        "{} attackers cost more than {mana}",
        declared.len()
    );
    let free = declared
        .iter()
        .filter(|(_, target)| *target != AttackTarget::Player(pid(1)))
        .count();
    if seats == 2 {
        assert_eq!(free, 0, "a seat that taxes every attack is not free");
        assert!(declared.len() <= usize::try_from(mana / 2).unwrap());
    } else {
        // About a quarter of the crowd (at three seats) to a third (at four) joins and picks a free seat.
        let size = usize::try_from(size).unwrap();
        assert!(free >= size / 8, "only {free} of {size} attack for nothing");
    }
}

/// Seat 0 holds Rhys the Redeemed (a token each round, then every token doubled) with the lands for his abilities; every
/// other seat holds a Propaganda, so the crowd cannot simply attack its way out.
fn doubling_table(seats: usize) -> Scenario {
    let mut mine = vec![ready(card("Rhys the Redeemed").unwrap())];
    mine.extend(lands(8, forest));
    mine.extend(lands(8, plains));
    let mut scenario = Scenario::new(seats).battlefield(0, mine);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, vec![ready(card("Propaganda").unwrap())]);
    }
    scenario
}

#[rstest]
fn a_table_that_doubles_every_round_stops_growing_at_the_crowded_limit(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] seed: u64,
) {
    let setup = doubling_table(seats).setup().unwrap();
    let outcome = play_game_with(setup, seed, PlayOptions::default());
    assert!(outcome.is_ok(), "{:?}", outcome.err().map(|f| f.kind));
}

fn pass() -> Answer {
    Answer::Priority(PriorityAction::Pass)
}

/// The panic message of `run`.
fn message(run: impl FnOnce()) -> String {
    let payload = catch_unwind(AssertUnwindSafe(run)).expect_err("the watchdog did not stop");
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_default()
}

/// What the watchdog says once `answers` answers were given, `per_turn` of them in each turn from turn 1 on.
fn report_after(answers: usize, per_turn: usize) -> String {
    let turns = u32::try_from(answers / per_turn + 1).unwrap();
    let engines: Vec<Engine> = (1..=turns)
        .map(|turn| Scenario::new(2).at(turn, 0, Step::Upkeep).build().unwrap())
        .collect();
    message(|| {
        let mut watchdog = Watchdog::scripted(41);
        let mut left = answers;
        for engine in &engines {
            for _ in 0..per_turn.min(left) {
                watchdog.answered(engine.state(), pid(1), "Priority", &pass());
            }
            left = left.saturating_sub(per_turn);
        }
    })
}

#[rstest]
#[case::the_turn(MAX_TURN_ANSWERS + 1, MAX_TURN_ANSWERS + 1, 1, "answers in this turn")]
#[case::the_game(
    MAX_GAME_ANSWERS + 1,
    MAX_TURN_ANSWERS - 1,
    MAX_GAME_ANSWERS / (MAX_TURN_ANSWERS - 1) + 1,
    "answers in the game"
)]
fn the_watchdog_names_the_seed_the_turn_and_the_last_answers(
    #[case] answers: usize,
    #[case] per_turn: usize,
    #[case] last_turn: usize,
    #[case] over: &str,
) {
    let report = report_after(answers, per_turn);
    assert!(report.contains("seed 41"), "{report}");
    assert!(report.contains(&format!("turn {last_turn}")), "{report}");
    assert!(report.contains(over), "{report}");
    assert_eq!(report.matches("p1 Priority => Priority(Pass)").count(), 20);
    assert!(report.contains(&format!("#{answers} ")), "{report}");
}

/// A table of two seats where seat 0 holds one more crowd creature than the watchdog allows.
fn overfull_table() -> GameSetup {
    let creatures = (0..=MAX_PERMANENTS).map(|_| ready(free_creature("Crowd", 1, 1)));
    Scenario::new(2)
        .battlefield(0, creatures.collect::<Vec<_>>())
        .setup()
        .unwrap()
}

#[test]
fn the_watchdog_stops_a_table_that_grows_without_end_and_names_the_table_and_the_decks() {
    let setup = overfull_table();
    let engine = Engine::new(setup.clone()).unwrap();
    let report = message(|| {
        Watchdog::for_game(7, &setup).answered(engine.state(), pid(0), "Priority", &pass());
    });
    assert!(report.contains("permanents on the table"), "{report}");
    assert!(report.contains("seed 7"), "{report}");
    assert!(
        report.contains(&format!("Crowd x{}", MAX_PERMANENTS + 1)),
        "{report}"
    );
    assert!(report.contains("seat 0: Forest"), "{report}");
    assert!(report.contains("seat 1: Forest"), "{report}");
}

#[test]
fn a_scripted_game_may_fill_the_table_as_it_likes() {
    let engine = Engine::new(overfull_table()).unwrap();
    Watchdog::scripted(7).answered(engine.state(), pid(0), "Priority", &pass());
}
