//! Goal: "you may pay {1}{R} and discard a card. If you do, this creature deals damage to target creature or planeswalker equal to the discarded card's mana value" (Wolf of Devil's Breach) deals the mana value of the card the payment discarded, as it was in the hand, and nothing when the payment is declined, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_engine::choose::{CardsChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::{name_of, pid};

/// Seat 0 has the Wolf, two Mountains and `hand`; seat 1 has a Serra Angel.
fn table(seats: usize, hand: &[&str]) -> Scenario {
    (0..seats).fold(
        Scenario::new(seats)
            .hand_named(0, hand.iter().copied())
            .battlefield(
                0,
                vec![
                    ready(card("Wolf of Devil's Breach").unwrap()),
                    ready(mountain()),
                    ready(mountain()),
                ],
            )
            .battlefield(1, vec![ready(card("Serra Angel").unwrap())]),
        |scenario, seat| scenario.library(seat, vec![forest(); 12]),
    )
}

/// Attacks seat 1 with the Wolf, targets the Angel and resolves the trigger: pays and discards `discard`, or declines.
fn attack(driver: &mut Driver, discard: Option<&str>) -> Result<(), ScriptError> {
    driver.pass_until(Step::DeclareAttackers)?;
    driver.attack(&[("Wolf of Devil's Breach", 1)])?;
    let angel = driver.target_named("Serra Angel")?;
    driver.choose_targets(&[angel])?;
    for _ in 0..40 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(discard.is_some())))?;
            }
            DecisionKind::ChooseCards(options) => {
                let wanted = discard.unwrap();
                let pick = options
                    .from
                    .iter()
                    .copied()
                    .find(|id| name_of(driver.state(), *id) == wanted)
                    .unwrap();
                driver.answer(Answer::ChooseCards(CardsChoice(vec![pick])))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => panic!("unexpected decision {decision:?}"),
        }
    }
    Ok(())
}

fn angel_damage(driver: &Driver) -> u32 {
    let angel = driver.id_of("Serra Angel").unwrap();
    driver
        .state()
        .object(angel)
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| permanent.damage())
}

#[rstest]
#[case::bolt(&["Lightning Bolt"], Some("Lightning Bolt"), 1)]
#[case::bears(&["Grizzly Bears"], Some("Grizzly Bears"), 2)]
#[case::bears_of_two(&["Lightning Bolt", "Grizzly Bears"], Some("Grizzly Bears"), 2)]
#[case::bolt_of_two(&["Lightning Bolt", "Grizzly Bears"], Some("Lightning Bolt"), 1)]
#[case::declined(&["Grizzly Bears"], None, 0)]
fn the_wolf_deals_the_discarded_cards_mana_value(
    #[values(2, 3, 4)] seats: usize,
    #[case] hand: &[&str],
    #[case] discard: Option<&str>,
    #[case] damage: u32,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, hand))?;
    attack(&mut driver, discard)?;
    assert_eq!(angel_damage(&driver), damage);
    let kept = driver.state().zone(Zone::Hand(pid(0))).len();
    assert_eq!(kept, hand.len() - usize::from(discard.is_some()));
    Ok(())
}
