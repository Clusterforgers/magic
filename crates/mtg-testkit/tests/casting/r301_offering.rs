//! Goal: offering ("Fox offering", Patron of the Kitsune) casts the spell from its owner's hand at instant speed on an opponent's turn by sacrificing a Fox and paying the mana cost less the Fox's mana cost, colors included, at tables of two to four seats; it is not offered without a Fox, from too few lands, or with a creature of another type.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::lands;
use crate::common::{names, pid};
use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{island, plains};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn creature(name: &str, subtype: &str, cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(subtype)
            .with_mana_cost(ManaCost::parse(cost).unwrap())
            .with_power_toughness(1, 1),
    )
}

/// Seat 0 with Patron of the Kitsune in hand, `plains` Plains and the creature given, on the last seat's main phase.
fn table(seats: usize, plains_count: usize, sacrifice: Arc<CardDefinition>) -> Scenario {
    let mut lands_and_more = lands(plains_count, plains);
    lands_and_more.push(ready(sacrifice));
    let mut scenario = Scenario::new(seats)
        .at(u32::try_from(seats).unwrap(), seats - 1, Step::Main1)
        .hand_named(0, ["Patron of the Kitsune"])
        .battlefield(0, lands_and_more);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..10).map(|_| island()));
    }
    scenario
}

/// Passes until seat 0 has priority on the last seat's turn.
fn to_seat_zero(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..8 {
        if driver.decision()?.player == pid(0)
            && matches!(driver.decision()?.kind, DecisionKind::Priority(_))
        {
            return Ok(());
        }
        driver.pass()?;
    }
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::three_seats(3)]
#[case::four_seats(4)]
fn a_fox_pays_part_of_the_patron_at_instant_speed(#[case] seats: usize) -> Result<(), ScriptError> {
    // {4}{W}{W} less a {1}{W} Fox is {3}{W}: four Plains pay it.
    let mut driver = Driver::from_scenario(table(seats, 4, creature("Fox", "Fox", "{1}{W}")))?;
    to_seat_zero(&mut driver)?;
    driver.begin_cast("Patron of the Kitsune")?;
    let options = driver.pay_options()?;
    assert_eq!(options.cost.mana_value(), 4, "{}", options.cost);
    assert_eq!(options.cost.required(mtg_core::mana::ManaType::White), 1);
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    assert_eq!(names(driver.state(), Zone::Graveyard(pid(0))), ["Fox"]);
    assert_eq!(driver.state().stack().len(), 1);
    Ok(())
}

#[rstest]
#[case::three_plains_are_too_few(3, "Fox", true)]
#[case::a_goblin_is_no_fox(5, "Goblin", false)]
fn no_offering_without_a_fox_or_the_mana(
    #[case] plains_count: usize,
    #[case] subtype: &str,
    #[case] fox: bool,
) -> Result<(), ScriptError> {
    let sacrifice = creature("Other", subtype, "{1}{W}");
    let mut driver = Driver::from_scenario(table(2, plains_count, sacrifice))?;
    to_seat_zero(&mut driver)?;
    // Six Plains would pay the printed cost, but not at instant speed on another seat's turn.
    assert!(
        driver.begin_cast("Patron of the Kitsune").is_err(),
        "fox: {fox}"
    );
    Ok(())
}
