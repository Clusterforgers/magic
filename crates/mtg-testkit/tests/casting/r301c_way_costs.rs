//! Goal: a cost change of the spells cast a way ("Flashback costs you pay cost {2} less.", "Flashback costs your opponents pay cost {2} more.", "Dash costs you pay cost {2} less.") changes the cost of a spell cast that way by the player it names, and leaves the other ways and the other players alone; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::casting::{choose_way, lands, ways};
use mtg_engine::turn::Step;
use mtg_testkit::cards::mountain;
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat `caster`, the active player, has Faithless Looting in its graveyard (else in its hand) and five Mountains; seat
/// `stone` controls a Catalyst Stone.
fn looting(seats: usize, caster: usize, stone: usize, graveyard: bool) -> Scenario {
    let mut mine = lands(5, mountain);
    let catalyst = ready(card("Catalyst Stone").unwrap());
    let mut scenario = Scenario::new(seats).at(1, caster, Step::Main1);
    if stone == caster {
        mine.push(catalyst);
    } else {
        scenario = scenario.battlefield(stone, [catalyst]);
    }
    let scenario = scenario.battlefield(caster, mine);
    if graveyard {
        scenario.graveyard_named(caster, ["Faithless Looting"])
    } else {
        scenario.hand_named(caster, ["Faithless Looting"])
    }
}

#[rstest]
#[case::your_flashback_costs_less(0, 0, "{R}")]
#[case::an_opponents_flashback_costs_more(1, 0, "{4}{R}")]
fn catalyst_stone_changes_the_flashback_cost_of_its_player(
    #[values(2, 3, 4)] seats: usize,
    #[case] caster: usize,
    #[case] stone: usize,
    #[case] cost: &str,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(looting(seats, caster, stone, true))?;
    driver.begin_cast("Faithless Looting")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), cost);
    Ok(())
}

#[rstest]
#[case::yours(0, 0)]
#[case::an_opponents(1, 0)]
fn catalyst_stone_leaves_the_cast_from_the_hand_alone(
    #[values(2, 3, 4)] seats: usize,
    #[case] caster: usize,
    #[case] stone: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(looting(seats, caster, stone, false))?;
    driver.begin_cast("Faithless Looting")?;
    assert_eq!(driver.pay_options()?.cost.to_string(), "{R}");
    Ok(())
}

#[rstest]
#[case::dash_costs_two_less("Dash", "{R}")]
#[case::the_mana_cost_stays("Cast for", "{3}{R}")]
fn a_warbringer_on_the_battlefield_takes_two_off_the_dash_cost(
    #[values(2, 3, 4)] seats: usize,
    #[case] way: &str,
    #[case] cost: &str,
) -> Result<(), ScriptError> {
    let mut mine = lands(4, mountain);
    mine.push(ready(card("Warbringer").unwrap()));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Warbringer"])
            .battlefield(0, mine),
    )?;
    driver.begin_cast("Warbringer")?;
    assert_eq!(ways(&driver)?.len(), 2);
    choose_way(&mut driver, way)?;
    assert_eq!(driver.pay_options()?.cost.to_string(), cost);
    Ok(())
}
