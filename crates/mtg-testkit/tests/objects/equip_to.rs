//! Goal: an equip ability with a restriction ("Equip Knight {1}", "Equip legendary creature {3}") offers only the creatures its controller controls that fit it, next to the plain equip of the same Equipment that offers every creature they control, attaches to the one chosen and gives it the bonus, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::Supertype;
use mtg_engine::characteristics;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice, PriorityAction};
use mtg_testkit::cards::{free_creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::name_of;

/// A creature of the given name with a creature type, or legendary.
fn knight(name: &str) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::clone(&free_creature(name, 2, 2)).with_subtype("Knight"))
}

fn legend(name: &str) -> Arc<CardDefinition> {
    Arc::new(CardDefinition::clone(&free_creature(name, 2, 2)).with_supertype(Supertype::Legendary))
}

/// Seat 0 controls the Equipment, a Knight, a legend, a Bear and `mountains` Mountains; seat 1 a Knight and a legend.
fn table(seats: usize, equipment: &str, mountains: usize) -> Scenario {
    let mut mine = vec![
        ready(card(equipment).unwrap()),
        ready(knight("Sir Knight")),
        ready(legend("Legend")),
        ready(free_creature("Grizzly Bears", 2, 2)),
    ];
    mine.extend((0..mountains).map(|_| ready(mountain())));
    Scenario::new(seats)
        .battlefield(0, mine)
        .battlefield(1, [ready(knight("Rival Knight")), ready(legend("Rival"))])
}

/// Starts the `nth` ability of the named permanent that its controller may activate now, in the order of the card.
fn activate(driver: &mut Driver, name: &str, nth: usize) -> Result<(), ScriptError> {
    let DecisionKind::Priority(options) = driver.decision()?.kind.clone() else {
        panic!("a player has priority");
    };
    let found = options
        .abilities
        .iter()
        .filter(|a| name_of(driver.state(), a.object) == name)
        .nth(nth)
        .copied()
        .expect("the ability is offered");
    driver.answer(Answer::Priority(PriorityAction::Activate(found)))?;
    Ok(())
}

/// The names of the creatures the pending target choice offers, sorted.
fn offered(driver: &Driver) -> Vec<String> {
    let DecisionKind::ChooseTargets(options) = driver.decision().unwrap().kind.clone() else {
        panic!("an equip ability asks for its target");
    };
    let mut names: Vec<String> = options
        .legal
        .iter()
        .map(|target| name_of(driver.state(), target.object().unwrap().id))
        .collect();
    names.sort();
    names
}

#[rstest]
#[case::knights("Steelclaw Lance", 0, &["Sir Knight"])]
#[case::every_creature_by_the_plain_equip("Steelclaw Lance", 1, &["Grizzly Bears", "Legend", "Sir Knight"])]
#[case::legendary_creatures("Blackblade Reforged", 0, &["Legend"])]
#[case::every_creature_by_the_plain_equip_of_the_blade("Blackblade Reforged", 1, &["Grizzly Bears", "Legend", "Sir Knight"])]
fn the_equip_abilities_offer_the_creatures_their_restriction_allows(
    #[case] equipment: &str,
    #[case] ability: usize,
    #[case] expected: &[&str],
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, equipment, 8))?;
    activate(&mut driver, equipment, ability)?;
    assert_eq!(offered(&driver), expected);
    Ok(())
}

#[rstest]
#[case::knights("Steelclaw Lance", "Sir Knight", (4, 4))]
#[case::legendary_creatures("Blackblade Reforged", "Legend", (10, 10))]
fn the_restricted_equip_attaches_to_the_chosen_creature_and_gives_the_bonus(
    #[case] equipment: &str,
    #[case] creature: &str,
    #[case] stats: (i32, i32),
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, equipment, 8))?;
    activate(&mut driver, equipment, 0)?;
    let chosen = driver.target_named(creature)?;
    driver.choose_targets(&[chosen])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    let host = driver
        .state()
        .host_of(driver.id_of(equipment)?)
        .map(|host| name_of(driver.state(), host));
    assert_eq!(host.as_deref(), Some(creature));
    let chars = characteristics(driver.state(), driver.id_of(creature)?).unwrap();
    assert_eq!((chars.power.unwrap(), chars.toughness.unwrap()), stats);
    Ok(())
}
