//! Goal: a Hard or Very Hard bot with an opponent in reach makes a play that changes nothing (Crypt Rats at X = 0 with every land tapped) no more often than the turn's repeat cap and then goes on to the attack that wins, the endgame search skips plays held for the turn, and the cap still lets the bot repeat plays that change the game (a firebreathing pump with real mana for lethal).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::Engine;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::turn::Step;
use mtg_testkit::bot::apply_answer;
use mtg_testkit::brain::endgame::forced_win;
use mtg_testkit::brain::{Level, brain};
use mtg_testkit::cards::{free_creature, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Scenario, ready, tapped};
use rstest::rstest;

use crate::common::pid;

/// The turn of the table.
const TURN: u32 = 13;

/// Answers a test plays of one turn: far more than the plays and the attack need, far fewer than the watchdog's cap.
const ANSWERS: usize = 600;

/// Most times a brain may make one play in a turn (`REPEATS` of the brain).
const REPEATS: usize = 3;

/// The boss's seat.
const BOSS: usize = 1;

/// Seat 1 holds Crypt Rats with every land tapped (so X is 0 and the ability changes nothing) and seven 1/1 bodies;
/// seat 0 holds a 6/6 blocker at `life`.
fn crypt_table(life: i32) -> Engine {
    let rats = card("Crypt Rats").unwrap();
    let mut boss = vec![ready(rats)];
    boss.extend((0..4).map(|_| tapped(swamp())));
    boss.extend((0..7).map(|at| ready(free_creature(&format!("Rat {at}"), 1, 1))));
    Scenario::new(2)
        .at(TURN, BOSS, Step::Main1)
        .battlefield(BOSS, boss)
        .battlefield(0, vec![ready(free_creature("Tyrant", 6, 6))])
        .life(0, life)
        .build()
        .unwrap()
}

/// A creature costing nothing, 1/1, with a `{R}` pump of +1/+0 until end of turn.
fn firebreather() -> Arc<CardDefinition> {
    let pump: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":"{R}","effects":[{"Continuous":{"affected":"This","mods":[{"ModifyPT":{"power":1,"toughness":0}}],"duration":"UntilEndOfTurn"}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Firebreather")
            .with_type(CardType::Creature)
            .with_mana_cost(ManaCost::parse("{0}").unwrap())
            .with_power_toughness(1, 1)
            .with_rules(pump),
    )
}

/// Plays the turn of `table` with a brain of `level` for the boss and a Normal brain for the other seat; returns how
/// many activations the boss made and the table after.
fn play_turn(mut table: Engine, level: Level) -> (usize, Engine) {
    let mut boss = brain(level, 5, pid(BOSS), false);
    let mut hero = brain(Level::Normal, 5, pid(0), false);
    let mut activations = 0;
    for _ in 0..ANSWERS {
        let Some(decision) = table.decision().cloned() else {
            break;
        };
        if table.state().turn().number() != TURN {
            break;
        }
        let answer = if decision.player == pid(BOSS) {
            boss.choose(&decision, Some(&table))
        } else {
            hero.choose(&decision, Some(&table))
        };
        if decision.player == pid(BOSS)
            && matches!(answer, Answer::Priority(PriorityAction::Activate(_)))
        {
            activations += 1;
        }
        apply_answer(&mut table, &decision, answer).unwrap();
    }
    (activations, table)
}

#[rstest]
fn a_play_that_changes_nothing_is_made_at_most_the_repeat_cap_and_the_attack_follows(
    #[values(Level::Hard, Level::VeryHard)] level: Level,
    #[values(4, 40)] life: i32,
) {
    let (activations, table) = play_turn(crypt_table(life), level);
    assert!(activations <= REPEATS, "{activations} activations");
    if life == 4 {
        // Six attackers past one blocker are lethal: the turn ends in the win, not in the answer limit.
        assert!(table.result().is_some(), "the game goes on");
    } else {
        assert!(table.state().turn().number() > TURN, "the turn goes on");
    }
}

#[test]
fn the_endgame_search_skips_plays_held_for_the_turn() {
    let table = crypt_table(4);
    let decision = table.decision().cloned().unwrap();
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("not priority");
    };
    let play = PriorityAction::Activate(options.abilities[0]);
    // The activation "wins" because the quick policies attack for lethal whatever it did, so it is the first answer.
    let first = forced_win(&table, pid(BOSS), &decision, 5, None, &[]);
    assert_eq!(first, Some(Answer::Priority(play.clone())));
    let held = forced_win(&table, pid(BOSS), &decision, 5, None, &[play]);
    assert_ne!(held, first);
}

#[rstest]
fn repeated_pumps_with_real_mana_for_lethal_still_happen(
    #[values(Level::Hard, Level::VeryHard)] level: Level,
) {
    let mut boss = vec![ready(firebreather())];
    boss.extend((0..3).map(|_| ready(mountain())));
    let table = Scenario::new(2)
        .at(TURN, BOSS, Step::Main1)
        .battlefield(BOSS, boss)
        .life(0, 4)
        .build()
        .unwrap();
    let (activations, table) = play_turn(table, level);
    assert!(table.result().is_some(), "the game goes on");
    assert_eq!(activations, 3, "three pumps make the lethal 4");
}
