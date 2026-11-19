//! Goal: the exceptions to who receives and who assigns combat damage (CR 510.1) play as their cards print them at two
//! to four seats: trample over planeswalkers sends the damage beyond lethal to the planeswalker on to its controller
//! (Thrasta, a plain trampler stops at the planeswalker), Butcher Orgg divides among the defending player and any of
//! their creatures, Cunning Giant unblocked gives all its damage to one of their creatures or none, Defensive Formation
//! gives the division to the defending player attacked, and Ogre Enforcer dies to lethal damage only from one source;
//! the extra turn of Alchemist's Gambit lets no damage be prevented and its controller loses at its end step; each log
//! replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Decision, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{burn, free_creature, mountain, walker};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The log of the driver replays to its state.
fn replays(scenario: &Scenario, driver: &Driver) {
    let setup = scenario.clone().setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

fn seat(index: usize) -> PlayerId {
    PlayerId::from_index(index)
}

fn life(driver: &Driver, index: usize) -> i32 {
    driver.state().player(seat(index)).unwrap().life()
}

fn alive(driver: &Driver, name: &str) -> bool {
    driver.id_of(name).is_ok_and(|id| {
        driver
            .state()
            .object(id)
            .is_some_and(|o| o.zone() == mtg_engine::zones::Zone::Battlefield)
    })
}

fn trampler(name: &str, power: i32) -> Arc<CardDefinition> {
    let plain = free_creature(name, power, power);
    Arc::new(
        (*plain)
            .clone()
            .with_rules(Ability::Keyword(Keyword::Trample)),
    )
}

/// Passes through combat after attackers are declared, the defending player blocking as `blocks` says, up to the first
/// damage division asked (returned) or the end of combat.
fn to_division(
    driver: &mut Driver,
    blocks: &[(&str, &str)],
) -> Result<Option<Decision>, ScriptError> {
    let mut blocked = false;
    loop {
        if driver.state().turn().step() == Step::EndCombat {
            return Ok(None);
        }
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::DeclareBlockers(_) if !blocked => {
                blocked = true;
                driver.block(blocks)?;
            }
            DecisionKind::AssignDamage(_) => return Ok(Some(decision)),
            _ => {
                driver.answer_passively()?;
            }
        }
    }
}

#[rstest]
fn r109_trample_over_planeswalkers_reaches_the_controller(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] blocked: bool,
    #[values(false, true)] over: bool,
) -> Result<(), ScriptError> {
    let attacker = if over {
        card("Thrasta, Tempest's Roar").unwrap()
    } else {
        trampler("Thrasta, Tempest's Roar", 7)
    };
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(attacker)])
        .battlefield(
            1,
            [
                ready(walker("Walker", 3)),
                ready(free_creature("Bear", 2, 2)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.attack_planeswalkers(&[("Thrasta, Tempest's Roar", "Walker")])?;
    let blocks: &[(&str, &str)] = if blocked {
        &[("Bear", "Thrasta, Tempest's Roar")]
    } else {
        &[]
    };
    let division = to_division(&mut driver, blocks)?;
    match (blocked, over) {
        (false, false) => assert!(division.is_none(), "a plain trampler has nothing to divide"),
        (true, false) => {
            driver.assign_damage(&[("Bear", 2)])?;
        }
        (false, true) => {
            assert_eq!(division.unwrap().player, seat(0));
            // Less than lethal to the planeswalker keeps the rest from its controller.
            assert!(driver.assign_damage(&[("Walker", 2)]).is_err());
            driver.assign_damage(&[("Walker", 3)])?;
        }
        (true, true) => {
            assert!(driver.assign_damage(&[("Bear", 1), ("Walker", 3)]).is_err());
            driver.assign_damage(&[("Bear", 2), ("Walker", 3)])?;
        }
    }
    driver.pass_until(Step::EndCombat)?;
    let lost = 20 - life(&driver, 1);
    let expected = match (blocked, over) {
        (_, false) => 0,
        (false, true) => 4,
        (true, true) => 2,
    };
    assert_eq!(
        lost, expected,
        "blocked {blocked}, over planeswalkers {over}"
    );
    assert!(!alive(&driver, "Walker"));
    replays(&scenario, &driver);
    Ok(())
}

#[rstest]
fn r109_butcher_orgg_divides_among_the_defender_and_their_creatures(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] blocked: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(card("Butcher Orgg").unwrap())])
        .battlefield(
            1,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Cub", 1, 1)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.attack(&[("Butcher Orgg", 1)])?;
    let blocks: &[(&str, &str)] = if blocked {
        &[("Bear", "Butcher Orgg")]
    } else {
        &[]
    };
    let division = to_division(&mut driver, blocks)?.expect("a division");
    assert_eq!(division.player, seat(0));
    // The blocker is spared; the creature that did not block and the player share the damage.
    driver.assign_damage(&[("Bear", 0), ("Cub", 1)])?;
    driver.pass_until(Step::EndCombat)?;
    assert_eq!(20 - life(&driver, 1), 5);
    assert!(alive(&driver, "Bear"));
    assert!(!alive(&driver, "Cub"));
    replays(&scenario, &driver);
    Ok(())
}

