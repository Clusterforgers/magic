//! Goal: at a table of several opponents the bots aim by threat, not low life: attacks and burn go at the big board
//! with the high life over the empty board with the low life, a kill that gets past every blocker is still taken,
//! damage to the seat draws retaliation, an opponent hit hard lately is spared a little, and teammates are never aimed at.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::keyword::Keyword;
use mtg_core::house::HouseRules;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::choose::Target;
use mtg_engine::combat::{DamageForm, DamageTarget};
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::events::Event;
use mtg_engine::turn::Step;
use mtg_engine::{Engine, GameState, PresetPermanent};
use mtg_testkit::brain::combat::focus;
use mtg_testkit::brain::seen::Body;
use mtg_testkit::brain::targets::rank_targets;
use mtg_testkit::brain::{Seen, threat};
use mtg_testkit::cards::{free_creature, keyworded, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Scenario, ready};
use rstest::rstest;

use crate::common::pid;

/// `count` creatures of `power`/`toughness`, flying when `flying`, named for `side`.
fn bodies(
    side: &str,
    count: usize,
    power: i32,
    toughness: i32,
    flying: bool,
) -> Vec<PresetPermanent> {
    let keys: &[Keyword] = if flying { &[Keyword::Flying] } else { &[] };
    (0..count)
        .map(|at| ready(keyworded(&format!("{side} {at}"), power, toughness, keys)))
        .collect()
}

/// Four seats free-for-all (or two teams of two when `teams`), seat 0 about to attack with two 2/2 flyers. Seat 1 has
/// `big` ground 3/3s and `high` life, seat 2 `low` life and `guard` 1/1 flyers, seat 3 20 life and nothing.
fn table(big: usize, high: i32, low: i32, guard: usize, teams: bool) -> Engine {
    let mut scenario = Scenario::new(4);
    if teams {
        scenario = scenario.house_rules(HouseRules {
            teams: Some(Teams::even(TeamMode::Teams, 2, 2)),
            ..HouseRules::default()
        });
    }
    scenario
        .at(3, 0, Step::DeclareAttackers)
        .battlefield(0, bodies("Mine", 2, 2, 2, true))
        .battlefield(1, bodies("Big", big, 3, 3, false))
        .battlefield(2, bodies("Guard", guard, 1, 1, true))
        .life(1, high)
        .life(2, low)
        .build()
        .unwrap()
}

/// The opponent seat 0's attacks focus on, among every other seat.
fn focused(state: &GameState) -> Option<PlayerId> {
    let seen = Seen::new(state, pid(0));
    let bodies: BTreeMap<ObjectId, Body> = seen.bodies().into_iter().map(|b| (b.id, b)).collect();
    focus(&seen, &bodies, &[pid(1), pid(2), pid(3)])
}

/// `state` after the first creature of `from` dealt `amount` to `to`.
fn hurt(state: &GameState, from: usize, to: usize, amount: u32) -> GameState {
    let source = state
        .zone(mtg_engine::zones::Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| {
            state
                .object(*id)
                .is_some_and(|o| o.controller() == pid(from))
        })
        .unwrap();
    let mut state = state.clone();
    mtg_engine::apply_event(
        &mut state,
        &Event::DamageDealt {
            source,
            target: DamageTarget::Player(pid(to)),
            amount,
            deathtouch: false,
            form: DamageForm::default(),
            toxic: 0,
            unpreventable: false,
        },
    )
    .unwrap();
    state
}

#[rstest]
#[case::big_board_high_life_over_empty_low_life(2, 30, 6, 0, 1)]
#[case::a_kill_past_every_blocker_is_taken(2, 30, 4, 0, 2)]
#[case::a_kill_a_blocker_stops_is_no_kill(2, 30, 4, 1, 1)]
#[case::even_boards_the_higher_life_threatens_more(0, 30, 6, 0, 1)]
fn attacks_aim_by_threat_not_low_life(
    #[case] big: usize,
    #[case] high: i32,
    #[case] low: i32,
    #[case] guard: usize,
    #[case] expected: usize,
) {
    let engine = table(big, high, low, guard, false);
    assert_eq!(focused(engine.state()), Some(pid(expected)));
}

