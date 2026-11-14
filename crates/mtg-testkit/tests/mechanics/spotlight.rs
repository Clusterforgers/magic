//! Goal: the spotlight spreads a boss's pressure in real games: the boss's side's damage, life loss and removal count as
//! attention on the hero hit, a hero past their share of the window is capped, the boss eyes the standing hero with the
//! most presence less attention (minions the next ones), so a "hero in the spotlight" effect goes round the party;
//! the grace keeps every hero above 0 against the boss's side until each had three turns, but not against their own
//! cards; the lull keeps the boss's side from attacking and its schemes waiting until the boss's third turn; a boss
//! takes commander damage as damage and never loses to 21 of it; the bots of the boss's side spare a capped hero.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use std::sync::Arc;

use common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::house::HouseRules;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::events::{Event, NothingToDo};
use mtg_engine::format::Commander;
use mtg_engine::phases::BossPhase;
use mtg_engine::spotlight;
use mtg_engine::turn::Step;
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_engine::{GameState, replay};
use mtg_testkit::brain::seen::Seen;
use mtg_testkit::cards::free_creature;
use mtg_testkit::commander_cards::legend;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn emblem(name: &str, abilities: &[&str]) -> Arc<CardDefinition> {
    let abilities: Vec<Ability> = abilities
        .iter()
        .map(|a| serde_json::from_str(a).unwrap())
        .collect();
    Arc::new(emblem_definition(name, &abilities))
}

/// At the beginning of your upkeep, `effect`.
fn upkeep(effect: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"StepBegins":{{"step":"Upkeep","whose":"You"}}}},"effects":[{effect}]}}}}"#
    )
}

/// A fight: `heroes` heroes with these lives (each a ready 2/2 Bear and a Spirit emblem), then the boss (a ready Bear
/// and a phase that begins at once with `boss_upkeep` each upkeep), then `minions` minions led by it.
fn fight(lives: &[i32], minions: usize, boss_upkeep: &str) -> Scenario {
    let heroes = lives.len();
    let boss = heroes;
    let mut seats = vec![0; heroes];
    seats.extend(std::iter::repeat_n(1, 1 + minions));
    let house = HouseRules {
        teams: Some(Teams {
            mode: TeamMode::Teams,
            seats,
        }),
        ..HouseRules::default()
    };
    let mut scenario = Scenario::new(heroes + 1 + minions).house_rules(house);
    for seat in 0..heroes + 1 + minions {
        scenario =
            scenario.battlefield(seat, [ready(free_creature(&format!("Bear {seat}"), 2, 2))]);
    }
    for (seat, life) in lives.iter().enumerate() {
        scenario = scenario
            .life(seat, *life)
            .spirit(seat, emblem("Haunt", &[]));
    }
    let phase = BossPhase {
        at_life: 20,
        emblem: emblem("Wrath", &[&upkeep(boss_upkeep)]),
    };
    scenario = scenario.life(boss, 20).phases(boss, [phase]);
    for at in 0..minions {
        scenario = scenario.leader(boss + 1 + at, boss);
    }
    scenario
}

/// The `n`th turn (from 1) of the seat at `seat` at a table of `seats`.
fn turn_of(seat: usize, n: u32, seats: usize) -> u32 {
    (n - 1) * u32::try_from(seats).unwrap() + u32::try_from(seat).unwrap() + 1
}

fn life(state: &GameState, seat: usize) -> i32 {
    state.player(pid(seat)).unwrap().life()
}

const DRAIN_SPOTLIT: &str = r#"{"LoseLife":{"who":"Spotlit","amount":3}}"#;

