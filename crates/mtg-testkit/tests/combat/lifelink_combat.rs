//! Goal: lifelink gains its controller the combat damage the creature deals, whichever way it has the keyword (printed, from an Aura, from an Equipment or from a continuous effect), with or without first strike, and to a player, a planeswalker, a battle or a creature, at tables of two to four seats in Commander.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::format::Commander;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, free_creature, keyworded, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::loyalty_cards::dawn_walker;
use mtg_testkit::pump_cards::grant;
use mtg_testkit::{Driver, Scenario, ScriptError, attached_to, protected_by, ready, with_counters};
use rstest::rstest;

use crate::common;
use common::pid;

const SIEGE: &str = "Invasion of Theros";

#[derive(Debug, Clone, Copy)]
enum Source {
    Printed,
    Aura,
    Equipment,
    Lord,
    AuraAbility,
    LordAbility,
}

#[derive(Debug, Clone, Copy)]
enum Victim {
    Player,
    Walker,
    Battle,
    Creature,
}

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

const GRANT: &str = r#"{"Static":{"affected":"Attached","mods":[{"AddKeyword":"Lifelink"}]}}"#;
const GRANT_ABILITY: &str =
    r#"{"Static":{"affected":"Attached","mods":[{"GrantAbility":{"Keyword":"Lifelink"}}]}}"#;
const LORD_ABILITY: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"GrantAbility":{"Keyword":"Lifelink"}}]}}"#;
const LORD: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"mods":[{"AddKeyword":"Lifelink"}]}}"#;

fn granter(subtype: &str, ability: &str, kind: CardType) -> Arc<CardDefinition> {
    let mut def = CardDefinition::new("Granter")
        .with_type(kind)
        .with_mana_cost(cost(0, &[]));
    if !subtype.is_empty() {
        def = def.with_subtype(subtype);
    }
    if subtype == "Aura" {
        def = def.with_rules(rules(r#"{"Keyword":{"Enchant":[{"Type":"Creature"}]}}"#));
    }
    Arc::new(def.with_rules(rules(ability)))
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

#[rstest]
fn lifelink_gains_the_combat_damage_dealt(
    #[values(
        Source::Printed,
        Source::Aura,
        Source::Equipment,
        Source::Lord,
        Source::AuraAbility,
        Source::LordAbility
    )]
    source: Source,
    #[values(Victim::Player, Victim::Walker, Victim::Battle, Victim::Creature)] victim: Victim,
    #[values(false, true)] first_strike: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut keywords = vec![];
    if matches!(source, Source::Printed) {
        keywords.push(Keyword::Lifelink);
    }
    if first_strike {
        keywords.push(Keyword::FirstStrike);
    }
    let mut mine = vec![ready(keyworded("A", 3, 3, &keywords))];
    match source {
        Source::Aura => mine.push(attached_to(
            ready(granter("Aura", GRANT, CardType::Enchantment)),
            0,
            0,
        )),
        Source::Equipment => mine.push(attached_to(
            ready(granter("Equipment", GRANT, CardType::Artifact)),
            0,
            0,
        )),
        Source::Lord => mine.push(ready(granter("", LORD, CardType::Enchantment))),
        Source::AuraAbility => mine.push(attached_to(
            ready(granter("Aura", GRANT_ABILITY, CardType::Enchantment)),
            0,
            0,
        )),
        Source::LordAbility => {
            mine.push(ready(granter("", LORD_ABILITY, CardType::Enchantment)));
        }
        Source::Printed => {}
    }
    let mut theirs = vec![];
    match victim {
        Victim::Player => {}
        Victim::Walker => theirs.push(with_counters(
            ready(dawn_walker().unwrap()),
            CounterKind::Loyalty,
            5,
        )),
        Victim::Battle => theirs.push(protected_by(
            with_counters(ready(card(SIEGE).unwrap()), CounterKind::defense(), 5),
            1,
        )),
        Victim::Creature => theirs.push(ready(free_creature("B", 4, 4))),
    }
    let scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, mine)
        .battlefield(1, theirs);
    let mut driver = Driver::from_scenario(scenario)?;
    match victim {
        Victim::Player => {
            driver.attack(&[("A", 1)])?;
        }
        Victim::Walker => {
            driver.attack_planeswalkers(&[("A", "Dawn Walker")])?;
        }
        Victim::Battle => {
            driver.attack_planeswalkers(&[("A", SIEGE)])?;
        }
        Victim::Creature => {
            driver.attack(&[("A", 1)])?;
            common::combat::to_blockers(&mut driver, 1)?;
            driver.block(&[("B", "A")])?;
        }
    }
    driver.idle_until(2, Step::EndCombat)?;
    assert_eq!(
        life(&driver, 0),
        43,
        "{source:?} {victim:?} first_strike={first_strike} seats={seats}"
    );
    Ok(())
}

