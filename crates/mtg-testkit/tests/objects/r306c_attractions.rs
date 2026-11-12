//! Goal: the r30-6c Attractions play as printed at tables of two to four seats: Balloon Stand makes a Balloon or trades
//! one for flying (and gives nothing without one), Pick-a-Beeble keeps a luck counter per pip and claims its prize from
//! six on, and Hall of Mirrors turns each other creature of its controller into a nonlegendary copy until end of turn.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_cards::keyword::Keyword;
use mtg_cards::value::CounterKind;
use mtg_core::attraction::Lights;
use mtg_core::card::CardType;
use mtg_engine::Event;
use mtg_engine::choose::{ModeChoice, TargetChoice};
use mtg_engine::decision::{Answer, Decision, DecisionKind, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, free_creature, un33_opener};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 has `attraction` (lit at every number) as its whole Attraction deck, an opener of one in hand and `mine` on the
/// battlefield; it opens the Attraction in its upkeep.
fn r306c_visit_table(
    seats: usize,
    attraction: &str,
    mine: Vec<Arc<CardDefinition>>,
    seed: u64,
) -> Result<Driver, ScriptError> {
    let scenario = Scenario::new(seats)
        .seed(seed)
        .at(1, 0, Step::Upkeep)
        .attractions(0, [(card(attraction).unwrap(), Lights::of(1..=6))])
        .hand(0, [un33_opener("Open One", Some(1))])
        .battlefield(0, mine.into_iter().map(ready).collect::<Vec<_>>());
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Open One")?;
    r306c_settle(&mut driver, &mut |_, _| None)?;
    driver.pass_until(Step::Main1)?;
    Ok(driver)
}

/// Answers until seat 0 has priority with an empty stack; `custom` answers first when it returns an answer.
fn r306c_settle(
    driver: &mut Driver,
    custom: &mut dyn FnMut(&Driver, &Decision) -> Option<Answer>,
) -> Result<(), ScriptError> {
    for _ in 0..120 {
        let decision = driver.decision()?.clone();
        if let Some(answer) = custom(driver, &decision) {
            driver.answer(answer)?;
            continue;
        }
        match &decision.kind {
            DecisionKind::Priority(_)
                if decision.player == pid(0)
                    && driver.state().stack().is_empty()
                    && driver.state().pending_triggers().is_empty()
                    && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack did not empty".to_owned()))
}

fn r306c_count(driver: &Driver, name: &str) -> usize {
    names(driver.state(), Zone::Battlefield)
        .iter()
        .filter(|n| *n == name)
        .count()
}

/// A creature card of the subtype Balloon.
fn balloon() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Old Balloon")
            .with_type(CardType::Creature)
            .with_subtype("Balloon")
            .with_power_toughness(1, 1),
    )
}

/// Chooses mode `mode` and targets `target` (by name) when asked.
fn balloon_answers(
    mode: usize,
    target: &str,
) -> impl FnMut(&Driver, &Decision) -> Option<Answer> + '_ {
    move |driver, decision| match &decision.kind {
        DecisionKind::ChooseMode(_) => Some(Answer::ChooseMode(ModeChoice::Modes(vec![mode]))),
        DecisionKind::ChooseTargets(_) if decision.player == pid(0) => {
            let chosen = driver.target_named(target).ok()?;
            Some(Answer::ChooseTargets(TargetChoice::Targets(vec![chosen])))
        }
        _ => None,
    }
}

#[rstest]
fn balloon_stand_makes_a_balloon(#[values(2, 3, 4)] seats: usize) -> Result<(), ScriptError> {
    let mut driver = r306c_visit_table(seats, "Balloon Stand", vec![bear()], 1)?;
    r306c_settle(&mut driver, &mut balloon_answers(0, "Grizzly Bears"))?;
    assert_eq!(r306c_count(&driver, "Balloon"), 1);
    let id = driver.id_of("Balloon")?;
    assert!(has_keyword(driver.state(), id, &Keyword::Flying));
    Ok(())
}

#[rstest]
fn balloon_stand_trades_a_balloon_for_flying(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] has_balloon: bool,
) -> Result<(), ScriptError> {
    let mut mine = vec![bear()];
    if has_balloon {
        mine.push(balloon());
    }
    let mut driver = r306c_visit_table(seats, "Balloon Stand", mine, 1)?;
    r306c_settle(&mut driver, &mut balloon_answers(1, "Grizzly Bears"))?;
    assert_eq!(r306c_count(&driver, "Old Balloon"), 0);
    let id = driver.id_of("Grizzly Bears")?;
    assert_eq!(
        has_keyword(driver.state(), id, &Keyword::Flying),
        has_balloon
    );
    Ok(())
}

fn rolled(driver: &Driver) -> u32 {
    driver
        .engine()
        .events()
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::DieRolled { result, .. } => Some(*result),
            _ => None,
        })
        .unwrap()
}

#[rstest]
fn pick_a_beeble_counts_luck_and_claims_at_six(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3, 4, 5, 6, 7, 8)] seed: u64,
) -> Result<(), ScriptError> {
    let mut driver = r306c_visit_table(seats, "Pick-a-Beeble", Vec::new(), seed)?;
    r306c_settle(&mut driver, &mut |_, _| None)?;
    // The visit roll comes first; the luck roll is the last die rolled.
    let luck = rolled(&driver);
    if luck >= 6 {
        assert_eq!(r306c_count(&driver, "Pick-a-Beeble"), 0);
        assert_eq!(r306c_count(&driver, "Treasure"), 3);
    } else {
        let id = driver.id_of("Pick-a-Beeble")?;
        let held = driver
            .state()
            .object(id)
            .and_then(|object| object.permanent())
            .map_or(0, |p| p.counters_of(&CounterKind::Other("luck".to_owned())));
        assert_eq!(held, luck);
        assert_eq!(r306c_count(&driver, "Treasure"), 1);
    }
    Ok(())
}

#[rstest]
fn hall_of_mirrors_copies_the_target_onto_each_other_creature(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mine = vec![
        bear(),
        free_creature("Small", 1, 1),
        card("Craig Boone, Novac Guard").unwrap(),
    ];
    let mut driver = r306c_visit_table(seats, "Hall of Mirrors", mine, 1)?;
    r306c_settle(
        &mut driver,
        &mut balloon_answers(0, "Craig Boone, Novac Guard"),
    )?;
    // Three Craig Boones, two of them nonlegendary copies, so the legend rule keeps them all.
    assert_eq!(r306c_count(&driver, "Craig Boone, Novac Guard"), 3);
    Ok(())
}
