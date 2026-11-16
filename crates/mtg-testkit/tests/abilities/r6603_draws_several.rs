//! Goal: Alms Collector replaces an opponent's draw of two or more cards at once, whole, with one card for its
//! controller and one for that player (`ReplaceWhen::R6603DrawsSeveral`); a draw of one card and its controller's own
//! draws are left alone; at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::pid;
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A free sorcery: "Each player draws `count` cards." with `each`, else "You draw `count` cards."
fn draws(count: u32, each: bool) -> Arc<CardDefinition> {
    let who = if each { r#""who":"EachPlayer","# } else { "" };
    let rules: Ability = serde_json::from_str(&format!(
        r#"{{"Spell":{{"effects":[{{"Draw":{{{who}"count":{count}}}}}]}}}}"#
    ))
    .unwrap();
    Arc::new(
        CardDefinition::new("Study")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rules),
    )
}

fn hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        let priority = matches!(driver.decision()?.kind, DecisionKind::Priority(_));
        if priority && driver.state().stack().is_empty() {
            return Ok(());
        }
        if priority {
            driver.pass()?;
        } else {
            driver.answer_passively()?;
        }
    }
    Ok(())
}

/// Seat `caster` casts a Study; seat 0 has Alms Collector. The hand sizes of seat 0 and seat 1 are counted after it.
#[rstest]
#[case::an_opponent_draws_two(1, 2, false, (1, 1))]
#[case::an_opponent_draws_three(1, 3, false, (1, 1))]
#[case::an_opponent_draws_one(1, 1, false, (0, 1))]
#[case::its_controller_draws_two(0, 2, false, (2, 0))]
#[case::each_player_draws_two(1, 2, true, (2 + 1, 1))]
fn r6603_a_draw_of_several_is_replaced_whole(
    #[values(2, 3, 4)] seats: usize,
    #[case] caster: usize,
    #[case] count: u32,
    #[case] each: bool,
    #[case] expected: (usize, usize),
) -> Result<(), ScriptError> {
    let libraries = (0..seats).fold(
        Scenario::new(seats)
            .at(1, caster, Step::Main1)
            .battlefield(0, [ready(card("Alms Collector").unwrap())])
            .hand(caster, [draws(count, each)]),
        |scenario, seat| scenario.library(seat, (0..10).map(|_| bear())),
    );
    let mut driver = Driver::from_scenario(libraries)?;
    driver.cast("Study")?;
    settle(&mut driver)?;
    // With "each player", seat 0 draws its own two and one from each opponent's replaced draw.
    let expected = if each { (2 + (seats - 1), 1) } else { expected };
    assert_eq!((hand(&driver, 0), hand(&driver, 1)), expected);
    Ok(())
}