#[rstest]
#[case::retaliation_against_who_hit_the_seat(3, 0, 12, 3)]
#[case::the_most_hit_opponent_is_spared(3, 1, 12, 2)]
fn recent_damage_leans_the_focus(
    #[case] from: usize,
    #[case] to: usize,
    #[case] amount: u32,
    #[case] expected: usize,
) {
    // Seats 1 and 2 even at 20 life with one 3/3 each, seat 3 a 3/3 too: recent damage decides.
    let engine = Scenario::new(4)
        .at(3, 0, Step::DeclareAttackers)
        .battlefield(0, bodies("Mine", 2, 2, 2, true))
        .battlefield(1, bodies("One", 1, 3, 3, false))
        .battlefield(2, bodies("Two", 1, 3, 3, false))
        .battlefield(3, bodies("Three", 1, 3, 3, false))
        .build()
        .unwrap();
    let state = hurt(engine.state(), from, to, amount);
    assert_eq!(
        mtg_engine::rivalry::dealt(&state, pid(from), pid(to)),
        amount
    );
    let seen = Seen::new(&state, pid(0));
    if to == 0 {
        assert!(threat::rivalry_lean(&seen, pid(from)) > 0.0);
    } else {
        assert!(threat::rivalry_lean(&seen, pid(to)) < 0.0);
    }
    assert_eq!(focused(&state), Some(pid(expected)));
}

#[test]
fn teammates_are_never_focused() {
    // Seats 0 and 2 play together; the teammate has the lowest life and seat 1 the biggest board.
    let engine = table(2, 30, 1, 0, true);
    assert_eq!(focused(engine.state()), Some(pid(1)));
}

/// The targets seat 0's brain ranks for a Lightning Bolt cast at a four-seat table: seat 1 at 33 life with two 4/4s the
/// Bolt cannot kill, seat 2 at `low` life with nothing (a teammate when `teams`), seat 3 at 20 with nothing.
fn bolt_targets(low: i32, teams: bool) -> Vec<Target> {
    let mut scenario = Scenario::new(4);
    if teams {
        scenario = scenario.house_rules(HouseRules {
            teams: Some(Teams::even(TeamMode::Teams, 2, 2)),
            ..HouseRules::default()
        });
    }
    let mut engine = scenario
        .at(3, 0, Step::Main1)
        .hand(0, [card("Lightning Bolt").unwrap()])
        .battlefield(0, vec![ready(mountain())])
        .battlefield(
            1,
            vec![
                ready(free_creature("Big One", 4, 4)),
                ready(free_creature("Big Two", 4, 4)),
            ],
        )
        .life(1, 33)
        .life(2, low)
        .build()
        .unwrap();
    let DecisionKind::Priority(options) = &engine.decision().unwrap().kind else {
        panic!("priority expected");
    };
    let bolt = options.castable[0];
    engine
        .apply(Answer::Priority(PriorityAction::CastSpell(bolt)))
        .unwrap();
    let DecisionKind::ChooseTargets(options) = &engine.decision().unwrap().kind else {
        panic!("targets expected");
    };
    rank_targets(&Seen::new(engine.state(), pid(0)), options)
}

#[rstest]
#[case::free_for_all(false)]
#[case::teams(true)]
fn burn_at_players_goes_by_threat_never_at_teammates(#[case] teams: bool) {
    let ranked = bolt_targets(3, teams);
    let players: Vec<Target> = ranked
        .iter()
        .copied()
        .filter(|t| t.player().is_some())
        .collect();
    assert_eq!(players.first(), Some(&Target::Player(pid(1))), "{ranked:?}");
    if teams {
        assert!(!ranked.contains(&Target::Player(pid(2))));
        assert!(!ranked.contains(&Target::Player(pid(0))));
    }
}
