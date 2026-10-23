//! Goal: "If you control a commander, you may cast this spell without paying its mana cost." is an alternative cost offered only while the caster controls a commander on the battlefield (not with it in the command zone, not without a commander), announced beside the mana cost and paid with no mana while the cost increases still apply; strive adds the mana of its cost again for each target beyond the first, which is announced once the targets are chosen; at 2 to 4 seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::casting::{choose_way, lands, pay, ways};
use crate::common::on_battlefield;
use mtg_core::color::Color;
use mtg_core::mana::ManaCost;
use mtg_engine::choose::Target;
use mtg_engine::decision::DecisionKind;
use mtg_engine::format::Commander;
use mtg_engine::zones::Zone;
use mtg_engine::{Event, PresetPermanent, replay};
use mtg_testkit::cards::{bear, forest, mountain, swamp};
use mtg_testkit::commander_cards::legend;
use mtg_testkit::cost_cards::tax_rock;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A Commander table where seat 0 holds `hand`, has the commander "Cmd" ({1}{R}) in its command zone, two Mountains for it and
/// `swamps` Swamps, and seat 1 has a Grizzly Bears and the `extra` permanents.
fn table(seats: usize, hand: &[&str], swamps: usize, extra: Vec<PresetPermanent>) -> Scenario {
    let mut mine = lands(2, mountain);
    mine.extend(lands(swamps, swamp));
    let mut theirs = vec![ready(bear())];
    theirs.extend(extra);
    Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [legend("Cmd", 1, &[Color::Red], 2, 2)])
        .hand_named(0, hand.iter().copied())
        .battlefield(0, mine)
        .battlefield(1, theirs)
}

/// Casts the commander from the command zone and lets it resolve.
fn commander_in_play(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    driver.cast("Cmd")?;
    for _ in 0..seats {
        driver.pass()?;
    }
    assert!(on_battlefield(driver.state(), "Cmd").is_some());
    Ok(())
}

/// The permanents called `name`, as targets.
fn named(driver: &Driver, name: &str) -> Vec<Target> {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .filter(|object| object.card().name == name)
        .map(|object| Target::Object(object.reference()))
        .collect()
}

/// How many permanents became tapped since the log had `from` events.
fn tapped_since(driver: &Driver, from: usize) -> usize {
    driver.engine().events()[from..]
        .iter()
        .filter(|event| matches!(event, Event::PermanentTapped { .. }))
        .count()
}

/// Casts `name` at `count` of the permanents called `target`, choosing the way that starts with `way` when it is asked, and
/// pays when payment is asked. The labels of the ways asked are returned.
fn cast_at(
    driver: &mut Driver,
    name: &str,
    target: &str,
    count: usize,
    way: &str,
) -> Result<Vec<String>, ScriptError> {
    let mut asked = Vec::new();
    driver.begin_cast(name)?;
    for _ in 0..6 {
        match &driver.decision()?.kind {
            DecisionKind::ChooseOption(_) => {
                asked = ways(driver)?;
                choose_way(driver, way)?;
            }
            DecisionKind::ChooseTargets(_) => {
                let targets: Vec<_> = named(driver, target).into_iter().take(count).collect();
                driver.choose_targets(&targets)?;
            }
            kind if kind.name() == "pay-mana" => {
                pay(driver)?;
            }
            _ => break,
        }
    }
    for _ in 0..8 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass()?;
    }
    Ok(asked)
}

/// The log replays to the state the game is in.
fn assert_replays(driver: &Driver) {
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
}

fn cost(text: &str) -> ManaCost {
    ManaCost::parse(text).unwrap()
}

#[rstest]
fn a_commander_on_the_battlefield_opens_the_free_cast_and_one_in_the_command_zone_does_not(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] in_play: bool,
) -> Result<(), ScriptError> {
    // Two Mountains and nothing else: once the commander is cast no mana is left, and without it {3}{B} is out of reach.
    let mut driver = Driver::from_scenario(table(seats, &["Deadly Rollick"], 0, Vec::new()))?;
    if in_play {
        commander_in_play(&mut driver, seats)?;
    }
    if !in_play {
        assert!(driver.begin_cast("Deadly Rollick").is_err());
        return Ok(());
    }
    let from = driver.engine().events().len();
    cast_at(
        &mut driver,
        "Deadly Rollick",
        "Grizzly Bears",
        1,
        "Cast without",
    )?;
    assert!(on_battlefield(driver.state(), "Grizzly Bears").is_none());
    assert_eq!(tapped_since(&driver, from), 0, "no mana was paid");
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn a_game_without_a_commander_has_no_free_cast(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .hand_named(0, ["Deadly Rollick"])
            .battlefield_named(1, ["Grizzly Bears"]),
    )?;
    assert!(driver.begin_cast("Deadly Rollick").is_err());
    Ok(())
}

