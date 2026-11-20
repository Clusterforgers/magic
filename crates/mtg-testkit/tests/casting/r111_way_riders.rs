//! Goal: an alternative cost a static ability gives a card outside the battlefield works like a printed one, and the way
//! brings the riders its keyword has (CR 702.185a warp, 702.190a sneak): Tannuk's warp is offered for artifact and red
//! creature cards in its controller's hand alone, and the warped permanent is exiled at the next end step even once
//! Tannuk is gone and may be cast from exile later; the riders are not doubled for a card that prints warp.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CastWay;
use mtg_core::color::Color;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, mountain, swamp};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, lands, pay, ways};
use crate::common::{names, on_battlefield, pid};

const TANNUK: &str = "Tannuk, Steadfast Second";

/// Seat 0 holds Tannuk and four Mountains with `hand` in its hand at its first main phase.
fn table(seats: usize, hand: Vec<Arc<CardDefinition>>) -> Result<Driver, ScriptError> {
    let mut permanents = lands(4, mountain);
    permanents.push(ready(card(TANNUK).unwrap()));
    Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, hand)
            .battlefield(0, permanents),
    )
}

/// The labels of the ways `name` may be cast now (none past the plain cost when no way is asked).
fn ways_of(driver: &mut Driver, name: &str) -> Result<Vec<String>, ScriptError> {
    driver.begin_cast(name)?;
    let found = if matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        ways(driver)?
    } else {
        Vec::new()
    };
    Ok(found)
}

/// Answers whatever is asked passively until the stack is empty.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..80 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

#[rstest]
#[case::red_creature(Color::Red, true)]
#[case::green_creature(Color::Green, false)]
fn r111_tannuk_gives_warp_to_red_creature_cards_in_hand(
    #[values(2, 3, 4)] seats: usize,
    #[case] color: Color,
    #[case] warped: bool,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, vec![creature("Hand Beast", 3, &[color], 3, 3)])?;
    if !warped {
        // The Mountains pay no green mana: the card is castable only for a warp cost it was given.
        let DecisionKind::Priority(options) = &driver.decision()?.kind else {
            panic!("priority");
        };
        assert_eq!(options.castable, Vec::new());
        return Ok(());
    }
    let offered = ways_of(&mut driver, "Hand Beast")?;
    assert!(
        offered.iter().any(|way| way.starts_with("Warp")),
        "{offered:?}"
    );
    Ok(())
}

#[rstest]
fn r111_a_warped_permanent_is_exiled_at_the_end_step_and_may_be_cast_later(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, vec![creature("Hand Beast", 3, &[Color::Red], 3, 3)])?;
    driver.begin_cast("Hand Beast")?;
    choose_way(&mut driver, "Warp")?;
    for _ in 0..10 {
        if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    pay(&mut driver)?;
    settle(&mut driver)?;
    let beast = on_battlefield(driver.state(), "Hand Beast").unwrap();
    assert_eq!(
        driver.state().object(beast).unwrap().cast_way(),
        Some(CastWay::Warp)
    );
    driver.idle_until(2, Step::Upkeep)?;
    assert_eq!(names(driver.state(), Zone::Exile), ["Hand Beast"]);
    driver.idle_until(1 + u32::try_from(seats).unwrap(), Step::Main1)?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority");
    };
    let exiled = driver.state().zone(Zone::Exile)[0];
    assert!(options.castable.contains(&exiled));
    Ok(())
}

#[rstest]
fn r111_a_card_that_prints_warp_keeps_its_own_riders_alone(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let printed =
        mtg_testkit::alt_cast_cards::card(mtg_testkit::alt_cast_cards::row("Warp Beast").unwrap());
    let mut driver = table(seats, vec![printed])?;
    driver.begin_cast("Warp Beast")?;
    choose_way(&mut driver, "Warp")?;
    for _ in 0..10 {
        if matches!(driver.decision()?.kind, DecisionKind::PayMana(_)) {
            break;
        }
        driver.answer_passively()?;
    }
    pay(&mut driver)?;
    settle(&mut driver)?;
    let beast = on_battlefield(driver.state(), "Warp Beast").unwrap();
    let riders = mtg_engine::ability_reader::abilities_of(driver.state(), beast)
        .into_iter()
        .filter(|(address, _)| address.source == mtg_engine::abilities::AbilitySource::R111WayRider)
        .count();
    assert_eq!(riders, 0);
    Ok(())
}

/// Seat 0 has Ninja Teen raised to `level`, Grizzly Bears, seven Swamps and a creature card in its graveyard; it attacks
/// the last seat with the Bears unblocked and the game stops in the declare blockers step.
fn ninja_table(seats: usize, level: u32) -> Result<Driver, ScriptError> {
    let mut permanents = lands(7, swamp);
    permanents.push(ready(card("Ninja Teen").unwrap()));
    permanents.push(ready(card("Grizzly Bears").unwrap()));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .graveyard(0, [creature("Grave Rogue", 3, &[Color::Black], 2, 2)])
            .battlefield(0, permanents),
    )?;
    // The level up abilities are the card's second and fourth rules.
    for (rule, _) in [(1, 2), (3, 3)].into_iter().filter(|(_, at)| *at <= level) {
        driver.begin_ability("Ninja Teen", rule)?;
        pay(&mut driver)?;
        settle(&mut driver)?;
    }
    driver.idle_until(1, Step::DeclareAttackers)?;
    driver.attack(&[("Grizzly Bears", seats - 1)])?;
    loop {
        if matches!(driver.decision()?.kind, DecisionKind::DeclareBlockers(_)) {
            driver.answer(Answer::DeclareBlockers(Vec::new()))?;
        } else if driver.state().turn().step() == Step::DeclareBlockers
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(driver);
        } else {
            driver.answer_passively()?;
        }
    }
}

fn castable(driver: &Driver, name: &str) -> Result<bool, ScriptError> {
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        return Ok(false);
    };
    Ok(options.castable.iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|object| object.card().name == name)
    }))
}

#[rstest]
fn r111_ninja_teen_at_level_three_sneaks_a_creature_card_from_the_graveyard(
    #[values(2, 3, 4)] seats: usize,
    #[values(2, 3)] level: u32,
) -> Result<(), ScriptError> {
    let mut driver = ninja_table(seats, level)?;
    assert_eq!(castable(&driver, "Grave Rogue")?, level == 3);
    if level < 3 {
        return Ok(());
    }
    driver.begin_cast("Grave Rogue")?;
    for _ in 0..10 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseOption(_) => choose_way(&mut driver, "Sneak")?,
            DecisionKind::ChooseCards(options) => {
                let pick = options.from.clone();
                driver.answer(Answer::ChooseCards(mtg_engine::choose::CardsChoice(pick)))?;
            }
            DecisionKind::PayMana(_) => break,
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    pay(&mut driver)?;
    settle(&mut driver)?;
    let rogue = on_battlefield(driver.state(), "Grave Rogue").unwrap();
    let object = driver.state().object(rogue).unwrap();
    // The sneak rider: the permanent enters tapped, and attacking.
    assert!(object.permanent().unwrap().tapped());
    assert!(
        driver
            .state()
            .combat()
            .attackers()
            .iter()
            .any(|a| a.object() == rogue)
    );
    assert_eq!(names(driver.state(), Zone::Hand(pid(0))), ["Grizzly Bears"]);
    Ok(())
}
