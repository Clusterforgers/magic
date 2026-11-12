//! Goal: the compiled Observed Stasis plays as the rules say: on entering it removes the attacking creature it enchants from combat and draws a card for each tapped creature that creature's controller controls, the creature then loses every ability (keywords, activated and triggered) and can neither be declared as an attacker nor as a blocker, and all of it ends when the Aura leaves the battlefield.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::{name_of, pid};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::ids::ObjectId;
use mtg_engine::ability_reader::abilities_of;
use mtg_engine::decision::DecisionKind;
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, free_creature, island, keyworded};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, tapped};
use rstest::rstest;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// A 3/3 with flying, an activated ability and a trigger of its own.
fn raider(vigilant: bool) -> Arc<CardDefinition> {
    let keywords: &[Keyword] = if vigilant {
        &[Keyword::Flying, Keyword::Vigilance]
    } else {
        &[Keyword::Flying]
    };
    let base = keyworded("Raider", 3, 3, keywords);
    Arc::new(
        (*base)
            .clone()
            .with_rules(ability(
                r#"{"Triggered":{"when":{"Attacks":{"what":["This"]}},"effects":[{"GainLife":{"amount":1}}]}}"#,
            ))
            .with_rules(ability(
                r#"{"Activated":{"cost":"{0}","effects":[{"GainLife":{"amount":1}}]}}"#,
            )),
    )
}

/// A free instant that destroys target enchantment.
fn smite() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Smite")
            .with_type(mtg_core::card::CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability(
                r#"{"Spell":{"targets":[{"what":{"Object":[{"Type":"Enchantment"}]}}],"effects":[{"Destroy":{"what":{"Target":0}}}]}}"#,
            )),
    )
}

/// Turn 2 of seat 0 at the declaration of attackers: the Raider, a Wolf, an untapped Idle and a tapped Sleeper of seat 0,
/// a Hound, four Islands, Observed Stasis and a Smite of seat 1.
fn duel(vigilant: bool) -> Result<Driver, ScriptError> {
    let filler = || (0..12).map(|_| free_creature("Filler", 1, 1));
    let lands = (0..4).map(|_| ready(island())).collect::<Vec<_>>();
    let mut seat_one = vec![ready(free_creature("Hound", 2, 2))];
    seat_one.extend(lands);
    Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .library(0, filler())
            .library(1, filler())
            .battlefield(
                0,
                [
                    ready(raider(vigilant)),
                    ready(free_creature("Wolf", 2, 2)),
                    ready(free_creature("Idle", 1, 1)),
                    tapped(free_creature("Sleeper", 1, 1)),
                ],
            )
            .battlefield(1, seat_one)
            .hand(1, [card("Observed Stasis").unwrap(), smite()]),
    )
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// Seat 0 attacks with `attackers`, seat 1 casts Observed Stasis on the Raider and everything resolves.
fn stasis_after_attack(attackers: &[&str], vigilant: bool) -> Result<Driver, ScriptError> {
    let mut driver = duel(vigilant)?;
    let pairs: Vec<(&str, usize)> = attackers.iter().map(|name| (*name, 1)).collect();
    driver.attack(&pairs)?;
    driver.pass()?;
    let raider = driver.target_named("Raider")?;
    driver.cast_at("Observed Stasis", &[raider])?;
    settle(&mut driver)?;
    Ok(driver)
}

fn hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

fn attacking(driver: &Driver) -> Vec<String> {
    let mut names: Vec<String> = driver
        .state()
        .combat()
        .attackers()
        .iter()
        .map(|a| name_of(driver.state(), a.object()))
        .collect();
    names.sort();
    names
}

fn raider_id(driver: &Driver) -> ObjectId {
    driver.id_of("Raider").unwrap()
}

/// The creatures the pending declaration of attackers offers.
fn attack_offers(driver: &Driver) -> Vec<String> {
    let DecisionKind::DeclareAttackers(options) = &driver.decision().unwrap().kind else {
        panic!("attackers are declared");
    };
    let mut names: Vec<String> = options
        .attackers
        .iter()
        .map(|option| name_of(driver.state(), option.object))
        .collect();
    names.sort();
    names
}

/// The creatures the pending declaration of blockers offers.
fn block_offers(driver: &Driver) -> Vec<String> {
    let DecisionKind::DeclareBlockers(options) = &driver.decision().unwrap().kind else {
        panic!("blockers are declared");
    };
    let mut names: Vec<String> = options
        .blockers
        .iter()
        .map(|option| name_of(driver.state(), option.object))
        .collect();
    names.sort();
    names
}

