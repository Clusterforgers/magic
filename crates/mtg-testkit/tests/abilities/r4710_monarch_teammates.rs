//! Goal: "You can't become the monarch this turn." and "each of your teammates" play at two to four seats: Jared
//! Carthalion makes the target opponent the monarch and Okoye's "you become the monarch" changes nothing the same turn,
//! while Okoye alone does make its caster the monarch; Imperial Mask gives one copy to each teammate (none at a table
//! without teams), and the copy, a token, gives none.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::house::HouseRules;
use mtg_core::ids::PlayerId;
use mtg_core::teams::{TeamMode, Teams};
use mtg_engine::characteristics::controller_of;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Passes priority until the stack is empty, choosing `target` for a trigger that asks for its target.
fn r4710_settle(driver: &mut Driver, target: usize) -> Result<(), ScriptError> {
    for _ in 0..80 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseTargets(_) => {
                let player = driver.target_player(target);
                driver.choose_targets(&[player])?;
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
    Ok(())
}

/// Seat 0 casts Jared Carthalion (targeting `target`) when `jared`, then Okoye, Mighty and Adored; the monarch after.
#[rstest]
#[case::two_seats(2, 1, true, 1)]
#[case::three_seats(3, 2, true, 2)]
#[case::four_seats(4, 3, true, 3)]
#[case::okoye_alone_crowns_you(3, 1, false, 0)]
fn r4710_jared_keeps_you_from_the_crown_this_turn(
    #[case] seats: usize,
    #[case] target: usize,
    #[case] jared: bool,
    #[case] monarch: usize,
) -> Result<(), ScriptError> {
    let lands = [
        mountain(),
        forest(),
        plains(),
        forest(),
        plains(),
        mountain(),
        mountain(),
    ];
    let scenario = Scenario::new(seats)
        .battlefield(0, lands.into_iter().map(ready))
        .hand(
            0,
            [
                card("Jared Carthalion, True Heir").unwrap(),
                card("Okoye, Mighty and Adored").unwrap(),
            ],
        )
        .at(1, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    if jared {
        driver.cast("Jared Carthalion, True Heir")?;
        r4710_settle(&mut driver, target)?;
        assert_eq!(driver.state().monarch(), Some(PlayerId::from_index(target)));
    }
    driver.cast("Okoye, Mighty and Adored")?;
    r4710_settle(&mut driver, target)?;
    assert_eq!(
        driver.state().monarch(),
        Some(PlayerId::from_index(monarch))
    );
    Ok(())
}

/// Seat 0 casts Imperial Mask at a table with `teams`; the seats that then control a copy of it.
#[rstest]
#[case::no_teams(2, None, &[])]
#[case::free_for_all(4, None, &[])]
#[case::teams_of_two(4, Some(TeamMode::Teams), &[2])]
#[case::two_headed_giant(4, Some(TeamMode::TwoHeadedGiant), &[1])]
fn r4710_imperial_mask_copies_go_to_each_teammate(
    #[case] seats: usize,
    #[case] teams: Option<TeamMode>,
    #[case] copies: &[usize],
) -> Result<(), ScriptError> {
    let house = HouseRules {
        teams: teams.map(|mode| Teams::even(mode, 2, 2)),
        ..HouseRules::default()
    };
    let scenario = Scenario::new(seats)
        .house_rules(house)
        .battlefield(0, (0..5).map(|_| ready(plains())))
        .hand(0, [card("Imperial Mask").unwrap()])
        .at(1, 0, Step::Main1);
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Imperial Mask")?;
    r4710_settle(&mut driver, 1)?;
    let state = driver.state();
    let mut holders: Vec<usize> = state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| crate::common::name_of(state, **id) == "Imperial Mask")
        .filter_map(|id| controller_of(state, *id))
        .map(PlayerId::index)
        .collect();
    holders.sort_unstable();
    let mut expected = vec![0];
    expected.extend_from_slice(copies);
    assert_eq!(holders, expected);
    Ok(())
}