#[rstest]
#[case::free("Cast without", 0)]
#[case::for_the_mana_cost("Cast for", 4)]
fn the_free_cast_is_announced_beside_the_mana_cost(
    #[values(2, 3, 4)] seats: usize,
    #[case] way: &str,
    #[case] tapped: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(seats, &["Deadly Rollick"], 4, Vec::new()))?;
    commander_in_play(&mut driver, seats)?;
    let from = driver.engine().events().len();
    let asked = cast_at(&mut driver, "Deadly Rollick", "Grizzly Bears", 1, way)?;
    assert_eq!(asked.len(), 2, "{asked:?}");
    assert!(asked.iter().any(|label| label == "Cast for {3}{B}"));
    assert!(
        asked
            .iter()
            .any(|label| label == "Cast without paying its mana cost")
    );
    assert_eq!(tapped_since(&driver, from), tapped);
    assert!(on_battlefield(driver.state(), "Grizzly Bears").is_none());
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn a_cost_increase_still_applies_to_a_free_cast(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] a_land_pays_it: bool,
) -> Result<(), ScriptError> {
    // Noncreature spells cost {1} more: the free cast costs {1}, which needs a land that is not tapped for the commander.
    let rock = ready(tax_rock().unwrap());
    let swamps = usize::from(a_land_pays_it);
    let mut driver = Driver::from_scenario(table(seats, &["Deadly Rollick"], swamps, vec![rock]))?;
    commander_in_play(&mut driver, seats)?;
    if !a_land_pays_it {
        assert!(driver.begin_cast("Deadly Rollick").is_err());
        return Ok(());
    }
    driver.begin_cast("Deadly Rollick")?;
    let targets = named(&driver, "Grizzly Bears");
    driver.choose_targets(&targets)?;
    assert_eq!(driver.pay_options()?.cost, cost("{1}"));
    Ok(())
}

/// Seat 0 holds Consign to Dust and `forests` Forests; seat 1 has three Sol Rings to destroy.
fn strive_table(seats: usize, forests: usize) -> Scenario {
    Scenario::new(seats)
        .hand_named(0, ["Consign to Dust"])
        .battlefield(0, lands(forests, forest))
        .battlefield_named(1, ["Sol Ring", "Sol Ring", "Sol Ring"])
}

#[rstest]
#[case::no_target_costs_the_base(0, "{2}{G}")]
#[case::one_target_costs_the_base(1, "{2}{G}")]
#[case::two_targets_cost_it_twice(2, "{4}{G}{G}")]
#[case::three_targets_cost_it_three_times(3, "{6}{G}{G}{G}")]
fn strive_adds_its_cost_for_each_target_beyond_the_first(
    #[values(2, 3, 4)] seats: usize,
    #[case] targets: usize,
    #[case] total: &str,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(strive_table(seats, 12))?;
    driver.begin_cast("Consign to Dust")?;
    let chosen: Vec<_> = named(&driver, "Sol Ring")
        .into_iter()
        .take(targets)
        .collect();
    driver.choose_targets(&chosen)?;
    assert_eq!(driver.pay_options()?.cost, cost(total));
    pay(&mut driver)?;
    for _ in 0..seats {
        driver.pass()?;
    }
    assert_eq!(named(&driver, "Sol Ring").len(), 3 - targets);
    assert_replays(&driver);
    Ok(())
}

#[rstest]
fn a_target_that_makes_the_total_unaffordable_rewinds_the_announcement(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    // Five Forests pay {2}{G} for one target; a second target makes it {4}{G}{G}.
    let mut driver = Driver::from_scenario(strive_table(seats, 5))?;
    let before = (driver.engine().events().len(), driver.engine().state_hash());
    driver.begin_cast("Consign to Dust")?;
    let two: Vec<_> = named(&driver, "Sol Ring").into_iter().take(2).collect();
    driver.choose_targets(&two)?;
    driver.expect_decision(0, "priority")?;
    assert_eq!(
        (driver.engine().events().len(), driver.engine().state_hash()),
        before,
        "nothing was paid and nothing was logged"
    );
    Ok(())
}
