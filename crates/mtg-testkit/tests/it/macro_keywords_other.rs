//! Goal: the macro keywords and verbs beyond the first table play whole at tables of two to four seats as their rules say: living metal makes a Vehicle a creature on its controller's turn only, increment adds a counter when more mana was spent than the power or the toughness, teamwork is an optional tap-power cost the spell reads, blight puts -1/-1 counters on a creature its player controls, recruit loots and makes a Soldier only for a nonland discard, empower makes one Jace token and adds loyalty to it, and assimilate steals a creature card as a Borg artifact with a counter.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use crate::macro_keywords::{fight_on, macro_keyword, permanents, play, size, table};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::Effect;
use mtg_cards::macros;
use mtg_cards::refs::Ref;
use mtg_cards::value::{Amount, CounterKind};
use mtg_core::card::CardType;
use mtg_engine::characteristics;
use mtg_engine::choose::{CardsChoice, CostsChoice, Target, TargetChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, keyworded, keyworded_creature, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const ATTACKS: &str = r#"{"Attacks":{"what":["This"]}}"#;

/// A creature with one triggered ability: the trigger `when`, the target specifications `targets` and the effects.
fn triggered(name: &str, when: &str, targets: &str, effects: &[Effect]) -> Arc<CardDefinition> {
    let effects = serde_json::to_string(effects).unwrap();
    let ability: Ability = serde_json::from_str(&format!(
        r#"{{"Triggered":{{"when":{when},"targets":{targets},"effects":{effects}}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(4, 4)
            .with_rules(ability),
    )
}

/// How many counters of the kind the permanent holds.
fn counters(driver: &Driver, name: &str, kind: &CounterKind) -> u32 {
    let id = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters()
        .filter(|(held, _)| *held == kind)
        .map(|(_, count)| count)
        .sum()
}

fn is_creature(driver: &Driver, name: &str) -> bool {
    let id = driver.id_of(name).unwrap();
    characteristics(driver.state(), id).unwrap().is_creature()
}

/// Passes until the stack is empty, declining optional targets.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseTargets(_) => {
                driver.choose_targets(&[])?;
            }
            _ if driver.state().stack().is_empty() => return Ok(()),
            _ => {
                driver.pass()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn vehicle(name: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_power_toughness(3, 3)
            .with_rules(Ability::Keyword(macro_keyword("living metal", 0))),
    )
}

#[rstest]
fn living_metal_is_a_creature_on_its_controllers_turn_only(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1)] active: usize,
) -> Result<(), ScriptError> {
    let scenario = (0..seats)
        .fold(Scenario::new(seats), |scenario, seat| {
            scenario.battlefield(seat, [ready(free_creature(&format!("Pal {seat}"), 1, 1))])
        })
        .battlefield(0, [ready(vehicle("Truck"))])
        .at(1, active, Step::Main1);
    let driver = Driver::from_scenario(scenario)?;
    assert_eq!(is_creature(&driver, "Truck"), active == 0);
    Ok(())
}

/// Seat 0 casts a spell that costs `spent` with an Increment creature of this size and a land for each.
#[rstest]
#[case::two_two_one(2, 2, 1, 0)]
#[case::two_two_two(2, 2, 2, 0)]
#[case::two_two_three(2, 2, 3, 1)]
#[case::one_four_two(1, 4, 2, 1)]
#[case::four_one_two(4, 1, 2, 1)]
#[case::four_four_four(4, 4, 4, 0)]
fn increment_counts_the_mana_spent_against_power_and_toughness(
    #[values(2, 3, 4)] seats: usize,
    #[case] power: i32,
    #[case] toughness: i32,
    #[case] spent: u32,
    #[case] counters_after: u32,
) -> Result<(), ScriptError> {
    let student = keyworded(
        "Student",
        power,
        toughness,
        &[macro_keyword("increment", 0)],
    );
    let spell = keyworded_creature("Spell", spent, &[], 0, 1, &[]);
    let lands: Vec<_> = (0..spent).map(|_| ready(forest())).collect();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, std::iter::once(ready(student)).chain(lands))
        .hand(0, [spell]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Spell")?;
    settle(&mut driver)?;
    assert_eq!(
        counters(&driver, "Student", &CounterKind::PlusOnePlusOne),
        counters_after
    );
    // An opponent's spell never counts.
    Ok(())
}

