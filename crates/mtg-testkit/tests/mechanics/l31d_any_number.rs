//! Goal: "sacrifice any number of ..." and "discard any number of ..." take as many matching objects as the player picks, none included, and the steps after them count what was taken ("that many", "that much", "for each ... this way"), for Last-Ditch Effort, Renounce, Pitiless Carnage and Illuminate History at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::ObjectId;
use mtg_engine::Answer;
use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::card as corpus_card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

/// The corpus card with its mana cost made {0}.
fn free(name: &str) -> Arc<CardDefinition> {
    let mut def = (*corpus_card(name).unwrap()).clone();
    def.mana_cost = Some(mtg_core::mana::ManaCost::parse("{0}").unwrap());
    Arc::new(def)
}

fn creatures(prefix: &str, count: usize) -> Vec<Arc<CardDefinition>> {
    (0..count)
        .map(|at| free_creature(&format!("{prefix} {at}"), 1, 1))
        .collect()
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// Passes the decisions the stack asks until it is empty, picking the first `taken` objects offered.
fn settle(driver: &mut Driver, taken: usize) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(taken > 0)))?;
            }
            DecisionKind::ChooseCards(options) => {
                let picked: Vec<ObjectId> = options.from.iter().take(taken).copied().collect();
                driver.answer(Answer::ChooseCards(CardsChoice(picked)))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// A table where seat 0 holds `spell` and `held` creature cards and controls three creatures and two Forests, and the last seat a creature.
fn table(seats: usize, spell: &str, held: usize) -> Scenario {
    let mut hand = vec![free(spell)];
    hand.extend(creatures("Held", held));
    let mut field: Vec<_> = creatures("Bear", 3).into_iter().map(ready).collect();
    field.extend((0..2).map(|_| ready(forest())));
    (0..seats).fold(
        Scenario::new(seats)
            .hand(0, hand)
            .battlefield(0, field)
            .battlefield(seats - 1, [ready(free_creature("Foe", 1, 1))]),
        |scenario, seat| scenario.library(seat, vec![forest(); 6]),
    )
}

#[rstest]
fn last_ditch_effort_deals_as_much_damage_as_creatures_sacrificed(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2, 3)] taken: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, "Last-Ditch Effort", 0))?;
    driver.begin_cast("Last-Ditch Effort")?;
    let foe = driver.target_player(seats - 1);
    driver.choose_targets(&[foe])?;
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    settle(&mut driver, taken)?;
    let gone = names(driver.state(), Zone::Graveyard(pid(0)));
    assert_eq!(
        gone.iter().filter(|name| name.starts_with("Bear")).count(),
        taken,
        "{gone:?}"
    );
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Foe".to_owned()));
    assert_eq!(life(&driver, seats - 1), 20 - i32::try_from(taken).unwrap());
    Ok(())
}

#[rstest]
fn renounce_gains_two_life_for_each_permanent_sacrificed(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 4)] taken: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, "Renounce", 0))?;
    driver.cast("Renounce")?;
    settle(&mut driver, taken)?;
    let left = names(driver.state(), Zone::Battlefield);
    assert_eq!(left.len(), 6 - taken, "any permanent of yours: {left:?}");
    assert_eq!(life(&driver, 0), 20 + 2 * i32::try_from(taken).unwrap());
    Ok(())
}

#[rstest]
fn pitiless_carnage_draws_as_many_cards_as_permanents_sacrificed(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2, 5)] taken: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, "Pitiless Carnage", 0))?;
    driver.cast("Pitiless Carnage")?;
    settle(&mut driver, taken)?;
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len(), taken);
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))).len(),
        taken + 1,
        "the permanents and the spell"
    );
    Ok(())
}

#[rstest]
fn illuminate_history_draws_as_many_cards_as_it_discarded(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 1, 3)] taken: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, "Illuminate History", 3);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Illuminate History")?;
    settle(&mut driver, taken)?;
    let hand = names(driver.state(), Zone::Hand(pid(0)));
    assert_eq!(hand.len(), 3, "{hand:?}");
    assert_eq!(
        hand.iter().filter(|name| name.starts_with("Held")).count(),
        3 - taken
    );
    let graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    assert_eq!(graveyard.len(), taken + 1, "{graveyard:?}");
    Ok(())
}

#[rstest]
fn sacred_rites_gives_toughness_for_each_card_discarded_and_keeps_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(0, 2, 3)] taken: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats, "Sacred Rites", 3);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Sacred Rites")?;
    settle(&mut driver, taken)?;
    let bear = driver.id_of("Bear 0")?;
    let toughness = |driver: &Driver| {
        mtg_engine::characteristics(driver.state(), bear)
            .unwrap()
            .toughness
    };
    let expected = 1 + i32::try_from(taken).unwrap();
    assert_eq!(toughness(&driver), Some(expected));
    let foe = driver.id_of("Foe")?;
    assert_eq!(
        mtg_engine::characteristics(driver.state(), foe)
            .unwrap()
            .toughness,
        Some(1),
        "only your creatures"
    );
    Ok(())
}
