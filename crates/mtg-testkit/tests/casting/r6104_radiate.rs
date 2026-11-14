//! Goal: Radiate copies an instant or sorcery spell with a single target once for each other permanent and each other
//! player that spell could target, each copy aimed at a different one and never at a permanent it could not target (a
//! land), whether the original aims at a creature or a player; at two to four seats, and each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::common::{pid, spell};

/// "Ping deals 1 damage to any target."
fn r6104_ping() -> std::sync::Arc<mtg_cards::CardDefinition> {
    spell(
        "Ping",
        CardType::Instant,
        r#"[{"what":"AnyTarget"}]"#,
        r#"[{"DealDamage":{"to":{"Target":0},"amount":1}}]"#,
    )
}

/// The damage marked on each creature on the battlefield, in battlefield order.
fn r6104_damage(driver: &Driver) -> Vec<u32> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|o| o.card().name == "Bear")
        .map(|o| o.permanent().unwrap().damage())
        .collect()
}

#[rstest]
fn r6104_radiate_copies_for_each_other_permanent_and_player(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] at_a_creature: bool,
) {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, vec![r6104_ping(), card("Radiate").unwrap()])
        .battlefield(
            0,
            (0..5)
                .map(|_| ready(mountain()))
                .chain([ready(creature("Bear", 1, &[], 2, 3))]),
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(creature("Bear", 1, &[], 2, 3))]);
    }
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let aim = if at_a_creature {
        driver.target_named("Bear").unwrap()
    } else {
        driver.target_player(1)
    };
    driver.cast_at("Ping", &[aim]).unwrap();
    let on_stack = *driver.state().stack().last().unwrap();
    let ping =
        mtg_engine::choose::Target::Object(driver.state().object(on_stack).unwrap().reference());
    driver.cast_at("Radiate", &[ping]).unwrap();
    for _ in 0..300 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively().unwrap();
    }
    // Every player and every creature is dealt 1 damage: the original's target by the original, the rest by a copy each.
    for seat in 0..seats {
        assert_eq!(
            driver.state().player(pid(seat)).unwrap().life(),
            19,
            "seat {seat}"
        );
    }
    assert_eq!(r6104_damage(&driver), vec![1; seats]);
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