#[rstest]
fn r109_a_plain_attacker_cannot_reach_a_creature_that_did_not_block(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(free_creature("Brute", 6, 6))])
        .battlefield(
            1,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Cub", 1, 1)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.attack(&[("Brute", 1)])?;
    assert!(to_division(&mut driver, &[])?.is_none());
    assert_eq!(20 - life(&driver, 1), 6);
    assert!(alive(&driver, "Cub"));
    Ok(())
}

#[rstest]
fn r109_cunning_giant_gives_all_or_nothing_to_one_creature(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] to_creature: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(card("Cunning Giant").unwrap())])
        .battlefield(
            1,
            [
                ready(free_creature("Boar", 3, 3)),
                ready(free_creature("Cub", 1, 1)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.attack(&[("Cunning Giant", 1)])?;
    let division = to_division(&mut driver, &[])?.expect("a division");
    assert_eq!(division.player, seat(0));
    // Its damage goes to one creature whole: a split between two, or part to one, is refused.
    assert!(driver.assign_damage(&[("Boar", 3), ("Cub", 1)]).is_err());
    assert!(driver.assign_damage(&[("Boar", 2)]).is_err());
    if to_creature {
        driver.assign_damage(&[("Boar", 4)])?;
    } else {
        driver.assign_damage(&[("Boar", 0)])?;
    }
    driver.pass_until(Step::EndCombat)?;
    assert_eq!(20 - life(&driver, 1), if to_creature { 0 } else { 4 });
    assert_eq!(alive(&driver, "Boar"), !to_creature);
    assert!(alive(&driver, "Cub"));
    replays(&scenario, &driver);
    Ok(())
}

#[rstest]
fn r109_defensive_formation_gives_the_division_to_the_player_attacked(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] formation: bool,
) -> Result<(), ScriptError> {
    let mut defender = vec![
        ready(free_creature("Bear", 2, 2)),
        ready(free_creature("Cub", 1, 1)),
    ];
    if formation {
        defender.push(ready(card("Defensive Formation").unwrap()));
    }
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(free_creature("Brute", 4, 4))])
        .battlefield(1, defender);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.attack(&[("Brute", 1)])?;
    let division =
        to_division(&mut driver, &[("Bear", "Brute"), ("Cub", "Brute")])?.expect("a division");
    let assigner = if formation { 1 } else { 0 };
    assert_eq!(division.player, seat(assigner));
    driver.assign_damage(&[("Bear", 0), ("Cub", 4)])?;
    driver.pass_until(Step::EndCombat)?;
    assert!(alive(&driver, "Bear"));
    assert!(!alive(&driver, "Cub"));
    replays(&scenario, &driver);
    Ok(())
}

