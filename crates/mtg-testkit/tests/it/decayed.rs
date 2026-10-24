//! Goal: decayed plays as CR 702.147 says at tables of two, three and four seats: a creature with it is offered no block, whether the keyword is printed on it or a token or a card gets it later, it is sacrificed at the end of the combat it attacked in (blocked or not, once, by its controller) and a creature that does not attack is left alone, a creature without it that attacks with it is not touched, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{free_creature, keyworded};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, names, pid};

fn zombie() -> Arc<CardDefinition> {
    keyworded("Zombie", 2, 2, &[Keyword::Decayed])
}

/// Seat 0 is about to attack with `attackers`; seat 1 holds `blockers`; the other seats hold a bystander of power 1.
fn table(
    seats: usize,
    attackers: Vec<Arc<CardDefinition>>,
    blockers: Vec<Arc<CardDefinition>>,
) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, attackers.into_iter().map(ready))
        .battlefield(1, blockers.into_iter().map(ready));
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..8).map(|_| free_creature("Filler", 1, 1)));
    }
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Bystander", 1, 1))]);
    }
    scenario
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// The blockers the pending decision offers, sorted.
fn offered(driver: &Driver) -> Vec<String> {
    let DecisionKind::DeclareBlockers(options) = &driver.decision().unwrap().kind else {
        panic!("blockers are asked");
    };
    let mut found: Vec<String> = options
        .blockers
        .iter()
        .map(|blocker| name_of(driver.state(), blocker.object))
        .collect();
    found.sort();
    found
}

#[rstest]
fn a_creature_with_decayed_is_offered_no_block(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        vec![free_creature("Raider", 3, 3)],
        vec![zombie(), free_creature("Bear", 2, 2)],
    ))?;
    driver.attack(&[("Raider", 1)])?;
    assert_eq!(offered(&driver), ["Bear"]);
    Ok(())
}

#[rstest]
fn a_lone_creature_with_decayed_leaves_the_block_unasked(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        vec![free_creature("Raider", 3, 3)],
        vec![zombie()],
    ))?;
    driver.attack(&[("Raider", 1)])?;
    // No creature can block: the declaration is not asked for.
    assert!(!matches!(
        driver.decision()?.kind,
        DecisionKind::DeclareBlockers(_)
    ));
    Ok(())
}

/// The attackers of seat 0 attack seat 1 and combat is played out with the blocks `blocks` (blocker, attacker): who
/// is in the graveyard of seat 0 after combat, and how much life seat 1 lost.
#[rstest]
#[case::unblocked(&["Zombie"], &[], &["Zombie"], 2)]
#[case::blocked_by_a_wall(&["Zombie"], &[("Wall", "Zombie")], &["Zombie"], 0)]
#[case::a_plain_creature_stays(&["Raider"], &[], &[], 3)]
#[case::both_attack_only_decayed_goes(&["Zombie", "Raider"], &[], &["Zombie"], 5)]
fn a_creature_with_decayed_is_sacrificed_at_the_end_of_combat_it_attacked_in(
    #[values(2, 3, 4)] seats: usize,
    #[case] attackers: &[&str],
    #[case] blocks: &[(&str, &str)],
    #[case] sacrificed: &[&str],
    #[case] life_lost: i32,
) -> Result<(), ScriptError> {
    let pool = |name: &str| match name {
        "Zombie" => zombie(),
        "Raider" => free_creature("Raider", 3, 3),
        _ => panic!("{name}"),
    };
    let game = || {
        table(
            seats,
            attackers.iter().map(|name| pool(name)).collect(),
            vec![free_creature("Wall", 0, 5)],
        )
    };
    let setup = game().setup().unwrap();
    let mut driver = Driver::from_scenario(game())?;
    let attacks: Vec<(&str, usize)> = attackers.iter().map(|name| (*name, 1)).collect();
    driver.attack(&attacks)?;
    if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
        driver.block(blocks)?;
    }
    driver.pass_until(Step::Main2)?;
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), sacrificed);
    let battlefield = names(driver.state(), Zone::Battlefield);
    for name in attackers.iter().filter(|name| !sacrificed.contains(name)) {
        assert!(battlefield.contains(&(*name).to_owned()), "{name}");
    }
    assert_eq!(
        20 - driver.state().player(pid(1)).unwrap().life(),
        life_lost
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_creature_with_decayed_that_does_not_attack_is_left_alone(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(
        seats,
        vec![zombie(), free_creature("Raider", 3, 3)],
        vec![],
    ))?;
    driver.attack(&[("Raider", 1)])?;
    driver.pass_until(Step::Main2)?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Zombie".to_owned()));
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).is_empty());
    Ok(())
}