#[rstest]
fn increment_ignores_a_spell_an_opponent_casts() -> Result<(), ScriptError> {
    let student = keyworded("Student", 1, 1, &[macro_keyword("increment", 0)]);
    let scenario = Scenario::new(2)
        .at(1, 1, Step::Main1)
        .battlefield(0, [ready(student)])
        .battlefield(1, [ready(forest()), ready(forest())])
        .hand(1, [keyworded_creature("Spell", 2, &[], 1, 1, &[])]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Spell")?;
    settle(&mut driver)?;
    assert_eq!(
        counters(&driver, "Student", &CounterKind::PlusOnePlusOne),
        0
    );
    Ok(())
}

/// A spell with teamwork `power` that gains 2 life, or 4 when it was cast using teamwork.
fn teamwork_spell(power: u32) -> Arc<CardDefinition> {
    let effects = r#"{"Spell":{"effects":[{"If":{"condition":"AdditionalCostPaid","then":[{"GainLife":{"amount":4}}],"otherwise":[{"GainLife":{"amount":2}}]}}]}}"#;
    Arc::new(
        CardDefinition::new("Huddle")
            .with_type(CardType::Instant)
            .with_mana_cost(mtg_testkit::cards::cost(1, &[]))
            .with_rules(Ability::Keyword(macro_keyword("teamwork", power)))
            .with_rules(serde_json::from_str(effects).unwrap()),
    )
}

#[rstest]
#[case::enough_and_used(&[2, 1], 3, true, 4)]
#[case::enough_and_declined(&[2, 1], 3, false, 2)]
#[case::exact(&[3], 3, true, 4)]
#[case::too_weak_is_not_offered(&[1, 1], 3, false, 2)]
#[case::no_creature(&[], 2, false, 2)]
fn teamwork_taps_creatures_of_enough_total_power_for_the_better_spell(
    #[values(2, 3, 4)] seats: usize,
    #[case] powers: &[i32],
    #[case] needed: u32,
    #[case] pays: bool,
    #[case] gained: i32,
) -> Result<(), ScriptError> {
    let crew: Vec<_> = powers
        .iter()
        .enumerate()
        .map(|(at, power)| ready(free_creature(&format!("Mate {at}"), *power, 1)))
        .collect();
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, crew.into_iter().chain([ready(forest())]))
        .hand(0, [teamwork_spell(needed)]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Huddle")?;
    if let DecisionKind::ChooseCosts(_) = &driver.decision()?.kind {
        let chosen = if pays { vec![0] } else { vec![] };
        driver.answer(Answer::ChooseCosts(CostsChoice::Costs(chosen)))?;
    }
    while let DecisionKind::ChooseCards(options) = &driver.decision()?.kind {
        let all = options.from.clone();
        driver.answer(Answer::ChooseCards(CardsChoice(all)))?;
    }
    driver.expect_decision(0, "pay-mana")?;
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    settle(&mut driver)?;
    let life = driver.state().player(pid(0)).unwrap().life();
    assert_eq!(life, 20 + gained);
    let tapped = powers
        .iter()
        .enumerate()
        .filter(|(at, _)| {
            let id = driver.id_of(&format!("Mate {at}")).unwrap();
            driver
                .state()
                .object(id)
                .unwrap()
                .permanent()
                .unwrap()
                .tapped()
        })
        .count();
    assert_eq!(tapped > 0, gained == 4);
    Ok(())
}

#[rstest]
fn blight_puts_minus_counters_on_a_creature_its_player_controls(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2)] amount: u32,
) -> Result<(), ScriptError> {
    let blighter = triggered(
        "Blighter",
        ATTACKS,
        "[]",
        &macros::action_effects("blight", amount, &Ref::This).unwrap(),
    );
    let mut driver =
        Driver::from_scenario(table(seats, vec![blighter], &[free_creature("Wall", 0, 4)]))?;
    play(
        &mut driver,
        &["Blighter"],
        &[],
        &|_, _| None,
        Step::EndCombat,
    )?;
    // The only creature seat 0 controls takes them; the opponents' creatures take none.
    assert_eq!(
        counters(&driver, "Blighter", &CounterKind::MinusOneMinusOne),
        amount
    );
    assert_eq!(counters(&driver, "Wall", &CounterKind::MinusOneMinusOne), 0);
    Ok(())
}

