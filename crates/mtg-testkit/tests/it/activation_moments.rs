//! Goal: an ability that may be activated only in a moment of the turn is offered at that moment and at no other, to its controller alone, at tables of two, three and four seats: Eternal Dragon from the graveyard only during its controller's upkeep, Hua Tuo only during its controller's turn before attackers are declared, Desert only during the end of combat step and a source of "during combat" only in combat; Tomb Tyrant only during its controller's turn and with three Zombie creature cards in the graveyard, and what it returns at random is a Zombie; every game replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::{names, pid};
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Answer, Event, replay};
use mtg_testkit::cards::{forest, plains, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::restriction_cards::moment_source;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// The name of the card an object is.
fn names_of(state: &mtg_engine::GameState, id: mtg_core::ids::ObjectId) -> String {
    common::name_of(state, id)
}

/// Turns played in a sweep: two rounds of the largest table.
const TURNS: u32 = 8;

/// Every seat has a library to draw from.
fn libraries(scenario: Scenario, seats: usize) -> Scenario {
    (0..seats).fold(scenario, |scenario, seat| {
        scenario.library(seat, (0..30).map(|_| forest()))
    })
}

/// One record for each time seat 0 had priority during `turns` turns: the step, whether it was seat 0's turn and whether
/// the named card offered an ability.
fn offers(
    mut driver: Driver,
    name: &str,
    turns: u32,
) -> Result<Vec<(Step, bool, bool)>, ScriptError> {
    let mut seen = Vec::new();
    for _ in 0..4000 {
        if driver.state().turn().number() > turns + 1 {
            break;
        }
        let decision = driver.decision()?.clone();
        if decision.player == pid(0) && matches!(decision.kind, DecisionKind::Priority(_)) {
            let mine = driver.state().turn().active() == pid(0);
            let offered = !driver.offered_abilities(name)?.is_empty();
            seen.push((driver.state().turn().step(), mine, offered));
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    Ok(seen)
}

/// The step at which the ability is offered, for each of the moments the cards of the table name.
fn eternal_dragon(step: Step, mine: bool) -> bool {
    mine && step == Step::Upkeep
}

fn hua_tuo(step: Step, mine: bool) -> bool {
    mine && step <= Step::BeginCombat
}

fn combat_source(step: Step, _mine: bool) -> bool {
    matches!(
        step,
        Step::BeginCombat
            | Step::DeclareAttackers
            | Step::DeclareBlockers
            | Step::FirstStrikeDamage
            | Step::CombatDamage
            | Step::EndCombat
    )
}

#[rstest]
fn eternal_dragon_returns_only_during_its_controllers_upkeep(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = libraries(
        Scenario::new(seats)
            .at(2, 0, Step::Untap)
            .graveyard_named(0, ["Eternal Dragon"])
            .battlefield(0, (0..5).map(|_| ready(plains()))),
        seats,
    );
    let seen = offers(Driver::from_scenario(scenario)?, "Eternal Dragon", TURNS)?;
    assert!(
        seen.iter().any(|(_, mine, _)| !mine),
        "an opponent's turn was seen"
    );
    assert!(
        seen.iter()
            .any(|(step, mine, _)| eternal_dragon(*step, *mine))
    );
    for (step, mine, offered) in seen {
        assert_eq!(offered, eternal_dragon(step, mine), "{step:?} mine={mine}");
    }
    Ok(())
}

#[rstest]
fn hua_tuo_works_only_in_its_controllers_turn_before_attackers(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = libraries(
        Scenario::new(seats)
            .at(2, 0, Step::Untap)
            .battlefield_named(0, ["Hua Tuo, Honored Physician"])
            .graveyard_named(0, ["Tomb Tyrant"]),
        seats,
    );
    let seen = offers(
        Driver::from_scenario(scenario)?,
        "Hua Tuo, Honored Physician",
        TURNS,
    )?;
    assert!(
        seen.iter()
            .any(|(step, mine, _)| *mine && *step > Step::BeginCombat)
    );
    for (step, mine, offered) in seen {
        assert_eq!(offered, hua_tuo(step, mine), "{step:?} mine={mine}");
    }
    Ok(())
}

#[rstest]
fn a_source_of_combat_is_offered_in_the_combat_phase_of_every_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let source = moment_source("Combat Source", r#""InCombat""#).unwrap();
    let scenario = libraries(
        Scenario::new(seats)
            .at(2, 0, Step::Untap)
            .battlefield(0, [ready(source)]),
        seats,
    );
    let seen = offers(Driver::from_scenario(scenario)?, "Combat Source", TURNS)?;
    assert!(
        seen.iter()
            .any(|(step, _, offered)| *offered && *step == Step::BeginCombat)
    );
    for (step, mine, offered) in seen {
        assert_eq!(offered, combat_source(step, mine), "{step:?} mine={mine}");
    }
    Ok(())
}

#[rstest]
fn desert_pings_only_in_the_end_of_combat_step(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = libraries(
        Scenario::new(seats)
            .at(2, 0, Step::Main1)
            .battlefield_named(0, ["Grizzly Bears"])
            .battlefield_named(1, ["Desert"]),
        seats,
    );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.idle_until(2, Step::BeginCombat)?;
    while matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
        driver.pass()?;
    }
    driver.attack(&[("Grizzly Bears", 1)])?;
    let mut seen = Vec::new();
    for _ in 0..200 {
        if driver.state().turn().step() > Step::EndCombat || driver.state().turn().number() > 2 {
            break;
        }
        let decision = driver.decision()?.clone();
        if decision.player == pid(1) && matches!(decision.kind, DecisionKind::Priority(_)) {
            let offered = !driver.offered_abilities("Desert")?.is_empty();
            seen.push((driver.state().turn().step(), offered));
        }
        driver.answer(Driver::passive_answer(&decision))?;
    }
    assert!(
        seen.iter().any(|(step, _)| *step == Step::EndCombat),
        "{seen:?}"
    );
    for (step, offered) in seen {
        assert_eq!(offered, step == Step::EndCombat, "{step:?}");
    }
    Ok(())
}

/// Seat 0 controls Tomb Tyrant, three lands and a creature to sacrifice, with `zombies` Zombie creature cards and one
/// other creature card in the graveyard.
fn tyrant_table(seats: usize, seed: u64, active: usize, zombies: usize) -> Scenario {
    let names: Vec<&str> = std::iter::repeat_n("Dregscape Zombie", zombies)
        .chain(["Grizzly Bears"])
        .collect();
    libraries(
        Scenario::new(seats)
            .seed(seed)
            .at(2, active, Step::Main1)
            .battlefield(
                0,
                ["Tomb Tyrant", "Black Knight"]
                    .map(|name| ready(card(name).unwrap()))
                    .into_iter()
                    .chain((0..3).map(|_| ready(swamp()))),
            )
            .graveyard_named(0, names),
        seats,
    )
}

#[rstest]
#[case::three_zombies_on_my_turn(0, 3, true)]
#[case::four_zombies_on_my_turn(0, 4, true)]
#[case::two_zombies_on_my_turn(0, 2, false)]
#[case::no_zombies_on_my_turn(0, 0, false)]
#[case::three_zombies_on_an_opponents_turn(1, 3, false)]
fn tomb_tyrant_needs_my_turn_and_three_zombie_cards(
    #[values(2, 3, 4)] seats: usize,
    #[case] active: usize,
    #[case] zombies: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(tyrant_table(seats, 5, active, zombies))?;
    driver.idle_until(2, Step::Main1)?;
    while driver.decision()?.player != pid(0) {
        driver.pass()?;
    }
    let abilities = driver.offered_abilities("Tomb Tyrant")?;
    assert_eq!(!abilities.is_empty(), offered, "{abilities:?}");
    Ok(())
}

#[rstest]
fn what_tomb_tyrant_returns_is_a_zombie_picked_from_the_seeded_source(
    #[values(2, 3, 4)] seats: usize,
    #[values(1, 2, 3, 4, 5, 6)] seed: u64,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(tyrant_table(seats, seed, 0, 3))?;
    driver.idle_until(2, Step::Main1)?;
    driver.begin_ability("Tomb Tyrant", 1)?;
    // The creature to sacrifice is Black Knight, then the mana is paid.
    for _ in 0..30 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseCards(options) => {
                let knight = *options
                    .from
                    .iter()
                    .find(|id| names_of(driver.state(), **id) == "Black Knight")
                    .expect("the knight is offered");
                driver.answer(Answer::ChooseCards(CardsChoice(vec![knight])))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.pass()?;
            }
        }
    }
    let battlefield = names(driver.state(), Zone::Battlefield);
    assert!(
        battlefield.contains(&"Dregscape Zombie".to_owned()),
        "{battlefield:?}"
    );
    assert!(
        !battlefield.contains(&"Grizzly Bears".to_owned()),
        "{battlefield:?}"
    );
    let picks = driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::ChosenAtRandom { .. }))
        .count();
    assert_eq!(picks, 1, "one public pick");
    let again = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(again.state_hash(), driver.state().state_hash());
    // The card used above is the real one.
    assert!(card("Tomb Tyrant").is_ok());
    Ok(())
}
