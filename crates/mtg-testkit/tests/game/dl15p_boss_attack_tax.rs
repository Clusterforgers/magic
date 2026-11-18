//! Goal: a boss bot facing a hero's attack tax (Archangel of Tithes, Ghostly Prison, Propaganda, Windborn Muse) never
//! declares an attack it cannot pay (CR 508.1h): every answer it gives the table is accepted, whatever mana it holds,
//! at every level, alone or with a minion sharing its turn, so its turn goes on instead of looping on a refused or
//! backed-out declaration.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::house::HouseRules;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::{Engine, PresetPermanent};
use mtg_testkit::brain::{Level, brain};
use mtg_testkit::cards::{free_creature, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Scenario, ready};
use rstest::rstest;

use crate::common::pid;

/// The turn of the table.
const TURN: u32 = 9;

/// Answers the boss's turn may take: far more than its attack needs.
const ANSWERS: usize = 400;

/// The boss's seat.
const BOSS: usize = 1;

/// The seat of the boss's minion.
const MINION: usize = 2;

/// Three 6/6s and `lands` untapped Swamps of a boss-side seat.
fn brutes(seat: usize, lands: usize) -> Vec<PresetPermanent> {
    let mut board: Vec<_> = (0..3)
        .map(|at| ready(free_creature(&format!("Brute {seat}.{at}"), 6, 6)))
        .collect();
    board.extend((0..lands).map(|_| ready(swamp())));
    board
}

/// The boss at its declare-attackers step with its brutes, and with a minion sharing its turn as in a Delves fight
/// when `minion`; the hero holds `tax`.
fn table(tax: &str, lands: usize, minion: bool) -> Engine {
    let mut scenario = if minion {
        Scenario::new(3).house_rules(HouseRules {
            teams: Some(Teams {
                mode: TeamMode::SharedTurns,
                seats: vec![0, 1, 1],
            }),
            ..HouseRules::default()
        })
    } else {
        Scenario::new(2)
    };
    scenario = scenario
        .at(TURN, BOSS, Step::DeclareAttackers)
        .battlefield(BOSS, brutes(BOSS, lands))
        .battlefield(0, vec![ready(card(tax).unwrap())])
        .life(0, 100);
    if minion {
        scenario = scenario.battlefield(MINION, brutes(MINION, lands));
    }
    scenario.build().unwrap()
}

#[rstest]
fn the_boss_never_declares_an_attack_it_cannot_pay(
    #[values("Archangel of Tithes", "Ghostly Prison", "Propaganda", "Windborn Muse")] tax: &str,
    #[values(0, 1, 4)] lands: usize,
    #[values(Level::Easy, Level::Normal, Level::Hard)] level: Level,
    #[values(false, true)] minion: bool,
) {
    let mut engine = table(tax, lands, minion);
    let mut boss = brain(level, 7, pid(BOSS), false);
    let mut second = brain(level, 7, pid(MINION), false);
    let mut hero = brain(Level::Normal, 7, pid(0), false);
    let mut attacked = false;
    for _ in 0..ANSWERS {
        if engine.state().turn().number() != TURN
            || engine.state().turn().step() > Step::DeclareBlockers
        {
            break;
        }
        let Some(decision) = engine.decision().cloned() else {
            break;
        };
        let answer = if decision.player == pid(BOSS) {
            boss.choose(&decision, Some(&engine))
        } else if decision.player == pid(MINION) {
            second.choose(&decision, Some(&engine))
        } else {
            hero.choose(&decision, Some(&engine))
        };
        if let Err(error) = engine.apply(answer.clone()) {
            panic!(
                "{tax}, {lands} lands: the table refused {answer:?} to {}: {error}",
                decision.kind.name()
            );
        }
        attacked |= matches!(decision.kind, DecisionKind::DeclareAttackers(_));
    }
    assert!(attacked, "the boss declared its attack");
    assert!(
        engine.state().turn().number() > TURN
            || engine.state().turn().step() > Step::DeclareAttackers,
        "{tax}, {lands} lands: the boss's turn is stuck at {:?}",
        engine.state().turn().step()
    );
}
