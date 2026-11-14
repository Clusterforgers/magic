//! Goal: a permanent that becomes a copy until end of turn is the copy with its exceptions and is itself again after the
//! turn, at two to four seats: Shifting Woodland copies a permanent card in its controller's graveyard only with four card
//! types there (delirium), and Mindlink Mech copies the nonlegendary creature that crewed it, as a 4/3 flying Vehicle
//! artifact, the first time it is crewed each turn; each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::{CardType, Subtype};
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, instant};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::casting::lands;

fn r6104_settle(driver: &mut Driver) {
    for _ in 0..200 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack does not empty");
}

fn r6104_replays(scenario: Scenario, driver: &Driver) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

/// The name the permanent `id` has now, and its power and toughness.
fn r6104_seen(driver: &Driver, id: mtg_core::ids::ObjectId) -> (String, Option<i32>, Option<i32>) {
    let seen = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    let name = driver.state().object(id).unwrap().card().name.clone();
    (name, seen.power, seen.toughness)
}

/// Seat 0 controls Shifting Woodland and four Forests, with Grizzly Bears and `types - 1` cards of other types in their
/// graveyard.
fn r6104_woodland(seats: usize, types: usize) -> Scenario {
    let others: Vec<Arc<CardDefinition>> = vec![
        Arc::new(CardDefinition::new("Rock").with_type(CardType::Artifact)),
        instant("Spark", 0, &[]),
        forest(),
    ];
    let mut graveyard = vec![card("Grizzly Bears").unwrap()];
    graveyard.extend(others.into_iter().take(types - 1));
    let mut board = vec![ready(card("Shifting Woodland").unwrap())];
    board.extend(lands(4, forest));
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board)
        .graveyard(0, graveyard)
}

#[rstest]
fn r6104_shifting_woodland_copies_a_graveyard_card_until_end_of_turn(
    #[values(2, 3, 4)] seats: usize,
) {
    let scenario = r6104_woodland(seats, 4);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let woodland = driver.id_of("Shifting Woodland").unwrap();
    let offered = driver.offered_abilities("Shifting Woodland").unwrap();
    assert!(offered.contains(&1), "{offered:?}");
    driver.begin_ability("Shifting Woodland", 1).unwrap();
    let graveyard = driver
        .state()
        .zone(Zone::Graveyard(crate::common::pid(0)))
        .to_vec();
    let bears = graveyard
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .find(|o| o.card().name == "Grizzly Bears")
        .map(|o| mtg_engine::choose::Target::Object(o.reference()))
        .unwrap();
    driver.choose_targets(&[bears]).unwrap();
    driver
        .answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))
        .unwrap();
    r6104_settle(&mut driver);
    assert_eq!(
        r6104_seen(&driver, woodland),
        ("Grizzly Bears".to_owned(), Some(2), Some(2))
    );
    assert_eq!(
        driver
            .state()
            .zone(Zone::Graveyard(crate::common::pid(0)))
            .len(),
        4
    );
    driver.idle_until(2, Step::Upkeep).unwrap();
    assert_eq!(r6104_seen(&driver, woodland).0, "Shifting Woodland");
    r6104_replays(scenario, &driver);
}

#[rstest]
fn r6104_shifting_woodland_needs_delirium(#[values(2, 3, 4)] seats: usize) {
    let driver = Driver::from_scenario(r6104_woodland(seats, 3)).unwrap();
    assert!(
        !driver
            .offered_abilities("Shifting Woodland")
            .unwrap()
            .contains(&1)
    );
}

/// Crews the Mech with the creature named `rider` and lets the trigger resolve.
fn r6104_crew(driver: &mut Driver, rider: &str) {
    driver.begin_activation("Mindlink Mech").unwrap();
    if matches!(
        driver.decision().unwrap().kind,
        DecisionKind::ChooseCards(_)
    ) {
        let id = driver.id_of(rider).unwrap();
        driver
            .answer(Answer::ChooseCards(CardsChoice(vec![id])))
            .unwrap();
    }
    r6104_settle(driver);
}

#[rstest]
fn r6104_mindlink_mech_copies_the_creature_that_crewed_it(#[values(2, 3, 4)] seats: usize) {
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1).battlefield(
        0,
        [
            ready(card("Mindlink Mech").unwrap()),
            ready(card("Grizzly Bears").unwrap()),
        ],
    );
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let mech = driver.id_of("Mindlink Mech").unwrap();
    r6104_crew(&mut driver, "Grizzly Bears");
    assert_eq!(
        r6104_seen(&driver, mech),
        ("Grizzly Bears".to_owned(), Some(4), Some(3))
    );
    let seen = mtg_engine::characteristics::characteristics(driver.state(), mech).unwrap();
    assert!(seen.types.contains(CardType::Artifact) && seen.types.contains(CardType::Creature));
    let values = driver.state().object(mech).unwrap().card();
    assert!(values.subtypes.contains(&Subtype("Vehicle".to_owned())));
    assert!(values.subtypes.contains(&Subtype("Bear".to_owned())));
    assert!(
        mtg_engine::keywords::has_keyword(
            driver.state(),
            mech,
            &mtg_cards::keyword::Keyword::Flying
        ),
        "it has flying"
    );
    driver.idle_until(2, Step::Upkeep).unwrap();
    assert_eq!(r6104_seen(&driver, mech).0, "Mindlink Mech");
    r6104_replays(scenario, &driver);
}
