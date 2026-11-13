//! Goal: "Change the target of target spell that targets only a player. The new target must be a player." (Rebound)
//! may target only a spell whose one target is a player, offers every other player and no creature as the new target,
//! and the spell then deals its damage to the new player, at tables of two to four.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::ids::PlayerId;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// A corpus card for no mana.
fn free(name: &str) -> Arc<CardDefinition> {
    let mut definition = (*card(name).unwrap()).clone();
    definition.mana_cost = Some(ManaCost::parse("{0}").unwrap());
    Arc::new(definition)
}

/// The spell on top of the stack as a target.
fn on_stack(driver: &Driver) -> Target {
    let top = *driver.state().stack().last().unwrap();
    Target::Object(driver.state().object(top).unwrap().reference())
}

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(PlayerId(seat as u64)).unwrap().life()
}

/// Seat 0 holds a free Lightning Bolt and Rebound; seat 1 controls Grizzly Bears.
fn table(seats: usize) -> Driver {
    let scenario = Scenario::new(seats)
        .hand(0, [free("Lightning Bolt"), free("Rebound")])
        .battlefield(1, [ready(card("Grizzly Bears").unwrap())]);
    Driver::from_scenario(scenario).unwrap()
}

#[rstest]
fn the_new_target_is_another_player(#[values(2, 3, 4)] seats: usize) {
    let mut driver = table(seats);
    let aim = driver.target_player(1);
    driver.cast_at("Lightning Bolt", &[aim]).unwrap();
    let bolt = on_stack(&driver);
    driver.cast_at("Rebound", &[bolt]).unwrap();
    let new = seats - 1;
    let mut asked = false;
    for _ in 0..12 {
        let decision = driver.decision().unwrap().clone();
        match &decision.kind {
            DecisionKind::ChooseTargets(options) => {
                asked = true;
                let mut players: Vec<Target> = (0..seats)
                    .filter(|seat| *seat != 1)
                    .map(|seat| driver.target_player(seat))
                    .collect();
                players.sort_by_key(|target| format!("{target:?}"));
                let mut legal = options.legal.clone();
                legal.sort_by_key(|target| format!("{target:?}"));
                assert_eq!(legal, players, "only the other players");
                let to = if new == 1 { 0 } else { new };
                driver
                    .answer(Answer::ChooseTargets(TargetChoice::Targets(vec![
                        driver.target_player(to),
                    ])))
                    .unwrap();
            }
            DecisionKind::Priority(_) if !driver.state().stack().is_empty() => {
                driver.pass().unwrap();
            }
            _ => break,
        }
    }
    assert!(asked || seats == 2, "the new target is chosen");
    let hit = if new == 1 { 0 } else { new };
    assert_eq!(life(&driver, hit), 17);
    assert_eq!(life(&driver, 1), 20);
}

#[rstest]
fn a_spell_aimed_at_a_creature_is_no_target(#[values(2, 4)] seats: usize) {
    let mut driver = table(seats);
    let bears = driver.target_named("Grizzly Bears").unwrap();
    driver.cast_at("Lightning Bolt", &[bears]).unwrap();
    let bolt = on_stack(&driver);
    assert!(driver.cast_at("Rebound", &[bolt]).is_err());
}
