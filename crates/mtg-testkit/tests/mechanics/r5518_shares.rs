//! Goal: Spirit-Sister's Call returns the chosen card at the end step only when a permanent that shares a card type
//! with it was sacrificed, at two to four seats: sacrificing a creature returns a creature card, declining or having
//! no permanent of a shared type returns nothing.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::PlayerId;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{free_creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Whether the object named `name` is in `zone`.
fn r5518_in(driver: &Driver, zone: Zone, name: &str) -> bool {
    driver.state().zone(zone).iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|object| object.card().name == name)
    })
}

#[rstest]
#[case::two_seats_sacrificed(2, true, true)]
#[case::three_seats_declined(3, false, true)]
#[case::four_seats_sacrificed(4, true, true)]
#[case::two_seats_no_creature(2, true, false)]
#[case::four_seats_no_creature(4, true, false)]
fn r5518_spirit_sisters_call_returns_the_card_only_after_a_shared_type_sacrifice(
    #[case] seats: usize,
    #[case] accept: bool,
    #[case] fodder_is_creature: bool,
) -> Result<(), ScriptError> {
    let fodder = if fodder_is_creature {
        ready(free_creature("Pip", 1, 1))
    } else {
        ready(island())
    };
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(
                0,
                vec![ready(card("Spirit-Sister's Call").unwrap()), fodder],
            )
            .graveyard(0, [free_creature("Ghost", 3, 3)])
            .at(1, 0, Step::Main2),
    )?;
    driver.pass_until(Step::End)?;
    for _ in 0..40 {
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(accept)))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            DecisionKind::Priority(_) => {
                driver.pass()?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    let me = PlayerId::from_index(0);
    let returned = accept && fodder_is_creature;
    assert_eq!(r5518_in(&driver, Zone::Battlefield, "Ghost"), returned);
    assert_eq!(r5518_in(&driver, Zone::Graveyard(me), "Ghost"), !returned);
    assert_eq!(r5518_in(&driver, Zone::Graveyard(me), "Pip"), returned);
    Ok(())
}
