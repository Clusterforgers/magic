//! Goal: disturb casts the back face of a transforming double-faced card from its owner's graveyard for the disturb cost
//! alone (Twinblade Geist // Twinblade Invocation): it is offered with the mana and in no other zone, the spell and the
//! permanent are the back face, the card from a hand is its front face for its mana cost, and the back face is exiled
//! instead of going to a graveyard; at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::keyword::Keyword;
use mtg_cards::value::CastWay;
use mtg_engine::Event;
use mtg_engine::choose::Target;
use mtg_engine::decision::{Answer, PayChoice};
use mtg_engine::keywords::has_keyword;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{burn, free_creature, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::{names, on_battlefield, pid};

/// Seat 0 controls a Bear, `plains` Plains and a Mountain; the Geist lies in `place`; every other seat has the Geist's
/// owner's rival Bear.
fn dk1d_geist(seats: usize, place: &str, plains_count: usize) -> Scenario {
    let geist = card("Twinblade Geist // Twinblade Invocation").unwrap();
    let mut hand = vec![burn("Bolt", 5)];
    let scenario = Scenario::new(seats).at(1, 0, Step::Main1);
    let scenario = match place {
        "hand" => {
            hand.push(geist);
            scenario
        }
        "library" => scenario.library(0, [geist]),
        _ => scenario.graveyard(0, [geist]),
    };
    let lands = (0..plains_count)
        .map(|_| ready(plains()))
        .chain([ready(mountain()), ready(free_creature("Bear", 2, 2))]);
    scenario.hand(0, hand).battlefield(0, lands)
}

/// Passes until the stack is empty.
fn dk1d_settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..60 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The Bear as a target.
fn dk1d_bear(driver: &Driver) -> Target {
    let id = on_battlefield(driver.state(), "Bear").unwrap();
    Target::Object(driver.state().object(id).unwrap().reference())
}

/// Casts the back face from the graveyard on the Bear and lets it resolve, returning the events of the cast.
fn dk1d_disturb(driver: &mut Driver) -> Result<Vec<Event>, ScriptError> {
    driver.begin_cast("Twinblade Geist")?;
    let bear = dk1d_bear(driver);
    driver.choose_targets(&[bear])?;
    driver.expect_decision(0, "pay-mana")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{2}{W}");
    let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    dk1d_settle(driver)?;
    Ok(events)
}

#[rstest]
fn dk1d_the_back_face_is_cast_from_the_graveyard_for_the_disturb_cost(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let setup = dk1d_geist(seats, "graveyard", 3).setup().unwrap();
    let mut driver = Driver::from_scenario(dk1d_geist(seats, "graveyard", 3))?;
    let events = dk1d_disturb(&mut driver)?;
    let cast = events
        .iter()
        .find_map(|event| match event {
            Event::SpellCast { choices, .. } => Some(choices.clone()),
            _ => None,
        })
        .expect("a spell was cast");
    assert_eq!(cast.way, Some(CastWay::Disturb));
    assert_eq!(cast.face, Some(1), "the back face is the spell");
    let bear = on_battlefield(driver.state(), "Bear").unwrap();
    let aura = on_battlefield(driver.state(), "Twinblade Invocation").unwrap();
    assert_eq!(driver.state().host_of(aura), Some(bear));
    assert!(has_keyword(driver.state(), bear, &Keyword::DoubleStrike));
    assert_eq!(
        names(driver.state(), Zone::Graveyard(pid(0))),
        Vec::<String>::new()
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::the_mana_of_the_cost("graveyard", 3, true)]
#[case::two_lands_are_short("graveyard", 1, false)]
#[case::from_a_hand_the_front_face_costs_its_own("hand", 1, true)]
#[case::from_a_library_it_is_no_card_to_cast("library", 3, false)]
fn dk1d_the_card_is_offered_where_a_way_to_cast_it_exists(
    #[values(2, 4)] seats: usize,
    #[case] place: &str,
    #[case] plains_count: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(dk1d_geist(seats, place, plains_count))?;
    assert_eq!(
        driver.begin_cast("Twinblade Geist").is_ok(),
        offered,
        "{place}"
    );
    if offered && place == "hand" {
        assert_eq!(driver.pay_options()?.cost.to_string(), "{1}{W}");
    }
    Ok(())
}

#[test]
fn dk1d_the_back_face_is_exiled_instead_of_going_to_the_graveyard() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(dk1d_geist(2, "graveyard", 3))?;
    dk1d_disturb(&mut driver)?;
    let bear = dk1d_bear(&driver);
    driver.cast_at("Bolt", &[bear])?;
    dk1d_settle(&mut driver)?;
    assert!(
        on_battlefield(driver.state(), "Bear").is_none(),
        "the Bear died"
    );
    assert!(on_battlefield(driver.state(), "Twinblade Invocation").is_none());
    assert_eq!(names(driver.state(), Zone::Exile), ["Twinblade Geist"]);
    let mut graveyard = names(driver.state(), Zone::Graveyard(pid(0)));
    graveyard.sort();
    assert_eq!(
        graveyard,
        ["Bear", "Bolt"],
        "the Aura is not in the graveyard"
    );
    Ok(())
}

#[test]
fn dk1d_a_disturbed_spell_that_does_not_resolve_is_exiled() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(dk1d_geist(2, "graveyard", 3))?;
    driver.begin_cast("Twinblade Geist")?;
    let bear = dk1d_bear(&driver);
    driver.choose_targets(&[bear])?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    // The Bear dies in response, so the Aura has no target left and does nothing.
    driver.cast_at("Bolt", &[bear])?;
    dk1d_settle(&mut driver)?;
    assert!(on_battlefield(driver.state(), "Twinblade Invocation").is_none());
    assert_eq!(names(driver.state(), Zone::Exile), ["Twinblade Geist"]);
    assert!(
        !names(driver.state(), Zone::Graveyard(pid(0))).contains(&"Twinblade Geist".to_owned())
    );
    Ok(())
}
