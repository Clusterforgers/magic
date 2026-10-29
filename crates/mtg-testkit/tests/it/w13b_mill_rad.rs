//! Goal: the mill and rad leftovers at tables of two to four seats: the rad mill of seat 0 makes Screeching Scorchbeast create a token for each nonland card once a turn, Zellix make one Horror for the player who milled creatures, The Wise Mothman offer as many targets as nonland cards were milled, and Tato Farmer put a land milled this turn (never one put there earlier) onto the battlefield; Mariposa Military Base enters tapped with two rad counters for its controller only when they say so; each game replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common;

use common::copying::replays;
use mtg_cards::value::CounterKind;
use mtg_core::ids::PlayerId;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

/// Seat 0 holds three rad counters over a library of a Grizzly Bears to draw, then `top` (a Forest for 'L', Grizzly
/// Bears otherwise, the first on top) and ten more Grizzly Bears; every other seat draws from ten Grizzly Bears. The
/// game starts in seat 0's upkeep of turn 2.
fn w13b_scenario(seats: usize, top: &str) -> Scenario {
    let library: Vec<&str> = std::iter::once("Grizzly Bears")
        .chain(
            top.chars()
                .map(|c| if c == 'L' { "Forest" } else { "Grizzly Bears" }),
        )
        .chain(std::iter::repeat_n("Grizzly Bears", 10))
        .collect();
    (1..seats).fold(
        Scenario::new(seats)
            .at(2, 0, Step::Upkeep)
            .library_named(0, library)
            .counters(0, rad(), 3),
        |scenario, seat| scenario.library_named(seat, std::iter::repeat_n("Grizzly Bears", 10)),
    )
}

fn rad() -> CounterKind {
    CounterKind::Other("rad".to_owned())
}