#[rstest]
#[case::two_attackers(&["Raider", "Wolf"], &["Wolf"], 3)]
#[case::the_raider_alone(&["Raider"], &[], 2)]
#[case::all_but_the_idle_one(&["Raider", "Wolf", "Idle"], &["Idle", "Wolf"], 4)]
fn the_enchanted_attacker_leaves_combat_and_its_controllers_tapped_creatures_are_counted(
    #[case] attackers: &[&str],
    #[case] still_attacking: &[&str],
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let driver = stasis_after_attack(attackers, false)?;
    assert_eq!(attacking(&driver), still_attacking);
    // The Aura left the hand and its owner drew a card for each tapped creature of the other seat: each attacker
    // (the Raider included) and the Sleeper.
    assert_eq!(hand(&driver, 1), 1 + drawn);
    assert_eq!(hand(&driver, 0), 0);
    Ok(())
}

#[test]
fn the_enchanted_creature_has_lost_every_ability() -> Result<(), ScriptError> {
    let mut driver = duel(false)?;
    let before = abilities_of(driver.state(), raider_id(&driver)).len();
    assert!(before >= 3, "a keyword, a trigger and an activated ability");
    assert!(has_keyword(
        driver.state(),
        raider_id(&driver),
        &Keyword::Flying
    ));
    driver.attack(&[("Raider", 1)])?;
    driver.pass()?;
    let raider = driver.target_named("Raider")?;
    driver.cast_at("Observed Stasis", &[raider])?;
    settle(&mut driver)?;
    let id = raider_id(&driver);
    assert_eq!(abilities_of(driver.state(), id).len(), 0);
    assert!(!has_keyword(driver.state(), id, &Keyword::Flying));
    assert_eq!(driver.offered_abilities("Raider")?.len(), 0);
    Ok(())
}

#[test]
fn the_enchanted_creature_is_offered_neither_as_an_attacker_nor_as_a_blocker_and_cannot_be_declared()
-> Result<(), ScriptError> {
    let mut driver = stasis_after_attack(&["Raider", "Wolf"], true)?;
    // Seat 1's turn: the vigilant Raider is untapped and would block, and so would the Idle creature.
    driver.idle_until(3, Step::DeclareAttackers)?;
    driver.attack(&[("Hound", 0)])?;
    for _ in 0..2 {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            break;
        }
        driver.pass()?;
    }
    let offered = block_offers(&driver);
    assert!(!offered.contains(&"Raider".to_owned()), "{offered:?}");
    assert!(offered.contains(&"Idle".to_owned()), "{offered:?}");
    assert!(driver.block(&[("Raider", "Hound")]).is_err());
    driver.block(&[])?;
    // Seat 0's next turn: the Raider is untapped and has been all along, and is still not offered.
    driver.idle_until(4, Step::DeclareAttackers)?;
    let offered = attack_offers(&driver);
    assert!(!offered.contains(&"Raider".to_owned()), "{offered:?}");
    assert!(offered.contains(&"Wolf".to_owned()), "{offered:?}");
    assert!(driver.attack(&[("Raider", 1)]).is_err());
    Ok(())
}

#[test]
fn everything_comes_back_when_the_aura_leaves_the_battlefield() -> Result<(), ScriptError> {
    let mut driver = duel(false)?;
    let before = abilities_of(driver.state(), raider_id(&driver)).len();
    driver.attack(&[("Raider", 1)])?;
    driver.pass()?;
    let raider = driver.target_named("Raider")?;
    driver.cast_at("Observed Stasis", &[raider])?;
    settle(&mut driver)?;
    assert_eq!(abilities_of(driver.state(), raider_id(&driver)).len(), 0);
    // The active player passes, and the other seat destroys the Aura.
    driver.pass()?;
    let aura = driver.target_named("Observed Stasis")?;
    driver.cast_at("Smite", &[aura])?;
    settle(&mut driver)?;
    let id = raider_id(&driver);
    assert_eq!(abilities_of(driver.state(), id).len(), before);
    assert!(has_keyword(driver.state(), id, &Keyword::Flying));
    // Its activated ability is offered again, and it does not return to the combat it was removed from.
    assert_eq!(driver.offered_abilities("Raider")?.len(), 1);
    assert_eq!(attacking(&driver).len(), 0);
    driver.idle_until(4, Step::DeclareAttackers)?;
    assert!(attack_offers(&driver).contains(&"Raider".to_owned()));
    Ok(())
}
