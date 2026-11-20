//! Goal: abilities that trigger on a triggered ability triggering (CR 603.2) at 2 to 4 seats: Strict Proctor counters
//! an ability a permanent entering made trigger unless its controller pays {2} (paid, declined, unable), Aboleth Spawn
//! copies the entering trigger of an opponent's creature for its own controller and asks nothing for its own creature,
//! The Masamune makes the dies triggers of the equipped creature and of
//! its controller's emblems trigger again, and each log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_engine::choose::{TargetChoice, YesNoChoice};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::w6b_emblems::emblem_definition;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, mountain};
use mtg_testkit::corpus::card;
use mtg_testkit::scenario::attached_to;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::replays;
use crate::common::pid;

/// Seat 0 holds Elvish Visionary with `lands` Forests and five cards in its library; `watcher` sits on seat `at`'s
/// battlefield; every seat has a library to draw from.
fn table(seats: usize, watcher: &str, at: usize, lands: usize) -> Scenario {
    let mut lands = vec![ready(forest()); lands];
    let watcher = ready(card(watcher).unwrap());
    let mut scenario = Scenario::new(seats).hand(0, [card("Elvish Visionary").unwrap()]);
    if at == 0 {
        lands.push(watcher);
    } else {
        scenario = scenario.battlefield(at, [watcher]);
    }
    scenario = scenario.battlefield(0, lands);
    for seat in 0..seats {
        scenario = scenario.library(seat, (0..5).map(|_| forest()).collect::<Vec<_>>());
    }
    scenario
}

/// Casts Elvish Visionary and plays on until the stack is empty, answering each yes-or-no with `yes` and paying
/// what a yes asks for with the engine's payment; returns how many yes-or-no questions were asked.
fn cast_visionary(driver: &mut Driver, yes: bool) -> Result<usize, ScriptError> {
    driver.cast("Elvish Visionary")?;
    let mut asked = 0;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(asked),
            DecisionKind::YesNo(options) => {
                asked += 1;
                driver.answer(Answer::YesNo(YesNoChoice(yes && !options.only_no)))?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the stack".into()))
}

fn hand(driver: &Driver, seat: usize) -> usize {
    driver.state().zone(Zone::Hand(pid(seat))).len()
}

#[rstest]
#[case::paid(4, true, 1)]
#[case::declined(4, false, 0)]
#[case::unable(2, true, 0)]
fn strict_proctor_counters_an_entering_trigger_unless_its_controller_pays(
    #[values(2, 3, 4)] seats: usize,
    #[case] lands: usize,
    #[case] pays: bool,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let at = seats - 1;
    let scenario = || table(seats, "Strict Proctor", at, lands);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let before = hand(&driver, 0);
    let asked = cast_visionary(&mut driver, pays)?;
    assert_eq!(
        asked, 1,
        "the controller of the entering trigger is asked once"
    );
    assert_eq!(hand(&driver, 0), before - 1 + drawn);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
#[case::copied(true, 1)]
#[case::not_copied(false, 0)]
fn aboleth_spawn_copies_an_opponents_entering_trigger(
    #[values(2, 3, 4)] seats: usize,
    #[case] copies: bool,
    #[case] drawn: usize,
) -> Result<(), ScriptError> {
    let at = seats - 1;
    let scenario = || table(seats, "Aboleth Spawn", at, 2);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let (before, theirs) = (hand(&driver, 0), hand(&driver, at));
    let asked = cast_visionary(&mut driver, copies)?;
    assert_eq!(asked, 1);
    // The original draws for the caster; the copy for Aboleth Spawn's controller.
    assert_eq!(hand(&driver, 0), before);
    assert_eq!(hand(&driver, at), theirs + drawn);
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn aboleth_spawn_ignores_its_controllers_own_creatures(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let scenario = || table(seats, "Aboleth Spawn", 0, 2);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let before = hand(&driver, 0);
    let asked = cast_visionary(&mut driver, true)?;
    assert_eq!(asked, 0, "no copy is offered");
    assert_eq!(hand(&driver, 0), before);
    replays(&driver, &setup);
    Ok(())
}

/// An emblem whose ability gains its owner 1 life whenever a creature dies.
fn mourning_emblem() -> Arc<CardDefinition> {
    let ability: Ability = serde_json::from_str(
        r#"{"Triggered":{"from":"Command","when":{"ZoneChange":{"what":[{"Type":"Creature"}],"from":"Battlefield","to":"Graveyard"}},"effects":[{"GainLife":{"amount":1}}]}}"#,
    )
    .unwrap();
    Arc::new(emblem_definition("Mourning", &[ability]))
}

/// Seat 0 controls Blood Artist (equipped with The Masamune when `equipped`), owns a mourning emblem when `emblem`, and
/// holds Lightning Bolt with a Mountain; the last seat controls Grizzly Bears.
fn masamune_table(seats: usize, equipped: bool, emblem: bool) -> Scenario {
    let mut field = vec![ready(card("Blood Artist").unwrap()), ready(mountain())];
    if equipped {
        field.push(attached_to(ready(card("The Masamune").unwrap()), 0, 0));
    }
    let mut scenario = Scenario::new(seats)
        .hand_named(0, ["Lightning Bolt"])
        .battlefield(0, field)
        .battlefield_named(seats - 1, ["Grizzly Bears"]);
    if emblem {
        scenario = scenario.emblems(0, [mourning_emblem()]);
    }
    scenario
}

#[rstest]
#[case::plain(false, false, 1)]
#[case::equipped(true, false, 2)]
#[case::emblem_plain(false, true, 2)]
#[case::emblem_equipped(true, true, 4)]
fn the_masamune_makes_dies_triggers_of_its_creature_and_your_emblems_trigger_again(
    #[values(2, 3, 4)] seats: usize,
    #[case] equipped: bool,
    #[case] emblem: bool,
    #[case] gained: i32,
) -> Result<(), ScriptError> {
    let scenario = || masamune_table(seats, equipped, emblem);
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    let life = driver.state().player(pid(0)).unwrap().life();
    let bears = driver.target_named("Grizzly Bears")?;
    driver.cast_at("Lightning Bolt", &[bears])?;
    for _ in 0..200 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_)) && driver.state().stack().is_empty() {
            break;
        }
        // Blood Artist aims at the last seat.
        if let DecisionKind::ChooseTargets(_) = decision.kind {
            let target = driver.target_player(seats - 1);
            driver.answer(Answer::ChooseTargets(TargetChoice::Targets(vec![target])))?;
            continue;
        }
        driver.answer_passively()?;
    }
    assert_eq!(driver.state().player(pid(0)).unwrap().life(), life + gained);
    replays(&driver, &setup);
    Ok(())
}
