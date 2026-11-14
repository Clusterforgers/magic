//! Goal: Zygon Infiltrator taps another creature, puts a stun counter on it and is a copy of it for as long as that
//! creature remains tapped: the stun counter keeps it tapped through the first untap (a step or an effect, CR 122.1d), and
//! the copy ends when it untaps or leaves the battlefield; at two to four seats, and each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::CardType;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::island;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::spell;

const ZYGON: &str = "Zygon Infiltrator";

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

/// Seat 0 controls the Infiltrator, Grizzly Bears and three Islands, and holds two instants that untap a creature and
/// one that destroys it.
fn r6104_table(seats: usize) -> Scenario {
    let mut board = vec![
        ready(card(ZYGON).unwrap()),
        ready(card("Grizzly Bears").unwrap()),
    ];
    board.extend(lands(3, island));
    Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(0, board)
        .hand(
            0,
            ["Wake", "Wake Again", "Kill"]
                .iter()
                .map(|name| {
                    let effect = if *name == "Kill" {
                        r#"[{"Destroy":{"what":{"Target":0}}}]"#
                    } else {
                        r#"[{"Untap":{"what":{"Target":0}}}]"#
                    };
                    spell(
                        name,
                        CardType::Instant,
                        r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
                        effect,
                    )
                })
                .collect::<Vec<_>>(),
        )
}

/// The name the Infiltrator shows now.
fn r6104_name(driver: &Driver, id: mtg_core::ids::ObjectId) -> String {
    driver.state().object(id).unwrap().card().name.clone()
}

fn r6104_print(driver: &mut Driver) -> (mtg_core::ids::ObjectId, mtg_core::ids::ObjectId) {
    let zygon = driver.id_of(ZYGON).unwrap();
    let bears = driver.id_of("Grizzly Bears").unwrap();
    driver.begin_activation(ZYGON).unwrap();
    let at = driver.target_named("Grizzly Bears").unwrap();
    driver.choose_targets(&[at]).unwrap();
    if matches!(driver.decision().unwrap().kind, DecisionKind::PayMana(_)) {
        driver.answer(Answer::PayMana(PayChoice::Auto)).unwrap();
    }
    r6104_settle(driver);
    (zygon, bears)
}

fn r6104_tapped(driver: &Driver, id: mtg_core::ids::ObjectId) -> bool {
    driver
        .state()
        .object(id)
        .and_then(|o| o.permanent())
        .is_some_and(mtg_engine::object::PermanentState::tapped)
}

#[rstest]
fn r6104_the_copy_lasts_while_the_creature_stays_tapped(#[values(2, 3, 4)] seats: usize) {
    let scenario = r6104_table(seats);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let (zygon, bears) = r6104_print(&mut driver);
    assert!(r6104_tapped(&driver, bears));
    let stun = mtg_cards::value::CounterKind::Other("stun".to_owned());
    let counters = |driver: &Driver| {
        driver
            .state()
            .object(bears)
            .unwrap()
            .permanent()
            .unwrap()
            .counters_of(&stun)
    };
    assert_eq!(counters(&driver), 1);
    assert_eq!(r6104_name(&driver, zygon), "Grizzly Bears");
    // The next untap step removes the stun counter instead of untapping the Bears: the copy goes on.
    let next = u32::try_from(1 + seats).unwrap();
    driver.idle_until(next, Step::Upkeep).unwrap();
    assert!(r6104_tapped(&driver, bears));
    assert_eq!(counters(&driver), 0);
    assert_eq!(r6104_name(&driver, zygon), "Grizzly Bears");
    // The one after untaps the Bears, and the Infiltrator is itself again.
    driver
        .idle_until(next + u32::try_from(seats).unwrap(), Step::Upkeep)
        .unwrap();
    assert!(!r6104_tapped(&driver, bears));
    assert_eq!(r6104_name(&driver, zygon), ZYGON);
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}

#[rstest]
#[case::untapped(&["Wake", "Wake Again"])]
#[case::destroyed(&["Kill"])]
fn r6104_the_copy_ends_when_the_creature_untaps_or_leaves(
    #[values(2, 3, 4)] seats: usize,
    #[case] spells: &[&str],
) {
    let scenario = r6104_table(seats);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    let (zygon, bears) = r6104_print(&mut driver);
    for (n, name) in spells.iter().enumerate() {
        // The first untap only removes the stun counter (CR 122.1d): the Bears stay tapped and the copy goes on.
        if n == 1 {
            assert!(r6104_tapped(&driver, bears));
            assert_eq!(r6104_name(&driver, zygon), "Grizzly Bears");
        }
        // The Infiltrator is named Grizzly Bears too: the Bears are named by their object.
        let at =
            mtg_engine::choose::Target::Object(driver.state().object(bears).unwrap().reference());
        driver.cast_at(name, &[at]).unwrap();
        r6104_settle(&mut driver);
    }
    assert!(!r6104_tapped(&driver, bears));
    assert_eq!(r6104_name(&driver, zygon), ZYGON);
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
