//! Goal: the Commander format plays by its rules through the engine at 2 to 6 seats: commanders start in the command zone and not the library, the first mulligan is free and only a two-player game skips the first draw, a commander is cast from the command zone for its cost plus {2} for each earlier cast from there (and a cast from a hand adds nothing), it returns to the command zone as a new object from every zone its owner is asked about, a refusal leaves it where it went, and 21 combat damage from one commander eliminates a player at 40 life while 20 does not.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_core::color::Color;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_core::mana::ManaCost;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::{Answer, DecisionKind, MulliganChoice, MulliganOptions, PayChoice};
use mtg_engine::events::Event;
use mtg_engine::format::Commander;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_engine::player::LossReason;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_engine::{Engine, replay};
use mtg_testkit::cards::mountain;
use mtg_testkit::commander_cards::legend;
use mtg_testkit::{Driver, Scenario, ScriptError, commander_fuzz_setup, ready};
use rstest::rstest;

const SEAT: PlayerId = PlayerId(0);

/// A Commander table where seat 0 has the commander "Cmd" ({1}{R}) in its command zone and `lands` Mountains.
fn table(seats: usize, lands: usize) -> Scenario {
    Scenario::new(seats)
        .format(Arc::new(Commander))
        .commanders(0, [legend("Cmd", 1, &[Color::Red], 2, 2)])
        .battlefield(0, (0..lands).map(|_| ready(mountain())))
}

fn commander(driver: &Driver) -> ObjectId {
    driver.state().commanders().next().unwrap()
}

fn zone_of(driver: &Driver, id: ObjectId) -> Zone {
    driver.state().object(id).unwrap().zone()
}

/// Casts "Cmd" from where it is, pays for it and lets it resolve.
fn cast_and_resolve(driver: &mut Driver, seats: usize) -> Result<(), ScriptError> {
    driver.cast("Cmd")?;
    for _ in 0..seats {
        driver.pass()?;
    }
    assert_eq!(zone_of(driver, commander(driver)), Zone::Battlefield);
    Ok(())
}

fn say(driver: &mut Driver, yes: bool) -> Result<(), ScriptError> {
    assert!(
        matches!(driver.decision()?.kind, DecisionKind::YesNo(_)),
        "{:?}",
        driver.decision()?.kind
    );
    driver.answer(Answer::YesNo(YesNoChoice(yes)))?;
    Ok(())
}

fn move_by_hand(driver: &mut Driver, to: ManualZone) -> Result<(), ScriptError> {
    let object = commander(driver);
    driver.manual(ManualAction::Move { object, to })?;
    Ok(())
}

#[rstest]
fn every_seat_starts_with_40_life_and_the_commander_in_its_command_zone(
    #[values(2, 3, 4, 6)] seats: usize,
) {
    let driver = Driver::from_scenario(table(seats, 0)).unwrap();
    let id = commander(&driver);
    let state = driver.state();
    assert!(state.players().iter().all(|p| p.life() == 40));
    assert_eq!(state.zone(Zone::Command(SEAT)), [id]);
    assert!(state.is_commander(id));
    assert_eq!(state.commanders().count(), 1);
    assert!(
        state
            .zone(Zone::Library(SEAT))
            .iter()
            .all(|card| *card != id)
    );
    assert_eq!((state.commander_casts(id), state.commander_tax(id)), (0, 0));
}

fn mulligan_options(driver: &Driver) -> MulliganOptions {
    match &driver.decision().unwrap().kind {
        DecisionKind::Mulligan(options) => options.clone(),
        other => panic!("not a mulligan: {other:?}"),
    }
}

#[rstest]
fn the_first_mulligan_is_free_and_the_second_costs_a_card(#[values(2, 4)] seats: usize) {
    let mut driver =
        Driver::new(Engine::new(commander_fuzz_setup(seats, 3).unwrap()).unwrap()).unwrap();
    let first = mulligan_options(&driver);
    assert_eq!(first.hand.len(), 7);
    assert_eq!(
        (
            first.free_mulligans,
            first.bottom_if_keep,
            first.bottom_if_mulligan
        ),
        (1, 0, 0)
    );
    // Everyone who is asked mulligans once, so the next round asks them again.
    for _ in 0..seats {
        driver
            .answer(Answer::Mulligan(MulliganChoice::Mulligan))
            .unwrap();
    }
    let second = mulligan_options(&driver);
    assert_eq!(second.mulligans_taken, 1);
    assert_eq!((second.bottom_if_keep, second.bottom_if_mulligan), (0, 1));
}

#[rstest]
#[case::two_players_skip_it(2, 7)]
#[case::three_players_draw(3, 8)]
#[case::six_players_draw(6, 8)]
fn only_a_two_player_game_skips_the_first_draw(#[case] seats: usize, #[case] cards_in_hand: usize) {
    let mut driver =
        Driver::new(Engine::new(commander_fuzz_setup(seats, 5).unwrap()).unwrap()).unwrap();
    driver.idle_until(1, Step::Main1).unwrap();
    let first = driver.state().first_player();
    assert_eq!(driver.state().zone(Zone::Hand(first)).len(), cards_in_hand);
}

fn cost(text: &str) -> ManaCost {
    ManaCost::parse(text).unwrap()
}

#[rstest]
#[case::first_cast(0, "{1}{R}")]
#[case::after_one(1, "{3}{R}")]
#[case::after_two(2, "{5}{R}")]
#[case::after_three(3, "{7}{R}")]
fn the_tax_is_two_for_each_earlier_cast_from_the_command_zone(
    #[case] earlier: u32,
    #[case] total: &str,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, 24))?;
    for _ in 0..earlier {
        cast_and_resolve(&mut driver, 2)?;
        move_by_hand(&mut driver, ManualZone::Graveyard)?;
        say(&mut driver, true)?;
        assert_eq!(zone_of(&driver, commander(&driver)), Zone::Command(SEAT));
    }
    let id = commander(&driver);
    assert_eq!(driver.state().commander_casts(id), earlier);
    driver.begin_cast("Cmd")?;
    assert_eq!(driver.pay_options()?.cost, cost(total));
    Ok(())
}