#[rstest]
#[case::nonland_makes_a_soldier("Hound", false, 1)]
#[case::land_makes_none("Plains", true, 0)]
fn recruit_loots_and_makes_a_soldier_for_a_nonland_discard(
    #[values(2, 3, 4)] seats: usize,
    #[case] card: &str,
    #[case] land: bool,
    #[case] soldiers: usize,
) -> Result<(), ScriptError> {
    let scout = triggered(
        "Scout",
        ATTACKS,
        "[]",
        &macros::action_effects("recruit", 0, &Ref::This).unwrap(),
    );
    let top = if land {
        plains()
    } else {
        free_creature(card, 2, 2)
    };
    let scenario = table(seats, vec![scout], &[free_creature("Wall", 0, 4)]).library(0, [top]);
    let mut driver = Driver::from_scenario(scenario)?;
    play(&mut driver, &["Scout"], &[], &|_, _| None, Step::EndCombat)?;
    assert_eq!(permanents(&driver, 0), 1 + soldiers);
    assert_eq!(driver.state().zone(Zone::Graveyard(pid(0))).len(), 1);
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), 0);
    if soldiers == 1 {
        assert_eq!(size(&driver, "Human Soldier"), (1, 1));
    }
    Ok(())
}

#[rstest]
fn empower_makes_one_jace_and_adds_loyalty_to_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 4)] first: u32,
    #[values(1, 3)] second: u32,
) -> Result<(), ScriptError> {
    let make = |name: &str, amount: u32| {
        triggered(
            name,
            ATTACKS,
            "[]",
            &macros::action_effects("empower", amount, &Ref::This).unwrap(),
        )
    };
    let mut driver = Driver::from_scenario(table(
        seats,
        vec![make("Alpha", first), make("Beta", second)],
        &[free_creature("Wall", 0, 4)],
    ))?;
    play(
        &mut driver,
        &["Alpha", "Beta"],
        &[],
        &|_, _| None,
        Step::EndCombat,
    )?;
    let jaces = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| driver.state().object(**id).unwrap().card().name == "Jace")
        .count();
    assert_eq!(jaces, 1);
    assert_eq!(
        counters(&driver, "Jace", &CounterKind::Loyalty),
        first + second
    );
    Ok(())
}

#[rstest]
fn empower_by_a_counted_amount(#[values(2, 3)] seats: usize) -> Result<(), ScriptError> {
    // "Empower Jace X, where X is the number of creatures you control."
    let count: Amount =
        serde_json::from_str(r#"{"Count":[{"Type":"Creature"},{"ControlledBy":"You"}]}"#).unwrap();
    let leader = triggered(
        "Leader",
        ATTACKS,
        "[]",
        &macros::action_effects_of("empower", &count, &Ref::This).unwrap(),
    );
    let mut driver = Driver::from_scenario(table(
        seats,
        vec![leader, free_creature("Pal", 1, 1)],
        &[free_creature("Wall", 0, 4)],
    ))?;
    play(&mut driver, &["Leader"], &[], &|_, _| None, Step::EndCombat)?;
    assert_eq!(counters(&driver, "Jace", &CounterKind::Loyalty), 2);
    Ok(())
}

#[rstest]
fn assimilate_takes_a_creature_card_as_a_borg_artifact_with_a_counter(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let queen = triggered(
        "Queen",
        ATTACKS,
        r#"[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"},{"OwnedBy":"Opponents"}]}}]"#,
        &macros::action_effects("assimilate", 1, &Ref::Target(0)).unwrap(),
    );
    let elf = Arc::new(
        CardDefinition::new("Elf Scout")
            .with_type(CardType::Creature)
            .with_subtype("Elf")
            .with_power_toughness(2, 2),
    );
    let scenario = table(seats, vec![queen], &[free_creature("Wall", 0, 4)]).graveyard(1, [elf]);
    let mut driver = Driver::from_scenario(scenario)?;
    let hook = |driver: &Driver, decision: &Decision| match &decision.kind {
        DecisionKind::ChooseTargets(options) => {
            let found = options.legal.iter().find(|target| {
                matches!(target, Target::Object(object)
                    if driver.state().object(object.id).is_some_and(|o| o.card().name == "Elf Scout"))
            })?;
            Some(Answer::ChooseTargets(TargetChoice::Targets(vec![*found])))
        }
        _ => None,
    };
    play(&mut driver, &["Queen"], &[], &hook, Step::EndCombat)?;
    let id = driver.id_of("Elf Scout")?;
    let object = driver.state().object(id).unwrap();
    assert_eq!(object.controller(), pid(0));
    assert_eq!(object.zone(), Zone::Battlefield);
    let chars = characteristics(driver.state(), id).unwrap();
    assert!(chars.types.contains(CardType::Artifact) && chars.types.contains(CardType::Creature));
    assert_eq!(
        chars
            .subtypes
            .iter()
            .map(|s| s.0.as_str())
            .collect::<Vec<_>>(),
        ["Borg"]
    );
    assert_eq!(size(&driver, "Elf Scout"), (3, 3));
    fight_on(&mut driver, Step::Main2)?;
    Ok(())
}
