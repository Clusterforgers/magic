//! Goal: the words that change how a creature assigns combat damage play as the rules say: a creature that assigns no combat damage (CR 510.1) deals none whether it is blocked or not and a plain creature beside it still does, a creature that assigns damage equal to its toughness (CR 510.1a) deals that much to a player and to a blocker, and a blocked creature whose controller may assign damage as though it were not blocked (CR 510.1c) is asked each time, whatever the blockers, gives all its damage to the player attacked on the answer of no damage to any blocker, with or without trample, and is still held to the normal division otherwise.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::modify::{CombatQuirk, GameRule};
use mtg_cards::refs::{Ref, ZoneKind};
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common;
use common::combat::to_blockers;

/// A creature of the given size with the combat word on itself.
fn worded(
    name: &str,
    power: i32,
    toughness: i32,
    quirk: CombatQuirk,
    keywords: &[Keyword],
) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name)
        .with_type(CardType::Creature)
        .with_power_toughness(power, toughness)
        .with_rules(Ability::Rule {
            rule: GameRule::Combat {
                who: Ref::This,
                quirk,
            },
            condition: None,
            from: ZoneKind::Battlefield,
        });
    for keyword in keywords {
        card = card.with_rules(Ability::Keyword(keyword.clone()));
    }
    Arc::new(card)
}

fn attack_through(driver: &mut Driver, blocks: &[(&str, &str)]) -> Result<(), ScriptError> {
    driver.attack(&[("A", 1)])?;
    to_blockers(driver, 1)?;
    driver.block(blocks)?;
    driver.pass()?;
    driver.pass()?;
    Ok(())
}

#[rstest]
#[case::unblocked(false, 20, true)]
#[case::blocked(true, 20, false)]
fn a_creature_that_assigns_no_combat_damage_deals_none(
    #[case] blocked: bool,
    #[case] life: i32,
    #[case] blocker_hurt: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(
                0,
                [
                    ready(worded("A", 3, 3, CombatQuirk::AssignsNoCombatDamage, &[])),
                    ready(free_creature("Plain", 3, 3)),
                ],
            )
            .battlefield(1, [ready(free_creature("B", 1, 4))]),
    )?;
    driver.attack(&[("A", 1), ("Plain", 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&if blocked { vec![("B", "A")] } else { vec![] })?;
    driver.idle_until(2, Step::EndCombat)?;
    // The plain creature always hits for 3; the worded one adds nothing.
    driver.expect_life(1, life - 3)?;
    let hurt = driver
        .state()
        .object(driver.id_of("B")?)
        .and_then(|object| object.permanent())
        .is_some_and(|permanent| permanent.damage() > 0);
    assert_eq!(hurt, blocker_hurt && blocked);
    Ok(())
}

#[rstest]
#[case::to_a_player(false, 4)]
#[case::to_a_blocker(true, 0)]
fn a_creature_that_assigns_by_toughness_deals_its_toughness(
    #[case] blocked: bool,
    #[case] player_loses: i32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(
                0,
                [ready(worded(
                    "A",
                    1,
                    4,
                    CombatQuirk::AssignsByToughness,
                    &[],
                ))],
            )
            .battlefield(1, [ready(free_creature("B", 1, 4))]),
    )?;
    driver.attack(&[("A", 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&if blocked { vec![("B", "A")] } else { vec![] })?;
    driver.idle_until(2, Step::EndCombat)?;
    driver.expect_life(1, 20 - player_loses)?;
    if blocked {
        // Four damage kills the 1/4 blocker, which one would not.
        assert!(driver.id_of("B").is_err(), "B survived");
    }
    Ok(())
}

fn unblocked_table(keywords: &[Keyword], blockers: &[&str]) -> Result<Driver, ScriptError> {
    Driver::from_scenario(
        Scenario::new(2)
            .at(2, 0, Step::DeclareAttackers)
            .battlefield(
                0,
                [ready(worded(
                    "A",
                    3,
                    3,
                    CombatQuirk::AssignsAsUnblocked,
                    keywords,
                ))],
            )
            .battlefield(
                1,
                blockers.iter().map(|name| ready(free_creature(name, 1, 5))),
            ),
    )
}

fn question(driver: &Driver) -> Option<bool> {
    match &driver.decision().ok()?.kind {
        DecisionKind::AssignDamage(options) => Some(options.unblocked),
        _ => None,
    }
}

#[rstest]
#[case::one_blocker(&[][..], &["B1"][..])]
#[case::two_blockers(&[][..], &["B1", "B2"][..])]
#[case::trampler(&[Keyword::Trample][..], &["B1"][..])]
fn a_blocked_creature_is_asked_whether_to_assign_as_though_unblocked(
    #[case] keywords: &[Keyword],
    #[case] blockers: &[&str],
) -> Result<(), ScriptError> {
    let mut driver = unblocked_table(keywords, blockers)?;
    let blocks: Vec<(&str, &str)> = blockers.iter().map(|name| (*name, "A")).collect();
    attack_through(&mut driver, &blocks)?;
    driver.expect_decision(0, "assign-damage")?;
    assert_eq!(question(&driver), Some(true));
    // No damage to any blocker: all of it goes to the player attacked.
    let none: Vec<(&str, u32)> = blockers.iter().map(|name| (*name, 0)).collect();
    driver.assign_damage(&none)?;
    driver.idle_until(2, Step::EndCombat)?;
    driver.expect_life(1, 17)?;
    for name in blockers {
        let hurt = driver
            .state()
            .object(driver.id_of(name)?)
            .and_then(|object| object.permanent())
            .is_some_and(|permanent| permanent.damage() > 0);
        assert!(!hurt, "{name} was hurt");
    }
    Ok(())
}

#[test]
fn the_normal_division_is_still_open_and_still_held_to_the_rules() -> Result<(), ScriptError> {
    let mut driver = unblocked_table(&[], &["B1"])?;
    attack_through(&mut driver, &[("B1", "A")])?;
    // Two of three damage to the blocker is no division: all of it goes to the blocker, or none does.
    assert!(driver.assign_damage(&[("B1", 2)]).is_err());
    driver.assign_damage(&[("B1", 3)])?;
    driver.idle_until(2, Step::EndCombat)?;
    driver.expect_life(1, 20)?;
    Ok(())
}

#[test]
fn a_creature_that_cannot_be_asked_is_not_blocked_and_asks_nothing() -> Result<(), ScriptError> {
    let mut driver = unblocked_table(&[], &["B1"])?;
    driver.attack(&[("A", 1)])?;
    to_blockers(&mut driver, 1)?;
    driver.block(&[])?;
    driver.pass()?;
    driver.pass()?;
    assert_eq!(question(&driver), None);
    driver.idle_until(2, Step::EndCombat)?;
    driver.expect_life(1, 17)?;
    Ok(())
}
