//! Goal: mana an effect keeps ("Until end of turn, you don't lose this mana as steps and phases end") stays in the pool through every step and phase of the turn and goes as the turn ends, is spent after the mana that goes sooner, pays together with mana of the sources, and the cards that add it (Savage Ventmaw, Klauth, Photon) play whole at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::spend::ManaLasts;
use mtg_core::card::CardType;
use mtg_core::mana::{ManaCost, ManaType};
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::events::Event;
use mtg_engine::mana::ManaPool;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A spell with this cost.
fn spell(name: &str, cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// Seat 0 controls the attacker, ready, and these cards in hand and permanents; it is turn 1, precombat main phase.
fn table(
    seats: usize,
    attacker: &str,
    hand: Vec<Arc<CardDefinition>>,
    lands: usize,
) -> Result<Driver, ScriptError> {
    let mut permanents = vec![ready(card(attacker).unwrap())];
    permanents.extend((0..lands).map(|_| ready(mountain())));
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, hand)
        .battlefield(0, permanents);
    for seat in 1..seats {
        scenario =
            scenario.battlefield(seat, [ready(free_creature(&format!("Wall {seat}"), 0, 9))]);
    }
    Driver::from_scenario(scenario)
}

fn pool(driver: &Driver) -> ManaPool {
    driver.state().player(pid(0)).unwrap().mana().clone()
}

/// The mana of the pool that is kept to the end of the turn, by type.
fn kept_turn(pool: &ManaPool, mana: ManaType) -> u32 {
    pool.kept().map_or(0, |kept| kept.turn_end[mana.index()])
}

/// Attacks seat 1 with the attacker and answers until the attack trigger has resolved and the game waits at priority
/// with an empty stack.
fn attack(driver: &mut Driver, attacker: &str) -> Result<(), ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[(attacker, 1)])?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    panic!("the attack trigger did not resolve");
}

/// The total of the pool of seat 0 at every priority the game gives out until the third turn begins, with the turn and step.
fn walk(driver: &mut Driver) -> Result<Vec<(u32, Step, u32)>, ScriptError> {
    let mut seen = Vec::new();
    for _ in 0..400 {
        let state = driver.state();
        if state.turn().number() >= 3 {
            return Ok(seen);
        }
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) {
            seen.push((
                state.turn().number(),
                state.turn().step(),
                pool(driver).total(),
            ));
            driver.pass()?;
        } else {
            driver.answer_passively()?;
        }
    }
    panic!("the turns did not pass");
}

#[rstest]
fn mana_kept_to_the_end_of_the_turn_stays_through_the_steps_and_phases_and_goes_with_the_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, "Savage Ventmaw", Vec::new(), 0)?;
    attack(&mut driver, "Savage Ventmaw")?;
    let held = pool(&driver);
    assert_eq!(held.total(), 6);
    assert_eq!(kept_turn(&held, ManaType::Red), 3);
    assert_eq!(kept_turn(&held, ManaType::Green), 3);
    let seen = walk(&mut driver)?;
    let turn_one: Vec<_> = seen.iter().filter(|(turn, _, _)| *turn == 1).collect();
    for step in [
        Step::DeclareAttackers,
        Step::DeclareBlockers,
        Step::CombatDamage,
        Step::EndCombat,
        Step::Main2,
        Step::End,
    ] {
        assert!(
            turn_one.iter().any(|(_, at, _)| *at == step),
            "the walk reaches {step:?}: {seen:?}"
        );
    }
    assert!(
        turn_one.iter().all(|(_, _, total)| *total == 6),
        "six mana in every step of the turn: {seen:?}"
    );
    assert!(
        seen.iter()
            .filter(|(turn, _, _)| *turn == 2)
            .all(|(_, _, total)| *total == 0),
        "none in the next turn: {seen:?}"
    );
    let emptied: Vec<ManaLasts> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::ManaPoolEmptied { player, through } if *player == pid(0) => Some(*through),
            _ => None,
        })
        .collect();
    assert_eq!(
        emptied,
        [ManaLasts::Turn],
        "the pool empties once, with the turn"
    );
    Ok(())
}

#[rstest]
fn kept_mana_and_a_land_pay_together(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let wyrm = spell("Wyrm", "{R}{R}{R}{R}{G}{G}{G}");
    let mut driver = table(seats, "Savage Ventmaw", vec![wyrm], 1)?;
    attack(&mut driver, "Savage Ventmaw")?;
    driver.pass_until(Step::Main2)?;
    assert_eq!(pool(&driver).total(), 6);
    driver.cast("Wyrm")?;
    assert!(
        pool(&driver).is_empty(),
        "the kept mana is spent: {:?}",
        pool(&driver)
    );
    let mountain = driver.id_of("Mountain")?;
    assert!(
        driver
            .engine()
            .events()
            .iter()
            .any(|event| matches!(event, Event::PermanentTapped { object } if *object == mountain)),
        "the Mountain pays for the seventh mana"
    );
    Ok(())
}