/// Plays to seat 0's combat, answering "yes" to each may, every target it may choose to each target choice (noting the
/// most it was offered) and passively to the rest.
fn w13b_play(driver: &mut Driver, offered: &mut Vec<u32>) -> Result<(), ScriptError> {
    driver.idle_until(2, Step::Main1)?;
    for _ in 0..200 {
        if driver.state().turn().step() == Step::BeginCombat {
            return Ok(());
        }
        match &driver.decision()?.kind {
            DecisionKind::YesNo(_) => {
                driver.answer(Answer::YesNo(YesNoChoice(true)))?;
            }
            DecisionKind::ChooseTargets(options) => {
                offered.push(options.max);
                let chosen: Vec<_> = options
                    .legal
                    .iter()
                    .take(options.max as usize)
                    .copied()
                    .collect();
                driver.choose_targets(&chosen)?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("combat".to_owned()))
}

fn w13b_tokens(driver: &Driver, name: &str) -> usize {
    driver
        .state()
        .objects()
        .filter(|o| o.card().name == name && o.zone() == Zone::Battlefield)
        .count()
}

#[rstest]
#[case::all_nonland("CCC", 3)]
#[case::some_lands("LCL", 1)]
#[case::only_lands("LLL", 0)]
fn w13b_scorchbeast_creates_a_token_for_each_nonland_card(
    #[values(2, 3, 4)] seats: usize,
    #[case] top: &str,
    #[case] tokens: usize,
) -> Result<(), ScriptError> {
    let scenario =
        || w13b_scenario(seats, top).battlefield_named(seats - 1, ["Screeching Scorchbeast"]);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    w13b_play(&mut driver, &mut Vec::new())?;
    assert_eq!(w13b_tokens(&driver, "Zombie Mutant"), tokens);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::creatures("CLC", 1)]
#[case::only_lands("LLL", 0)]
fn w13b_zellix_makes_one_horror_for_the_player_who_milled_creatures(
    #[values(2, 3, 4)] seats: usize,
    #[case] top: &str,
    #[case] horrors: usize,
) -> Result<(), ScriptError> {
    let scenario =
        || w13b_scenario(seats, top).battlefield_named(seats - 1, ["Zellix, Sanity Flayer"]);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    w13b_play(&mut driver, &mut Vec::new())?;
    assert_eq!(w13b_tokens(&driver, "Horror"), horrors);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::three_nonland("CCC", vec![3])]
#[case::one_nonland("LCL", vec![1])]
#[case::only_lands("LLL", vec![])]
fn w13b_mothman_offers_as_many_targets_as_nonland_cards_milled(
    #[values(2, 3, 4)] seats: usize,
    #[case] top: &str,
    #[case] offered: Vec<u32>,
) -> Result<(), ScriptError> {
    let creatures = [
        "The Wise Mothman",
        "Grizzly Bears",
        "Grizzly Bears",
        "Grizzly Bears",
    ];
    let scenario = || w13b_scenario(seats, top).battlefield_named(seats - 1, creatures);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let mut seen = Vec::new();
    w13b_play(&mut driver, &mut seen)?;
    assert_eq!(seen, offered);
    let grown: u32 = driver
        .state()
        .objects()
        .filter_map(|o| o.permanent())
        .map(|p| p.counters_of(&CounterKind::PlusOnePlusOne))
        .sum();
    assert_eq!(grown, offered.iter().sum::<u32>());
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn w13b_tato_farmer_takes_a_land_milled_this_turn(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || {
        w13b_scenario(seats, "LCC")
            .battlefield_named(0, ["Tato Farmer"])
            .graveyard_named(seats - 1, ["Forest"])
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.idle_until(2, Step::Main1)?;
    while !matches!(driver.decision()?.kind, DecisionKind::Priority(_)) {
        driver.answer_passively()?;
    }
    let milled = driver
        .state()
        .zone(Zone::Graveyard(PlayerId::from_index(0)))
        .iter()
        .copied()
        .find(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.card().name == "Forest")
        })
        .unwrap();
    driver.begin_activation("Tato Farmer")?;
    let DecisionKind::ChooseTargets(options) = &driver.decision()?.kind else {
        panic!("Tato Farmer asks for its target");
    };
    assert_eq!(options.legal.len(), 1, "only the land milled this turn");
    let target = options.legal[0];
    driver.choose_targets(&[target])?;
    let mut offered = Vec::new();
    w13b_play(&mut driver, &mut offered)?;
    let land = driver.state().object(milled);
    assert!(land.is_none_or(|o| o.zone() != Zone::Graveyard(PlayerId::from_index(0))));
    assert_eq!(
        driver
            .state()
            .objects()
            .filter(|o| o.card().name == "Forest" && o.zone() == Zone::Battlefield)
            .filter(|o| o
                .permanent()
                .is_some_and(mtg_engine::object::PermanentState::tapped))
            .count(),
        1
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::tapped(true)]
#[case::untapped(false)]
fn w13b_mariposa_enters_tapped_for_two_rad_counters(
    #[values(2, 3, 4)] seats: usize,
    #[case] yes: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        (1..seats).fold(
            Scenario::new(seats)
                .at(2, 0, Step::Main1)
                .hand_named(0, ["Mariposa Military Base"]),
            |scenario, seat| scenario.library_named(seat, std::iter::repeat_n("Grizzly Bears", 10)),
        )
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    driver.play_land("Mariposa Military Base")?;
    assert!(matches!(driver.decision()?.kind, DecisionKind::YesNo(_)));
    driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
    let base = driver.id_of("Mariposa Military Base")?;
    let tapped = driver
        .state()
        .object(base)
        .and_then(|o| o.permanent())
        .is_some_and(mtg_engine::object::PermanentState::tapped);
    let counters: Vec<u32> = (0..seats)
        .map(|seat| {
            driver
                .state()
                .player(PlayerId::from_index(seat))
                .map_or(0, |p| p.counters_of(&rad()))
        })
        .collect();
    assert_eq!(tapped, yes);
    let mut expected = vec![0; seats];
    expected[0] = if yes { 2 } else { 0 };
    assert_eq!(counters, expected);
    replays(&driver, &setup);
    Ok(())
}