#[rstest]
fn lifelink_given_by_a_spell_after_attackers_gains_the_damage_to_the_player(
    #[values(false, true)] double_strike: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let keywords: &[Keyword] = if double_strike {
        &[Keyword::DoubleStrike]
    } else {
        &[]
    };
    let scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .at(2, 0, Step::DeclareAttackers)
        .hand(0, [grant("Gift", Keyword::Lifelink)])
        .battlefield(
            0,
            [ready(keyworded("A", 3, 3, keywords)), ready(mountain())],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("A", 1)])?;
    let target = driver.target_named("A")?;
    driver.cast_at("Gift", &[target])?;
    driver.idle_until(2, Step::EndCombat)?;
    let hits = if double_strike { 2 } else { 1 };
    assert_eq!(life(&driver, 1), 40 - 3 * hits);
    assert_eq!(life(&driver, 0), 40 + 3 * hits);
    Ok(())
}

#[rstest]
fn several_attackers_each_lifelinker_gains_its_own_damage(
    #[values(true, false)] plain_first: bool,
    #[values(3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let link = || ready(keyworded("L", 3, 3, &[Keyword::Lifelink]));
    let link2 = || ready(keyworded("M", 2, 2, &[Keyword::Lifelink, Keyword::Trample]));
    let plain = || ready(free_creature("P", 5, 5));
    let mine = if plain_first {
        vec![plain(), link(), link2()]
    } else {
        vec![link(), link2(), plain()]
    };
    let scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, mine)
        .battlefield(1, [ready(free_creature("B", 1, 1))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("L", 1), ("M", 2), ("P", 1)])?;
    common::combat::to_blockers(&mut driver, 1)?;
    driver.block(&[("B", "L")])?;
    driver.idle_until(2, Step::EndCombat)?;
    assert_eq!(life(&driver, 1), 35, "only P reaches seat 1");
    assert_eq!(life(&driver, 2), 38, "M reaches seat 2");
    assert_eq!(life(&driver, 0), 45, "L and M gain 3 and 2");
    Ok(())
}

#[rstest]
fn a_lifelink_commander_cast_from_the_command_zone_gains_its_combat_damage(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let commander = Arc::new(
        CardDefinition::new("Cmd")
            .with_type(CardType::Creature)
            .with_supertype(mtg_core::card::Supertype::Legendary)
            .with_mana_cost(cost(1, &[]))
            .with_power_toughness(3, 3)
            .with_rules(rules(r#"{"Keyword":"Lifelink"}"#))
            .with_rules(rules(r#"{"Keyword":"Haste"}"#)),
    );
    let scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [commander])
        .battlefield(0, [ready(mountain()), ready(mountain())]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Cmd")?;
    for _ in 0..40 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Cmd", 1)])?;
    driver.idle_until(1, Step::EndCombat)?;
    assert_eq!(life(&driver, 1), 37);
    assert_eq!(life(&driver, 0), 43);
    Ok(())
}

#[rstest]
fn two_blockers_and_trample_still_gain_all_the_damage(
    #[values(2, 4)] seats: usize,
    #[values(true, false)] trample: bool,
) -> Result<(), ScriptError> {
    let keywords: &[Keyword] = if trample {
        &[Keyword::Lifelink, Keyword::Trample]
    } else {
        &[Keyword::Lifelink]
    };
    let scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(keyworded("A", 5, 5, keywords))])
        .battlefield(
            1,
            [
                ready(free_creature("B", 1, 1)),
                ready(free_creature("C", 1, 1)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("A", 1)])?;
    common::combat::to_blockers(&mut driver, 1)?;
    driver.block(&[("B", "A"), ("C", "A")])?;
    for _ in 0..40 {
        match driver.decision()?.kind {
            mtg_engine::decision::DecisionKind::AssignDamage(_) => {
                driver.assign_damage(&[
                    ("B", if trample { 1 } else { 3 }),
                    ("C", 2 - u32::from(trample)),
                ])?;
            }
            _ if driver.state().turn().step() == Step::EndCombat => break,
            _ => {
                driver.pass()?;
            }
        }
    }
    let dealt = 5;
    assert_eq!(life(&driver, 0), 40 + dealt);
    assert_eq!(life(&driver, 1), if trample { 37 } else { 40 });
    Ok(())
}

#[rstest]
fn trample_lifelink_gains_the_damage_to_the_blocker_and_to_the_player(
    #[values(true, false)] printed: bool,
    #[values(false, true)] first_strike: bool,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut keywords = vec![Keyword::Trample];
    if printed {
        keywords.push(Keyword::Lifelink);
    }
    if first_strike {
        keywords.push(Keyword::FirstStrike);
    }
    let mut mine = vec![ready(keyworded("A", 4, 4, &keywords))];
    if !printed {
        mine.push(ready(granter("", LORD, CardType::Enchantment)));
    }
    let scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, mine)
        .battlefield(1, [ready(free_creature("B", 1, 1))]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("A", 1)])?;
    common::combat::to_blockers(&mut driver, 1)?;
    driver.block(&[("B", "A")])?;
    for _ in 0..40 {
        match driver.decision()?.kind {
            mtg_engine::decision::DecisionKind::AssignDamage(_) => {
                driver.assign_damage(&[("B", 1)])?;
            }
            _ if driver.state().turn().step() == Step::EndCombat => break,
            _ => {
                driver.pass()?;
            }
        }
    }
    assert_eq!(life(&driver, 1), 37, "3 of the 4 reached the player");
    assert_eq!(life(&driver, 0), 44, "all 4 dealt gain 4");
    Ok(())
}