#[rstest]
#[case::one_mana_short(5, false)]
#[case::just_enough(6, true)]
fn a_commander_is_offered_only_when_the_tax_can_be_paid(
    #[case] lands: usize,
    #[case] offered: bool,
) -> Result<(), ScriptError> {
    // The first cast taps two of the lands; the second costs {3}{R} with the tax, so four must be left.
    let mut driver = Driver::from_scenario(table(2, lands))?;
    cast_and_resolve(&mut driver, 2)?;
    move_by_hand(&mut driver, ManualZone::Graveyard)?;
    say(&mut driver, true)?;
    let DecisionKind::Priority(options) = &driver.decision()?.kind else {
        panic!("priority expected");
    };
    assert_eq!(options.castable.contains(&commander(&driver)), offered);
    Ok(())
}

#[rstest]
#[case::graveyard(ManualZone::Graveyard)]
#[case::exile(ManualZone::Exile)]
#[case::hand(ManualZone::Hand)]
#[case::library_top(ManualZone::LibraryTop)]
#[case::library_bottom(ManualZone::LibraryBottom)]
fn the_owner_may_put_a_commander_into_the_command_zone_instead(
    #[case] to: ManualZone,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(3, 2))?;
    cast_and_resolve(&mut driver, 3)?;
    let id = commander(&driver);
    let before = driver.state().object(id).unwrap().reference();
    move_by_hand(&mut driver, to)?;
    say(&mut driver, true)?;
    assert_eq!(zone_of(&driver, id), Zone::Command(SEAT));
    let after = driver.state().object(id).unwrap().reference();
    assert_eq!(after.id, id);
    assert!(after.counter > before.counter, "it is a new object");
    assert!(driver.state().is_commander(id));
    // One cast, so one tax, wherever it came back from.
    assert_eq!(driver.state().commander_tax(id), 2);
    Ok(())
}

#[rstest]
#[case::graveyard(ManualZone::Graveyard, Zone::Graveyard(SEAT), true)]
#[case::exile(ManualZone::Exile, Zone::Exile, true)]
#[case::hand(ManualZone::Hand, Zone::Hand(SEAT), false)]
#[case::library_top(ManualZone::LibraryTop, Zone::Library(SEAT), false)]
#[case::library_bottom(ManualZone::LibraryBottom, Zone::Library(SEAT), false)]
fn a_refusal_leaves_the_commander_where_it_went(
    #[case] to: ManualZone,
    #[case] lands_in: Zone,
    #[case] stays_remembered: bool,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, 2))?;
    cast_and_resolve(&mut driver, 2)?;
    let id = commander(&driver);
    move_by_hand(&mut driver, to)?;
    say(&mut driver, false)?;
    assert_eq!(zone_of(&driver, id), lands_in);
    // It is not asked again where it lies: the next decision is the priority of the active player.
    assert!(matches!(driver.decision()?.kind, DecisionKind::Priority(_)));
    let stayed = driver
        .engine()
        .events()
        .iter()
        .any(|event| matches!(event, Event::CommanderStayed { object } if *object == id));
    assert_eq!(stayed, stays_remembered);
    Ok(())
}

#[test]
fn a_commander_from_a_hand_costs_no_tax_and_counts_no_cast() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(2, 6))?;
    cast_and_resolve(&mut driver, 2)?;
    move_by_hand(&mut driver, ManualZone::Hand)?;
    say(&mut driver, false)?;
    let id = commander(&driver);
    driver.begin_cast("Cmd")?;
    assert_eq!(driver.pay_options()?.cost, cost("{1}{R}"));
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    for _ in 0..2 {
        driver.pass()?;
    }
    assert_eq!(driver.state().commander_casts(id), 1);
    Ok(())
}

#[rstest]
#[case::twenty_is_not_enough(20, false)]
#[case::twenty_one_is(21, true)]
fn twenty_one_combat_damage_from_one_commander_eliminates_a_player(
    #[case] power: i32,
    #[case] eliminated: bool,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(2)
        .format(Arc::new(Commander))
        .commanders(0, [legend("Cmd", 0, &[], power, power)]);
    let mut driver = Driver::from_scenario(scenario)?;
    cast_and_resolve(&mut driver, 2)?;
    driver.idle_until(3, Step::DeclareAttackers)?;
    driver.attack(&[("Cmd", 1)])?;
    driver.idle_until(3, Step::EndCombat).ok();
    let foe = driver.state().player(PlayerId(1)).unwrap();
    assert_eq!(foe.life(), 40 - power);
    assert_eq!(
        foe.status().clone(),
        if eliminated {
            mtg_engine::player::PlayerStatus::Lost(LossReason::CommanderDamage)
        } else {
            mtg_engine::player::PlayerStatus::InGame
        }
    );
    let id = commander(&driver);
    assert_eq!(
        driver.state().commander_damage(id, PlayerId(1)),
        power as u32
    );
    Ok(())
}

#[test]
fn a_commander_game_replays_to_the_live_state() -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(table(3, 14))?;
    for _ in 0..2 {
        cast_and_resolve(&mut driver, 3)?;
        move_by_hand(&mut driver, ManualZone::Exile)?;
        say(&mut driver, true)?;
    }
    let rebuilt = replay(&table(3, 14).setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(rebuilt.state_hash(), driver.state().state_hash());
    assert_eq!(rebuilt.commander_casts(commander(&driver)), 2);
    Ok(())
}