#[rstest]
fn r109_defensive_formation_leaves_attacks_on_others_alone(
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(free_creature("Brute", 4, 4))])
        .battlefield(1, [ready(card("Defensive Formation").unwrap())])
        .battlefield(
            2,
            [
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Cub", 1, 1)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.attack(&[("Brute", 2)])?;
    let division =
        to_division(&mut driver, &[("Bear", "Brute"), ("Cub", "Brute")])?.expect("a division");
    assert_eq!(division.player, seat(0));
    Ok(())
}

#[rstest]
fn r109_ogre_enforcer_dies_only_to_one_sources_lethal_damage(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] enforcer: bool,
    #[values(false, true)] one_bolt: bool,
) -> Result<(), ScriptError> {
    let target = if enforcer {
        card("Ogre Enforcer").unwrap()
    } else {
        free_creature("Ogre Enforcer", 4, 4)
    };
    let bolts = if one_bolt {
        vec![burn("Big Bolt", 4)]
    } else {
        vec![burn("Bolt A", 2), burn("Bolt B", 2)]
    };
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(0, [ready(mountain()), ready(mountain())])
        .battlefield(1, [ready(target)])
        .hand(0, bolts.clone());
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let ogre = driver.target_named("Ogre Enforcer")?;
    for bolt in &bolts {
        driver.cast_at(&bolt.name, std::slice::from_ref(&ogre))?;
        while !driver.state().stack().is_empty() {
            driver.pass()?;
        }
    }
    let marked = driver
        .state()
        .object(driver.id_of("Ogre Enforcer")?)
        .and_then(|o| o.permanent())
        .map(|p| (p.damage(), p.counters_of(&CounterKind::PlusOnePlusOne)));
    let survives = enforcer && !one_bolt;
    assert_eq!(
        alive(&driver, "Ogre Enforcer"),
        survives,
        "marked {marked:?}"
    );
    replays(&scenario, &driver);
    Ok(())
}

/// A 1/1 with "Prevent all damage that would be dealt to this creature."
fn warded() -> Arc<CardDefinition> {
    let shield: Ability =
        serde_json::from_str(r#"{"Replacement":{"when":{"Damage":{"to":"This"}},"by":"Prevent"}}"#)
            .unwrap();
    Arc::new((*free_creature("Ward", 1, 1)).clone().with_rules(shield))
}

#[rstest]
fn r109_alchemists_gambit_turn_lets_no_damage_be_prevented_then_loses(
    #[values(2, 3, 4)] seats: usize,
    #[values(false, true)] gambit: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .at(2, 0, Step::Main1)
        .battlefield(
            0,
            [
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
                ready(mountain()),
                ready(warded()),
            ],
        )
        .hand(0, [card("Alchemist's Gambit").unwrap(), burn("Bolt", 2)]);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    if gambit {
        driver.cast("Alchemist's Gambit")?;
        while !driver.state().stack().is_empty() {
            driver.pass()?;
        }
        driver.idle_until(3, Step::Main1)?;
        assert_eq!(
            driver.state().turn().active(),
            seat(0),
            "the extra turn is seat 0's"
        );
    }
    let ward = driver.target_named("Ward")?;
    driver.cast_at("Bolt", &[ward])?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(
        alive(&driver, "Ward"),
        !gambit,
        "the shield holds only outside that turn"
    );
    if gambit {
        driver.pass_until(Step::End)?;
        for _ in 0..20 {
            if !driver.state().is_alive(seat(0)) || driver.engine().decision().is_none() {
                break;
            }
            driver.answer_passively()?;
        }
        assert!(
            !driver.state().is_alive(seat(0)),
            "the uncleaved Gambit loses at that turn's end step"
        );
    }
    replays(&scenario, &driver);
    Ok(())
}
