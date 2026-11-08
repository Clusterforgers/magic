//! Goal: `Ref::OrController` names the player a target is, or the controller of the permanent it is ("target player or planeswalker ... that player or that planeswalker's controller"), at tables of two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, mountain, walker};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

const FIRE: &str = r#"{"Spell":{"targets":[{"what":{"Either":["Player",{"Object":[{"Type":"Planeswalker"}]}]}}],"effects":[{"DealDamage":{"to":{"OrController":{"Target":0}},"amount":3}}]}}"#;

/// A sorcery: "Deal 3 damage to the player or the planeswalker's controller."
fn fire() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Fire")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(serde_json::from_str::<Ability>(FIRE).unwrap()),
    )
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// Seat 0 casts Fire at seat `victim` (a player) or at the planeswalker of seat `victim`.
#[rstest]
fn the_damage_goes_to_the_player_or_to_the_controller_of_the_planeswalker(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] at_the_walker: bool,
) -> Result<(), ScriptError> {
    let victim = seats - 1;
    let mut scenario = Scenario::new(seats)
        .battlefield(0, vec![ready(mountain())])
        .hand(0, vec![fire()])
        .at(1, 0, Step::Main1);
    scenario = scenario.battlefield(victim, vec![ready(walker("Jace", 3))]);
    let mut driver = Driver::from_scenario(scenario)?;
    let before: Vec<i32> = (0..seats).map(|seat| life(&driver, seat)).collect();
    let target = if at_the_walker {
        driver.target_named("Jace")?
    } else {
        driver.target_player(victim)
    };
    driver.cast_at("Fire", &[target])?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.answer_passively()?;
    }
    for (seat, was) in before.iter().enumerate() {
        let lost = was - life(&driver, seat);
        assert_eq!(lost, if seat == victim { 3 } else { 0 }, "seat {seat}");
    }
    Ok(())
}
