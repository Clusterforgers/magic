//! Goal: blitz a static ability gives creature spells (Henzie "Toolbox" Torre) is offered for a creature spell of mana
//! value 4 or greater alone, for its mana cost less {1} for each cast of the commander from the command zone, and the
//! blitzed permanent has haste and is sacrificed at the next end step though its card prints no blitz (CR 702.152a).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::Supertype;
use mtg_core::color::Color;
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Commander;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, forest};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::{choose_way, lands, ways};
use crate::common::{names, on_battlefield, pid};

/// A legendary 1/1 for {G}: the commander.
fn legend() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::clone(&creature("Cmd", 0, &[Color::Green], 1, 1))
            .with_supertype(Supertype::Legendary),
    )
}

/// Seat 0 has Henzie and eight Forests, a commander "Cmd" in its command zone and `hand` in its hand.
fn table(seats: usize, generic: u32) -> Result<Driver, ScriptError> {
    let mut permanents = lands(8, forest);
    permanents.push(ready(card("Henzie \"Toolbox\" Torre").unwrap()));
    Driver::from_scenario(
        Scenario::new(seats)
            .format(Arc::new(Commander))
            .at(1, 0, Step::Main1)
            .commanders(0, [legend()])
            .hand(0, [creature("Big Beast", generic, &[Color::Green], 4, 4)])
            .battlefield(0, permanents),
    )
}

/// Begins casting the Beast and answers the way `way` when a way is asked; the ways offered.
fn begin(driver: &mut Driver, way: Option<&str>) -> Result<Vec<String>, ScriptError> {
    driver.begin_cast("Big Beast")?;
    if !matches!(driver.decision()?.kind, DecisionKind::ChooseOption(_)) {
        return Ok(Vec::new());
    }
    let offered = ways(driver)?;
    if let Some(way) = way {
        choose_way(driver, way)?;
    }
    Ok(offered)
}

#[rstest]
#[case::mana_value_four(3, true)]
#[case::mana_value_three(2, false)]
fn r111_blitz_is_given_to_creature_spells_of_mana_value_four_or_more(
    #[values(2, 3, 4)] seats: usize,
    #[case] generic: u32,
    #[case] blitz: bool,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, generic)?;
    let offered = begin(&mut driver, None)?;
    assert_eq!(
        offered.iter().any(|way| way.starts_with("Blitz")),
        blitz,
        "{offered:?}"
    );
    Ok(())
}

#[rstest]
fn r111_a_blitzed_permanent_has_haste_and_is_sacrificed_at_the_end_step(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = table(seats, 3)?;
    begin(&mut driver, Some("Blitz"))?;
    let DecisionKind::PayMana(options) = &driver.decision()?.kind else {
        panic!("the payment is asked");
    };
    // No commander was cast yet: the blitz cost is the mana cost.
    assert_eq!(options.cost.mana_value(), 4);
    driver.answer(mtg_engine::decision::Answer::PayMana(
        mtg_engine::decision::PayChoice::Auto,
    ))?;
    for _ in 0..40 {
        if on_battlefield(driver.state(), "Big Beast").is_some()
            && driver.state().stack().is_empty()
        {
            break;
        }
        driver.answer_passively()?;
    }
    let beast = on_battlefield(driver.state(), "Big Beast").unwrap();
    assert!(
        mtg_engine::keywords::keywords_of(driver.state(), beast)
            .iter()
            .any(|keyword| matches!(keyword, mtg_cards::keyword::Keyword::Haste))
    );
    driver.idle_until(2, Step::Upkeep)?;
    assert!(names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Big Beast".to_owned()));
    Ok(())
}
