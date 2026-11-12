//! Goal: "Spend only black mana on X" (`CostItem::XColor`) offers an X of at most the mana of that color left after the rest of the cost, asks that color for each 1 of X, and the X paid is the X the effects read, for an activated ability (Crypt Rats, Crimson Hellkite) and a spell (Consume Spirit), at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::color::Color;
use mtg_engine::Answer;
use mtg_engine::PresetPermanent;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{mountain, swamp};
use mtg_testkit::corpus::card as corpus_card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn life(driver: &Driver, seat: usize) -> i32 {
    driver.state().player(pid(seat)).unwrap().life()
}

/// `swamps` Swamps and `mountains` Mountains, untapped.
fn lands(swamps: usize, mountains: usize) -> Vec<PresetPermanent> {
    (0..swamps)
        .map(|_| ready(swamp()))
        .chain((0..mountains).map(|_| ready(mountain())))
        .collect()
}

/// The most X the pending question offers, or `None` when the next question is not about X.
fn most_x(driver: &Driver) -> Option<u32> {
    match &driver.decision().unwrap().kind {
        DecisionKind::ChooseNumber(options) => options.max,
        _ => None,
    }
}

/// Answers passively until nothing is on the stack.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..200 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn scenario(seats: usize, battlefield: Vec<PresetPermanent>) -> Scenario {
    (0..seats).fold(
        Scenario::new(seats)
            .battlefield(0, battlefield)
            .at(1, 0, Step::Main1),
        |scenario, seat| scenario.library(seat, vec![swamp(); 5]),
    )
}

#[rstest]
#[case::two_swamps(2, 3, Some(2))]
#[case::four_swamps(4, 1, Some(4))]
#[case::no_swamp(0, 4, None)]
fn crypt_rats_offers_x_of_its_black_mana_only(
    #[case] swamps: usize,
    #[case] mountains: usize,
    #[case] most: Option<u32>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut battlefield = vec![ready(corpus_card("Crypt Rats").unwrap())];
    battlefield.extend(lands(swamps, mountains));
    let mut driver = Driver::from_scenario(scenario(seats, battlefield))?;
    driver.begin_activation("Crypt Rats")?;
    assert_eq!(most_x(&driver), most);
    Ok(())
}

#[rstest]
fn crypt_rats_pays_x_in_black_and_deals_that_much_to_everything(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut battlefield = vec![ready(corpus_card("Crypt Rats").unwrap())];
    battlefield.extend(lands(3, 3));
    let mut driver = Driver::from_scenario(scenario(seats, battlefield))?;
    driver.begin_activation("Crypt Rats")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(2)))?;
    let cost = driver.pay_options()?.cost.clone();
    assert_eq!(cost.to_string(), "{B}{B}", "each 1 of X is black mana");
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    for seat in 0..seats {
        assert_eq!(life(&driver, seat), 18, "seat {seat}");
    }
    assert!(
        crate::common::names(driver.state(), Zone::Graveyard(pid(0)))
            .contains(&"Crypt Rats".to_owned())
    );
    let untapped_mountains = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| driver.state().object(*id))
        .filter(|object| object.card().name == "Mountain")
        .filter(|object| object.permanent().is_some_and(|p| !p.tapped()))
        .count();
    assert_eq!(untapped_mountains, 3, "no red mana pays for X");
    Ok(())
}

#[rstest]
#[case::black(Color::Black, "{B}")]
#[case::red(Color::Red, "{R}")]
fn the_color_of_x_is_the_color_the_cost_names(
    #[case] color: Color,
    #[case] symbol: &str,
    #[values(2, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let hellkite = corpus_card("Crimson Hellkite").unwrap();
    let mut def = (*hellkite).clone();
    let json = serde_json::to_string(&*def.rules[1])
        .unwrap()
        .replace(r#""Red""#, &format!("{:?}", format!("{color:?}")));
    def.rules[1] = std::sync::Arc::new(serde_json::from_str(&json).unwrap());
    let mut battlefield = vec![ready(std::sync::Arc::new(def))];
    battlefield.extend(lands(3, 3));
    let mut driver = Driver::from_scenario(scenario(seats, battlefield))?;
    driver.begin_activation("Crimson Hellkite")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(3)))?;
    let target = driver.target_named("Crimson Hellkite")?;
    driver.choose_targets(&[target])?;
    assert_eq!(driver.pay_options()?.cost.to_string(), symbol.repeat(3));
    Ok(())
}

#[rstest]
#[case::three_swamps(3, 3, Some(2))]
#[case::one_swamp(1, 5, None)]
fn consume_spirit_offers_x_of_the_black_mana_left_after_its_own(
    #[case] swamps: usize,
    #[case] mountains: usize,
    #[case] most: Option<u32>,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario =
        scenario(seats, lands(swamps, mountains)).hand(0, [corpus_card("Consume Spirit").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Consume Spirit")?;
    assert_eq!(most_x(&driver), most);
    Ok(())
}

#[rstest]
fn consume_spirit_drains_the_x_paid_in_black(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = scenario(seats, lands(4, 2)).hand(0, [corpus_card("Consume Spirit").unwrap()]);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.begin_cast("Consume Spirit")?;
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(3)))?;
    let foe = driver.target_player(seats - 1);
    driver.choose_targets(&[foe])?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{1}{B}{B}{B}{B}");
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(life(&driver, seats - 1), 17);
    assert_eq!(life(&driver, 0), 23);
    Ok(())
}
