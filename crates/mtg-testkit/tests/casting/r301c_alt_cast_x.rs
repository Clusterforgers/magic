//! Goal: an `{X}` in the cost of a way an `AltCast` rule prints ("Warp {X}{G}", "Surge {X}{R}") is announced as the spell is cast that way and counted once for each `{X}` of that cost, not of the mana cost, while the effects read the X announced; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::value::CounterKind;
use mtg_core::color::Color;
use mtg_engine::choose::NumberChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_testkit::cards::land;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, ways};

/// Passes until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..16 {
        if driver.state().stack().is_empty() {
            return Ok(());
        }
        driver.pass()?;
    }
    Ok(())
}

/// Answers the announcement of X.
fn say_x(driver: &mut Driver, x: u32) -> Result<(), ScriptError> {
    assert!(matches!(
        driver.decision()?.kind,
        DecisionKind::ChooseNumber(_)
    ));
    driver.answer(Answer::ChooseNumber(NumberChoice::Number(x)))?;
    Ok(())
}

fn lands(count: usize, color: Color) -> Vec<mtg_engine::PresetPermanent> {
    (0..count).map(|_| ready(land(color))).collect()
}

#[rstest]
#[case::warped("Warp", "{2}{G}")]
#[case::for_the_mana_cost("Cast for", "{2}{G}{G}")]
fn broodguard_elite_warped_for_x_pays_x_once_and_enters_with_x_counters(
    #[values(2, 3, 4)] seats: usize,
    #[case] way: &str,
    #[case] cost: &str,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [card("Broodguard Elite").unwrap()])
            .battlefield(0, lands(5, Color::Green)),
    )?;
    driver.begin_cast("Broodguard Elite")?;
    assert!(ways(&driver)?.iter().any(|label| label == "Warp {X}{G}"));
    choose_way(&mut driver, way)?;
    say_x(&mut driver, 2)?;
    assert_eq!(driver.pay_options()?.cost.to_string(), cost);
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    settle(&mut driver)?;
    let id = driver.id_of("Broodguard Elite")?;
    let permanent = driver
        .state()
        .object(id)
        .and_then(|o| o.permanent())
        .unwrap();
    assert_eq!(permanent.counters_of(&CounterKind::PlusOnePlusOne), 2);
    Ok(())
}

#[rstest]
fn fall_of_the_titans_surged_for_x_pays_x_once_and_deals_x(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = lands(4, Color::Red);
    mine.extend(lands(1, Color::Red));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(
                0,
                [
                    card("Fall of the Titans").unwrap(),
                    card("Lightning Bolt").unwrap(),
                ],
            )
            .battlefield(0, mine),
    )?;
    let target = driver.target_player(1);
    driver.cast_at("Lightning Bolt", &[target])?;
    settle(&mut driver)?;
    driver.begin_cast("Fall of the Titans")?;
    choose_way(&mut driver, "Surge")?;
    say_x(&mut driver, 3)?;
    let target = driver.target_player(1);
    driver.choose_targets(&[target])?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{3}{R}");
    driver.answer(Answer::PayMana(mtg_engine::decision::PayChoice::Auto))?;
    settle(&mut driver)?;
    assert_eq!(
        driver
            .state()
            .player(mtg_core::ids::PlayerId::from_index(1))
            .unwrap()
            .life(),
        20 - 3 - 3
    );
    Ok(())
}

#[rstest]
fn surge_is_not_offered_before_another_spell_was_cast(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand(0, [card("Fall of the Titans").unwrap()])
            .battlefield(0, lands(7, Color::Red)),
    )?;
    driver.begin_cast("Fall of the Titans")?;
    assert!(
        matches!(driver.decision()?.kind, DecisionKind::ChooseNumber(_)),
        "no way is asked"
    );
    Ok(())
}