#[rstest]
#[case::three_heroes(3, 0)]
#[case::four_heroes(4, 0)]
#[case::three_heroes_and_a_minion(3, 1)]
fn the_hero_in_the_spotlight_goes_round_the_party(
    #[case] heroes: usize,
    #[case] minions: usize,
) -> Result<(), ScriptError> {
    let scenario = fight(&vec![30; heroes], minions, DRAIN_SPOTLIT);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let seats = heroes + 1 + minions;
    let boss = heroes;
    if minions > 0 {
        let state = driver.state();
        assert_ne!(
            spotlight::focus(state, pid(boss)),
            spotlight::focus(state, pid(boss + 1)),
            "a minion eyes another hero than the boss"
        );
    }
    let mut hit = Vec::new();
    for n in 1..=u32::try_from(heroes).unwrap() {
        // The drain is on the stack in the boss's upkeep; it names the hero eyed now.
        driver.idle_until(turn_of(boss, n, seats), Step::Upkeep)?;
        let eyed = spotlight::focus(driver.state(), pid(boss)).unwrap();
        driver.idle_until(turn_of(boss, n, seats), Step::Draw)?;
        let state = driver.state();
        assert_eq!(life(state, eyed.index()), 27, "the eyed hero is drained");
        assert_eq!(spotlight::received(state, eyed), 3);
        hit.push(eyed);
    }
    hit.sort();
    hit.dedup();
    assert_eq!(hit.len(), heroes, "every hero was in the spotlight once");
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    Ok(())
}

