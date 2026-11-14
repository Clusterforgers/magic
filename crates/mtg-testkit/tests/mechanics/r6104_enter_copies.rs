//! Goal: a permanent entering as a copy keeps the exceptions its card prints at two to four seats: Hulking Metamorph is
//! an artifact creature with its own power and toughness (7/7 cast for its cost, 3/3 prototyped), and Moritte of the
//! Frost is legendary and snow, and gets two +1/+1 counters and changeling only when the copied permanent is a creature;
//! declining leaves Hulking Metamorph as printed, and each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::{CardType, Supertype};
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, lands, pay};
use crate::common::pid;

/// Seat 0 holds `name` with Grizzly Bears, nine Islands and two Forests on the battlefield.
fn r6104_table(seats: usize, name: &str) -> Scenario {
    let mut board = vec![ready(card("Grizzly Bears").unwrap())];
    board.extend(lands(9, island));
    board.extend(lands(2, forest));
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [card(name).unwrap()])
        .battlefield(0, board)
}

/// Casts `name` (for `way` when a way is offered) and copies `copy`, or declines with `None`.
fn r6104_cast_copying(driver: &mut Driver, name: &str, way: &str, copy: Option<&str>) {
    driver.begin_cast(name).unwrap();
    if matches!(
        driver.decision().unwrap().kind,
        DecisionKind::ChooseOption(_)
    ) {
        choose_way(driver, way).unwrap();
    }
    for _ in 0..10 {
        if matches!(driver.decision().unwrap().kind, DecisionKind::PayMana(_)) {
            break;
        }
        driver.answer_passively().unwrap();
    }
    pay(driver).unwrap();
    for _ in 0..80 {
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::ChooseCards(options) => {
                let chosen = copy.map(|wanted| {
                    *options
                        .from
                        .iter()
                        .find(|id| driver.state().object(**id).unwrap().card().name == wanted)
                        .expect("the permanent is offered")
                });
                driver
                    .answer(Answer::ChooseCards(CardsChoice(
                        chosen.into_iter().collect(),
                    )))
                    .unwrap();
            }
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return;
            }
            _ => {
                driver.answer_passively().unwrap();
            }
        }
    }
    panic!("the spell did not resolve");
}

/// The permanent on the battlefield that is not among `before`.
fn r6104_newest(driver: &Driver, before: &[mtg_core::ids::ObjectId]) -> mtg_core::ids::ObjectId {
    *driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .find(|id| !before.contains(id))
        .unwrap()
}

fn r6104_replays(scenario: Scenario, driver: &Driver) {
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
#[case::cast_for_its_cost("Cast for", 7)]
#[case::prototyped("Prototype", 3)]
fn r6104_hulking_metamorph_keeps_its_own_size(
    #[values(2, 3, 4)] seats: usize,
    #[case] way: &str,
    #[case] size: i32,
) {
    let scenario = r6104_table(seats, "Hulking Metamorph");
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let before = driver.state().zone(Zone::Battlefield).to_vec();
    r6104_cast_copying(&mut driver, "Hulking Metamorph", way, Some("Grizzly Bears"));
    let id = r6104_newest(&driver, &before);
    let seen = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    assert_eq!(
        driver.state().object(id).unwrap().card().name,
        "Grizzly Bears"
    );
    assert!(seen.types.contains(CardType::Artifact) && seen.types.contains(CardType::Creature));
    assert_eq!((seen.power, seen.toughness), (Some(size), Some(size)));
    assert_eq!(driver.state().object(id).unwrap().controller(), pid(0));
    r6104_replays(scenario, &driver);
}

#[rstest]
#[case::a_creature("Grizzly Bears", true)]
#[case::a_land("Forest", false)]
fn r6104_moritte_adds_counters_and_changeling_only_to_a_creature(
    #[values(2, 3, 4)] seats: usize,
    #[case] copied: &str,
    #[case] creature: bool,
) {
    let scenario = r6104_table(seats, "Moritte of the Frost");
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let before = driver.state().zone(Zone::Battlefield).to_vec();
    r6104_cast_copying(&mut driver, "Moritte of the Frost", "", Some(copied));
    let id = r6104_newest(&driver, &before);
    let object = driver.state().object(id).unwrap();
    assert_eq!(object.card().name, copied);
    let seen = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    let values = object.card();
    assert!(
        values.supertypes.contains(&Supertype::Legendary),
        "{:?}",
        values.supertypes
    );
    assert!(
        values.supertypes.contains(&Supertype::Snow),
        "{:?}",
        values.supertypes
    );
    let counters = object
        .permanent()
        .unwrap()
        .counters_of(&mtg_cards::value::CounterKind::PlusOnePlusOne);
    assert_eq!(counters, if creature { 2 } else { 0 });
    assert_eq!(
        seen.all_creature_types, creature,
        "changeling only on a creature"
    );
    assert_eq!(seen.types.contains(CardType::Creature), creature);
    r6104_replays(scenario, &driver);
}

#[rstest]
fn r6104_declining_leaves_the_card_as_printed(#[values(2, 3, 4)] seats: usize) {
    let name = "Hulking Metamorph";
    let scenario = r6104_table(seats, name);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let before = driver.state().zone(Zone::Battlefield).to_vec();
    r6104_cast_copying(&mut driver, name, "Cast for", None);
    let id = r6104_newest(&driver, &before);
    let object = driver.state().object(id).unwrap();
    assert_eq!(object.card().name, name);
    let seen = mtg_engine::characteristics::characteristics(driver.state(), id).unwrap();
    assert_eq!((seen.power, seen.toughness), (Some(7), Some(7)));
    r6104_replays(scenario, &driver);
}
