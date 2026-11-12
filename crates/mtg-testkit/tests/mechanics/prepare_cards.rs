//! Goal: the prepare cards of the corpus work as printed at two to four seats: Eiganjo Dynastorian becomes prepared when its controller attacks with two or more creatures (not with one) and casts Replenish from the battlefield, Adventurous Eater and Goblin Glasswright enter prepared and cast Have a Bite and Craft with Pride, and the opponents of the controller are never offered the spell.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::ObjectId;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_engine::object::PermanentState;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{names, pid};

const DYNASTORIAN: &str = "Eiganjo Dynastorian // Replenish";
const EATER: &str = "Adventurous Eater // Have a Bite";
const GLASSWRIGHT: &str = "Goblin Glasswright // Craft with Pride";

fn table(seats: usize) -> Scenario {
    let mut scenario = Scenario::new(seats);
    for seat in 0..seats {
        scenario = scenario.library(seat, vec![forest(); 12]);
    }
    scenario
}

fn prepared(driver: &Driver, name: &str) -> bool {
    let id: ObjectId = driver.id_of(name).unwrap();
    driver
        .state()
        .object(id)
        .and_then(|object| object.permanent())
        .is_some_and(PermanentState::prepared)
}

/// Every seat passes once: the top of the stack resolves.
fn resolve_top(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    for _ in 0..seats {
        driver.pass()?;
    }
    Ok(())
}

/// Seat 0 attacks with `attackers` of the Dynastorian and two Bears in the first combat; the trigger is on the stack.
fn attacked(seats: usize, attackers: &[&str]) -> Result<Driver, ScriptError> {
    let scenario = table(seats)
        .at(1, 0, Step::Main1)
        .battlefield_named(0, [DYNASTORIAN, "Grizzly Bears", "Grizzly Bears"])
        .graveyard_named(0, ["Glorious Anthem"]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(1, Step::DeclareAttackers)?;
    let attacks: Vec<(&str, usize)> = attackers.iter().map(|name| (*name, 1)).collect();
    driver.attack(&attacks)?;
    Ok(driver)
}

#[rstest]
fn attacking_with_two_creatures_prepares_the_dynastorian_and_attacking_with_one_does_not(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3)] attackers: usize,
) -> Result<(), ScriptError> {
    let names = ["Eiganjo Dynastorian", "Grizzly Bears", "Grizzly Bears"];
    let mut driver = attacked(seats, &names[..attackers])?;
    resolve_top(&mut driver, seats)?;
    assert_eq!(prepared(&driver, "Eiganjo Dynastorian"), attackers >= 2);
    Ok(())
}

#[rstest]
fn the_prepared_dynastorian_casts_replenish_from_the_battlefield_and_returns_the_enchantments(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats)
        .at(1, 0, Step::Main1)
        .battlefield(
            0,
            [DYNASTORIAN, "Grizzly Bears", "Grizzly Bears"]
                .into_iter()
                .map(|name| ready(card(name).unwrap()))
                .chain((0..4).map(|_| ready(plains()))),
        )
        .graveyard_named(0, ["Glorious Anthem"]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("Eiganjo Dynastorian", 1), ("Grizzly Bears", 1)])?;
    resolve_top(&mut driver, seats)?;
    driver.idle_until(1, Step::Main2)?;
    assert!(prepared(&driver, "Eiganjo Dynastorian"));
    driver.cast_prepared("Eiganjo Dynastorian")?;
    assert_eq!(names(driver.state(), Zone::Stack), ["Replenish"]);
    resolve_top(&mut driver, seats)?;
    assert!(names(driver.state(), Zone::Battlefield).contains(&"Glorious Anthem".to_owned()));
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        Vec::<String>::new()
    );
    assert!(!prepared(&driver, "Eiganjo Dynastorian"));
    Ok(())
}

#[rstest]
fn a_card_that_enters_prepared_casts_its_spell_for_the_cost_of_the_spell(
    #[values(2, 3, 4)] seats: usize,
    #[values(EATER, GLASSWRIGHT)] card: &str,
) -> Result<(), ScriptError> {
    let front = card.split(" // ").next().unwrap();
    let scenario = table(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, [card])
        .battlefield(
            0,
            [
                ready(swamp()),
                ready(swamp()),
                ready(swamp()),
                ready(mtg_testkit::cards::mountain()),
                ready(mtg_testkit::cards::mountain()),
                ready(mtg_testkit::cards::mountain()),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast(front)?;
    resolve_top(&mut driver, seats)?;
    assert!(prepared(&driver, front));
    let targets = card == EATER;
    driver.begin_prepared(front)?;
    if targets {
        let target = driver.target_named(front)?;
        driver.choose_targets(&[target])?;
    }
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    resolve_top(&mut driver, seats)?;
    assert!(!prepared(&driver, front));
    let life = driver.state().player(pid(0)).unwrap().life();
    let treasure = names(driver.state(), Zone::Battlefield).contains(&"Treasure".to_owned());
    assert_eq!(life, if targets { 21 } else { 20 });
    assert_eq!(treasure, !targets);
    Ok(())
}

#[rstest]
fn nobody_but_the_controller_is_offered_the_spell(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = table(seats)
        .at(1, 0, Step::Main1)
        .hand_named(0, [EATER])
        .battlefield(0, (0..3).map(|_| ready(swamp())));
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Adventurous Eater")?;
    resolve_top(&mut driver, seats)?;
    let eater = driver.id_of("Adventurous Eater")?;
    driver.pass()?;
    for _ in 1..seats {
        let decision = driver.decision()?.clone();
        let DecisionKind::Priority(options) = &decision.kind else {
            break;
        };
        assert_ne!(decision.player, pid(0));
        assert!(options.faces.iter().all(|face| face.card != eater));
        assert!(
            driver
                .answer(Answer::Priority(PriorityAction::CastPrepared(eater)))
                .is_err()
        );
        driver.pass()?;
    }
    Ok(())
}