#[rstest]
#[case::one_of_three_hit(3, &[0], true)]
#[case::all_three_hit_evenly(3, &[0, 1, 2], false)]
#[case::two_of_three_hit(3, &[0, 1], true)]
#[case::one_of_two_hit(2, &[0], true)]
fn a_hero_past_their_share_is_capped(
    #[case] heroes: usize,
    #[case] hit: &[usize],
    #[case] capped: bool,
) -> Result<(), ScriptError> {
    let scenario = fight(&vec![30; heroes], 0, r#"{"GainLife":{"amount":1}}"#);
    let driver = Driver::from_scenario(scenario)?;
    let boss = heroes;
    // Damage from the boss's Bear stands in for the boss's attacks.
    let source = driver.id_of(&format!("Bear {boss}"))?;
    let mut state = driver.state().clone();
    for seat in hit {
        mtg_engine::apply_event(
            &mut state,
            &Event::DamageDealt {
                source,
                target: mtg_engine::combat::DamageTarget::Player(pid(*seat)),
                amount: 6,
                deathtouch: false,
                form: mtg_engine::combat::DamageForm::default(),
                toxic: 0,
                unpreventable: false,
            },
        )
        .unwrap();
    }
    assert_eq!(spotlight::capped(&state, pid(hit[0])), capped);
    assert_eq!(spotlight::attention(&state, pid(hit[0])), 6);
    if capped {
        assert_ne!(spotlight::focus(&state, pid(boss)), Some(pid(hit[0])));
        let seen = Seen::new(&state, pid(boss));
        assert!(!mtg_testkit::brain::spotlight::may_aim(&seen, pid(hit[0])));
        assert!(mtg_testkit::brain::spotlight::may_aim(
            &seen,
            pid(heroes - 1)
        ));
    }
    // A hero's own damage to themselves is no attention.
    let own = driver.id_of("Bear 1")?;
    let before = spotlight::attention(&state, pid(1));
    mtg_engine::apply_event(
        &mut state,
        &Event::DamageDealt {
            source: own,
            target: mtg_engine::combat::DamageTarget::Player(pid(1)),
            amount: 4,
            deathtouch: false,
            form: mtg_engine::combat::DamageForm::default(),
            toxic: 0,
            unpreventable: false,
        },
    )
    .unwrap();
    assert_eq!(spotlight::attention(&state, pid(1)), before);
    Ok(())
}

const DRAIN_ALL: &str = r#"{"LoseLife":{"who":"Opponents","amount":5}}"#;
const BURN_ALL: &str = r#"{"DealDamage":{"to":"Opponents","amount":5}}"#;

#[rstest]
#[case::drained_in_the_first_boss_turn(DRAIN_ALL, 1, Some(1))]
#[case::drained_in_the_second(DRAIN_ALL, 2, Some(1))]
#[case::drained_once_every_hero_had_three_turns(DRAIN_ALL, 3, None)]
#[case::burned_in_the_first_boss_turn(BURN_ALL, 1, Some(1))]
#[case::burned_once_every_hero_had_three_turns(BURN_ALL, 3, None)]
fn no_hero_falls_to_the_boss_before_every_hero_had_three_turns(
    #[case] effect: &str,
    #[case] boss_turn: u32,
    #[case] left: Option<i32>,
) -> Result<(), ScriptError> {
    let scenario = fight(&[3, 40], 0, effect);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(turn_of(2, boss_turn, 3), Step::Draw)?;
    let state = driver.state();
    let hero = state.player(pid(0)).unwrap();
    match left {
        Some(life) => {
            assert_eq!(hero.life(), life);
            assert!(!hero.is_spirit());
        }
        None => assert!(hero.is_spirit(), "the grace is over"),
    }
    assert_eq!(life(state, 1), 40 - 5 * i32::try_from(boss_turn).unwrap());
    Ok(())
}

#[test]
fn a_hero_can_fall_to_their_own_cards_in_the_grace() -> Result<(), ScriptError> {
    let own = emblem(
        "Bargain",
        &[&upkeep(r#"{"LoseLife":{"who":"You","amount":5}}"#)],
    );
    let scenario = fight(&[3, 40], 0, r#"{"GainLife":{"amount":1}}"#).emblems(0, [own]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(turn_of(0, 2, 3), Step::Draw)?;
    assert!(driver.state().player(pid(0)).unwrap().is_spirit());
    Ok(())
}

#[rstest]
#[case::first_boss_turn(1, false)]
#[case::second_boss_turn(2, false)]
#[case::third_boss_turn(3, true)]
fn the_lull_holds_the_boss_sides_attacks_and_schemes_until_its_third_turn(
    #[case] boss_turn: u32,
    #[case] free: bool,
) -> Result<(), ScriptError> {
    let scenario =
        fight(&[40, 40], 1, r#"{"GainLife":{"amount":1}}"#).schemes(2, [emblem("Ambush", &[])]);
    let mut driver = Driver::from_scenario(scenario)?;
    let seats = 4;
    driver.idle_until(turn_of(2, boss_turn, seats), Step::Main1)?;
    assert_eq!(
        driver
            .state()
            .player(pid(2))
            .unwrap()
            .schemes()
            .unwrap()
            .0
            .is_some(),
        free,
        "a scheme in motion"
    );
    let mark = driver.engine().events().len();
    // Through the boss's and its minion's turns.
    driver.idle_until(turn_of(0, boss_turn + 1, seats), Step::Main1)?;
    let held = |seat: usize| {
        driver.engine().events()[mark..].iter().any(|event| {
            matches!(event, Event::NothingToDo { player, kind: NothingToDo::Attack } if *player == pid(seat))
        })
    };
    assert_eq!(held(2), !free, "the boss");
    assert_eq!(held(3), !free, "the minion");
    Ok(())
}

#[rstest]
#[case::the_boss_stays(true, 21, true)]
#[case::a_plain_seat_falls(false, 21, false)]
fn a_boss_takes_commander_damage_but_never_loses_to_it(
    #[case] boss: bool,
    #[case] power: i32,
    #[case] stays: bool,
) -> Result<(), ScriptError> {
    let house = HouseRules {
        teams: Some(Teams {
            mode: TeamMode::Teams,
            seats: vec![0, 1],
        }),
        ..HouseRules::default()
    };
    let mut scenario = Scenario::new(2)
        .format(Arc::new(Commander))
        .house_rules(house)
        .commanders(0, [legend("Cmd", 0, &[], power, power)])
        .spirit(0, emblem("Haunt", &[]))
        .life(1, 40);
    if boss {
        scenario = scenario.phases(
            1,
            [BossPhase {
                at_life: 1,
                emblem: emblem("Last", &[]),
            }],
        );
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Cmd")?;
    for _ in 0..2 {
        driver.pass()?;
    }
    driver.idle_until(3, Step::DeclareAttackers)?;
    driver.attack(&[("Cmd", 1)])?;
    driver.idle_until(3, Step::EndCombat).ok();
    let foe = driver.state().player(pid(1)).unwrap();
    assert_eq!(foe.life(), 40 - power);
    assert_eq!(foe.is_in_game(), stays);
    Ok(())
}