#[rstest]
fn mana_that_goes_sooner_is_spent_before_kept_mana(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let bolt = spell("Bolt", "{R}");
    let mut driver = table(seats, "Savage Ventmaw", vec![bolt], 1)?;
    attack(&mut driver, "Savage Ventmaw")?;
    driver.pass_until(Step::Main2)?;
    driver.tap_for_mana("Mountain")?;
    assert_eq!(pool(&driver).total(), 7);
    assert_eq!(kept_turn(&pool(&driver), ManaType::Red), 3);
    driver.cast("Bolt")?;
    let after = pool(&driver);
    assert_eq!(after.total(), 6, "one red mana is spent");
    assert_eq!(
        kept_turn(&after, ManaType::Red),
        3,
        "the mana of the land went first"
    );
    assert_eq!(after.get(ManaType::Red), 3);
    Ok(())
}

#[rstest]
fn an_unspent_mana_of_the_step_goes_while_the_kept_mana_stays(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, "Savage Ventmaw", Vec::new(), 1)?;
    attack(&mut driver, "Savage Ventmaw")?;
    driver.tap_for_mana("Mountain")?;
    assert_eq!(pool(&driver).total(), 7);
    driver.pass_until(Step::CombatDamage)?;
    let held = pool(&driver);
    assert_eq!(held.total(), 6, "the mana of the land went with the step");
    assert_eq!(kept_turn(&held, ManaType::Red), 3);
    assert_eq!(held.get(ManaType::Red), 3);
    Ok(())
}

#[rstest]
#[case::klauth("Klauth, Unrivaled Ancient", 4)]
fn mana_counted_by_the_attackers_is_kept_and_for_spells_only(
    #[case] attacker: &str,
    #[case] power: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, attacker, Vec::new(), 0)?;
    attack(&mut driver, attacker)?;
    let held = pool(&driver);
    assert_eq!(held.total(), power);
    assert_eq!(
        held.restricted().len(),
        power as usize,
        "all of it is for spells"
    );
    assert!(
        held.restricted()
            .iter()
            .all(|unit| unit.lasts == ManaLasts::Turn)
    );
    let seen = walk(&mut driver)?;
    assert!(
        seen.iter()
            .filter(|(turn, step, _)| *turn == 1 && *step >= Step::DeclareBlockers)
            .all(|(_, _, total)| *total == power),
        "kept through the turn: {seen:?}"
    );
    assert!(
        seen.iter()
            .filter(|(turn, _, _)| *turn == 2)
            .all(|(_, _, total)| *total == 0)
    );
    Ok(())
}

#[rstest]
fn damage_dealt_is_the_mana_added_by_photon(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, "Photon, Mighty Marvel", Vec::new(), 0)?;
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Photon, Mighty Marvel", 1)])?;
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().turn().step() == Step::CombatDamage
            && driver.state().stack().is_empty()
            && pool(&driver).total() > 0
        {
            break;
        }
        match &decision.kind {
            DecisionKind::DeclareBlockers(_) => {
                driver.block(&[])?;
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let held = pool(&driver);
    assert_eq!(held.total(), 2, "Photon deals 2 combat damage");
    let kept: u32 = ManaType::ALL
        .iter()
        .map(|mana| kept_turn(&held, *mana))
        .sum();
    assert_eq!(kept, 2);
    let life = driver.state().player(pid(1)).unwrap().life();
    assert_eq!(life, 18);
    Ok(())
}

/// Crews the Vehicle with the first creature the activation asks for and lets the activation resolve.
fn crew(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    let offered = driver.offered_abilities(name)?;
    let [index] = offered.as_slice() else {
        return Err(ScriptError::Stuck(format!("{name} offers {offered:?}")));
    };
    driver.begin_ability(name, *index)?;
    for _ in 0..40 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseCards(options) => {
                let ids = options.from.iter().take(options.min as usize).copied();
                driver.answer(Answer::ChooseCards(CardsChoice(ids.collect())))?;
            }
            _ if driver.state().stack().is_empty() && driver.state().resolving().is_none() => {
                return Ok(());
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the crew".into()))
}

#[rstest]
fn a_crewed_plow_adds_mana_that_stays_and_gains_life(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Colossal Plow").unwrap()),
            ready(free_creature("Giant", 6, 6)),
        ],
    );
    for seat in 1..seats {
        scenario =
            scenario.battlefield(seat, [ready(free_creature(&format!("Wall {seat}"), 0, 9))]);
    }
    let mut driver = Driver::from_scenario(scenario)?;
    crew(&mut driver, "Colossal Plow")?;
    attack(&mut driver, "Colossal Plow")?;
    let held = pool(&driver);
    assert_eq!(held.total(), 3);
    assert_eq!(kept_turn(&held, ManaType::White), 3);
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), 23);
    let seen = walk(&mut driver)?;
    assert!(
        seen.iter()
            .filter(|(turn, _, _)| *turn == 1)
            .all(|(_, _, total)| *total == 3),
        "three mana in every step of the turn: {seen:?}"
    );
    assert!(
        seen.iter()
            .filter(|(turn, _, _)| *turn == 2)
            .all(|(_, _, total)| *total == 0)
    );
    Ok(())
}
