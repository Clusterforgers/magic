//! Goal: an alternative cost that exiles a card chosen by X ("You may exile a red card with mana value X from your hand rather than pay this spell's mana cost.", Blazing Shoal) asks X first, offers only the values a card of the color in hand has, exiles that card and resolves with that X, at tables of two to four seats; a card of another color or the spell itself never counts.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::NumberChoice;
use mtg_engine::choose::{CardsChoice, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn spell(name: &str, cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

/// Seat 0 with Blazing Shoal and the cards given in hand, a Bear on the battlefield and no lands.
fn table(seats: usize, hand: Vec<Arc<CardDefinition>>) -> Scenario {
    let mut scenario = Scenario::new(seats)
        .hand(
            0,
            std::iter::once(card("Blazing Shoal").unwrap()).chain(hand),
        )
        .battlefield(0, [ready(free_creature("Bear", 2, 2))]);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    scenario
}

/// The values of X offered: min, max and the values left out between.
type Offered = (u32, Option<u32>, Vec<u32>);

/// Casts Blazing Shoal for its alternative cost with X = `x` on the Bear; what X offered.
fn cast(driver: &mut Driver, x: u32) -> Result<Option<Offered>, ScriptError> {
    driver.begin_cast("Blazing Shoal")?;
    let mut offered = None;
    for _ in 0..12 {
        let Ok(decision) = driver.decision() else {
            break;
        };
        match &decision.kind.clone() {
            DecisionKind::ChooseNumber(options) => {
                offered = Some((options.min, options.max, options.except.clone()));
                driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
            }
            DecisionKind::ChooseTargets(_) => {
                let bear = driver.target_named("Bear")?;
                driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![bear])))?;
            }
            DecisionKind::ChooseCards(options) => {
                let first = options.from[..1].to_vec();
                driver.answer(Answer::ChooseCards(CardsChoice(first)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::Priority(_) => break,
            other => panic!("unexpected question {}", other.name()),
        }
    }
    Ok(offered)
}

fn bear_power(driver: &Driver) -> Option<i32> {
    mtg_engine::characteristics::characteristics(driver.state(), driver.id_of("Bear").ok()?)?.power
}

#[rstest]
#[case::two_seats(2, 3)]
#[case::three_seats(3, 1)]
#[case::four_seats(4, 3)]
fn the_shoal_exiles_a_red_card_of_mana_value_x(
    #[case] seats: usize,
    #[case] x: u32,
) -> Result<(), ScriptError> {
    let hand = vec![
        spell("Red One", "{R}"),
        spell("Red Three", "{2}{R}"),
        spell("Green Two", "{1}{G}"),
    ];
    let mut driver = Driver::from_scenario(table(seats, hand))?;
    let offered = cast(&mut driver, x)?;
    // Green Two counts for nothing: a red card has mana value 1 or 3, never 2.
    assert_eq!(offered, Some((1, Some(3), vec![2])));
    let exiled = if x == 1 { "Red One" } else { "Red Three" };
    assert_eq!(names(driver.state(), Zone::Exile), [exiled]);
    for _ in 0..20 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.answer_passively()?;
    }
    assert_eq!(bear_power(&driver), Some(2 + i32::try_from(x).unwrap()));
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Blazing Shoal".to_owned()));
    Ok(())
}

#[test]
fn without_a_red_card_the_shoal_is_not_cast_for_free() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, vec![spell("Green Two", "{1}{G}")]))?;
    assert!(driver.begin_cast("Blazing Shoal").is_err());
    Ok(())
}
