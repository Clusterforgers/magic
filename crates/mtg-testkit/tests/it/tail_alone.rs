//! Goal: the cards that alone held back a few precon cards each play as printed at tables of two, three and four seats: the end step trigger that counts the life gained this turn (Angelic Accord), the choice to gain life as another creature enters (Soul's Attendant), the Equipment that attaches itself as it enters (Maul of the Skyclaves), the damage that is prevented for a counter (Phantom Nantuko) or only in combat (Guard Gomazoa, Seraph of the Sword), and the card that connives as it attacks (Kang).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::on_battlefield;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::Event;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::Answer;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, cost, free_creature, gain_life, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

/// An instant for `{G}` that gains `amount` life.
fn brew(amount: i32) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Brew")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Green]))
            .with_rules(Ability::Spell {
                targets: Vec::new(),
                effects: vec![gain_life(amount)],
            }),
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

/// The number of permanents called `name` on the battlefield.
fn count(driver: &Driver, name: &str) -> usize {
    crate::common::names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|found| found.as_str() == name)
        .count()
}

/// The `+1/+1` counters on the named permanent.
fn counters(driver: &Driver, name: &str) -> u32 {
    let id = on_battlefield(driver.state(), name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .map_or(0, |held| held.counters_of(&CounterKind::PlusOnePlusOne))
}

#[rstest]
fn life_gained_this_turn_decides_the_end_step_token(
    #[values(2, 3, 4)] seats: usize,
    #[values(3, 4, 5)] gained: i32,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [brew(gained)])
        .battlefield(
            0,
            [
                ready(card("Angelic Accord").unwrap()),
                ready(mtg_testkit::cards::forest()),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(plains())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Brew")?;
    settle(&mut driver)?;
    driver.pass_until(Step::End)?;
    settle(&mut driver)?;
    // Four or more life makes one Angel at the end step of the turn it was gained.
    assert_eq!(count(&driver, "Angel"), usize::from(gained >= 4));
    // The next turn starts the tally again: no second Angel at another player's end step.
    driver.idle_until(2, Step::End)?;
    settle(&mut driver)?;
    assert_eq!(count(&driver, "Angel"), usize::from(gained >= 4));
    Ok(())
}

#[rstest]
fn the_choice_to_gain_life_as_another_creature_enters(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] gains: bool,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [free_creature("Newcomer", 1, 1)])
        .battlefield(0, [ready(card("Soul's Attendant").unwrap())]);
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Newcomer")?;
    settle_with_choice(&mut driver, gains)?;
    driver.expect_life(0, if gains { 21 } else { 20 })?;
    assert_eq!(count(&driver, "Newcomer"), 1);
    Ok(())
}

/// Passes until the stack is empty and answers the "may" question of the trigger with `yes`.
fn settle_with_choice(driver: &mut Driver, yes: bool) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        if driver.decision()?.kind.name() == "yes-no" {
            driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
        } else {
            driver.answer_passively()?;
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
fn an_equipment_attaches_itself_to_the_creature_it_chooses_as_it_enters(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card("Maul of the Skyclaves").unwrap()])
        .battlefield(
            0,
            [
                ready(free_creature("Wearer", 2, 2)),
                ready(plains()),
                ready(plains()),
                ready(plains()),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Bystander", 1, 1))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Maul of the Skyclaves")?;
    // The Equipment resolves; its trigger asks for a creature this player controls, never another seat's.
    for _ in 0..4 {
        if driver.decision()?.kind.name() == "choose-targets" {
            break;
        }
        driver.answer_passively()?;
    }
    let target = driver.target_named("Wearer")?;
    driver.choose_targets(&[target])?;
    settle(&mut driver)?;
    let maul = driver.id_of("Maul of the Skyclaves")?;
    let wearer = driver.id_of("Wearer")?;
    assert_eq!(driver.state().host_of(maul), Some(wearer));
    Ok(())
}

#[rstest]
fn damage_to_a_phantom_is_prevented_for_a_counter(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3, 6)] held: u32,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [burn("Bolt", 3)])
        .battlefield(
            0,
            [
                ready(mtg_testkit::cards::mountain()),
                ready(mtg_testkit::cards::mountain()),
                with_counters(
                    ready(card("Phantom Nantuko").unwrap()),
                    CounterKind::PlusOnePlusOne,
                    held,
                ),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    let target = driver.target_named("Phantom Nantuko")?;
    driver.cast_at("Bolt", &[target])?;
    settle(&mut driver)?;
    // The damage is prevented and one counter is removed; the 0/0 Phantom with its last counter gone dies by its size.
    assert_eq!(count(&driver, "Phantom Nantuko"), usize::from(held >= 2));
    if held >= 2 {
        assert_eq!(counters(&driver, "Phantom Nantuko"), held - 1);
    }
    assert!(
        !driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::DamageDealt { target, .. } if format!("{target:?}").contains("Object"))),
        "no damage is dealt to the Phantom"
    );
    Ok(())
}

#[rstest]
#[case::guard_gomazoa("Guard Gomazoa")]
#[case::seraph_of_the_sword("Seraph of the Sword")]
fn combat_damage_to_the_card_is_prevented_and_other_damage_is_not(
    #[case] name: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Combat: a 5/5 attacks and the card blocks it; the card survives.
    let mut scenario = Scenario::new(seats)
        .at(2, 1, Step::DeclareAttackers)
        .battlefield(1, [ready(free_creature("Attacker", 5, 5))])
        .battlefield(0, [ready(card(name).unwrap())]);
    for seat in 2..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Attacker", 0)])?;
    for _ in 0..seats {
        if matches!(
            driver.decision()?.kind,
            mtg_engine::decision::DecisionKind::DeclareBlockers(_)
        ) {
            break;
        }
        driver.pass()?;
    }
    driver.block(&[(name, "Attacker")])?;
    driver.idle_until(2, Step::EndCombat)?;
    assert_eq!(count(&driver, name), 1, "{name} takes no combat damage");

    // Other damage: a Bolt kills it.
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, [burn("Bolt", 5)])
            .battlefield(
                0,
                [
                    ready(mtg_testkit::cards::mountain()),
                    ready(mtg_testkit::cards::mountain()),
                    ready(card(name).unwrap()),
                ],
            ),
    )?;
    let target = driver.target_named(name)?;
    driver.cast_at("Bolt", &[target])?;
    settle(&mut driver)?;
    assert_eq!(
        count(&driver, name),
        0,
        "{name} dies to damage that is not combat damage"
    );
    Ok(())
}

#[rstest]
fn a_card_that_connives_as_he_attacks(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats)
        .at(2, 0, Step::DeclareAttackers)
        .battlefield(0, [ready(card("Kang, Temporal Tyrant").unwrap())])
        .library(
            0,
            [free_creature("Filler", 1, 1), free_creature("Filler", 1, 1)],
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(swamp())]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    driver.attack(&[("Kang, Temporal Tyrant", 1)])?;
    // Connive draws a card and discards a card; a nonland card discarded puts a counter on Kang.
    for _ in 0..30 {
        if driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::Connived { .. }))
        {
            break;
        }
        driver.answer_passively()?;
    }
    assert!(
        driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::Connived { .. })),
        "Kang connived as it attacked"
    );
    Ok(())
}
