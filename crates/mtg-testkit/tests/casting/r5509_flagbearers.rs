//! Goal: a Flagbearer rule binds an opponent of its controller as they choose targets for a spell or activated ability:
//! at two to four seats a single target must be a Flagbearer if one can be chosen (any player's, as the Coalition Honor
//! Guard ruling says), the earlier of several specifications stay free, a specification of several targets asks the
//! Flagbearer first and then the rest, and the controller's own spells are not bound.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::choose::{Target, TargetOptions};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::events::Event;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature, mountain, plains};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// Seat 0 holds `spell` with three Mountains, two Forests and two Plains; the Flagbearer `bearer` is at seat
/// `bearer_seat` and every seat has a 1/1.
fn r5509_table(
    seats: usize,
    spell: &str,
    bearer: &str,
    bearer_seat: usize,
) -> Result<Driver, ScriptError> {
    let lands = vec![
        ready(mountain()),
        ready(mountain()),
        ready(mountain()),
        ready(forest()),
        ready(forest()),
        ready(plains()),
        ready(plains()),
    ];
    let mut scenario = Scenario::new(seats).hand(0, [card(spell).unwrap()]);
    for seat in 0..seats {
        let mut permanents = vec![ready(free_creature("Pip", 1, 1))];
        if seat == bearer_seat {
            permanents.push(ready(card(bearer).unwrap()));
        }
        if seat == 0 {
            permanents.extend(lands.clone());
        }
        scenario = scenario.battlefield(seat, permanents);
    }
    Driver::from_scenario(scenario.at(1, 0, Step::Main1))
}

fn r5509_named(driver: &Driver, name: &str) -> ObjectId {
    *driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .find(|id| driver.state().object(**id).unwrap().card().name == name)
        .unwrap()
}

fn r5509_object(driver: &Driver, id: ObjectId) -> Target {
    Target::Object(driver.state().object(id).unwrap().reference())
}

fn r5509_options(driver: &Driver) -> TargetOptions {
    match driver.decision().unwrap().kind.clone() {
        DecisionKind::ChooseTargets(options) => options,
        other => panic!("targets are asked, not {}", other.name()),
    }
}

#[rstest]
#[case::two_seats(2, 1, "Standard Bearer")]
#[case::three_seats(3, 2, "Coalition Honor Guard")]
#[case::four_seats(4, 3, "Standard Bearer")]
fn r5509_a_single_target_must_be_the_flagbearer(
    #[case] seats: usize,
    #[case] bearer_seat: usize,
    #[case] bearer: &str,
) -> Result<(), ScriptError> {
    let mut driver = r5509_table(seats, "Lightning Bolt", bearer, bearer_seat)?;
    driver.begin_cast("Lightning Bolt")?;
    let flag = r5509_object(&driver, r5509_named(&driver, bearer));
    let options = r5509_options(&driver);
    assert_eq!(options.legal, vec![flag], "only the Flagbearer is offered");
    assert_eq!((options.min, options.max), (1, 1));
    Ok(())
}

#[rstest]
#[case::two_seats(2)]
#[case::four_seats(4)]
fn r5509_the_controllers_own_spell_is_free(#[case] seats: usize) -> Result<(), ScriptError> {
    let mut driver = r5509_table(seats, "Lightning Bolt", "Standard Bearer", 0)?;
    driver.begin_cast("Lightning Bolt")?;
    let options = r5509_options(&driver);
    assert!(
        options.legal.len() > 1 + seats,
        "every creature and player is offered: {:?}",
        options.legal
    );
    assert!(
        options
            .legal
            .contains(&Target::Player(PlayerId::from_index(1)))
    );
    Ok(())
}

#[rstest]
#[case::two_seats(2, 1)]
#[case::three_seats(3, 1)]
#[case::four_seats(4, 2)]
fn r5509_only_the_last_specification_that_could_take_one_is_bound(
    #[case] seats: usize,
    #[case] bearer_seat: usize,
) -> Result<(), ScriptError> {
    let mut driver = r5509_table(seats, "Seeds of Strength", "Standard Bearer", bearer_seat)?;
    driver.begin_cast("Seeds of Strength")?;
    let flag = r5509_object(&driver, r5509_named(&driver, "Standard Bearer"));
    let pip = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .find(|id| driver.state().object(*id).unwrap().card().name == "Pip")
        .unwrap();
    let pip = r5509_object(&driver, pip);
    for _ in 0..2 {
        let options = r5509_options(&driver);
        assert!(options.legal.len() > 1, "an earlier specification is free");
        driver.choose_targets(&[pip])?;
    }
    let options = r5509_options(&driver);
    assert_eq!(
        options.legal,
        vec![flag],
        "the last one takes the Flagbearer"
    );
    Ok(())
}

#[rstest]
#[case::chosen_early_frees_the_last(2, 1)]
#[case::chosen_early_four_seats(4, 3)]
fn r5509_a_flagbearer_chosen_early_frees_the_rest(
    #[case] seats: usize,
    #[case] bearer_seat: usize,
) -> Result<(), ScriptError> {
    let mut driver = r5509_table(seats, "Seeds of Strength", "Standard Bearer", bearer_seat)?;
    driver.begin_cast("Seeds of Strength")?;
    let flag = r5509_object(&driver, r5509_named(&driver, "Standard Bearer"));
    driver.choose_targets(&[flag])?;
    driver.choose_targets(&[flag])?;
    let options = r5509_options(&driver);
    assert!(options.legal.len() > 1, "{:?}", options.legal);
    Ok(())
}

#[rstest]
#[case::two_seats(2, 1)]
#[case::three_seats(3, 2)]
#[case::four_seats(4, 1)]
fn r5509_several_targets_take_the_flagbearer_first_then_the_rest(
    #[case] seats: usize,
    #[case] bearer_seat: usize,
) -> Result<(), ScriptError> {
    let mut driver = r5509_table(seats, "Coordinated Assault", "Standard Bearer", bearer_seat)?;
    driver.begin_cast("Coordinated Assault")?;
    let flag = r5509_object(&driver, r5509_named(&driver, "Standard Bearer"));
    let options = r5509_options(&driver);
    assert_eq!(options.legal, vec![flag]);
    assert_eq!((options.min, options.max), (1, 1));
    driver.choose_targets(&[flag])?;
    let options = r5509_options(&driver);
    assert!(!options.legal.contains(&flag), "the Flagbearer is taken");
    assert_eq!((options.min, options.max), (0, 1));
    let other = options.legal[0];
    driver.choose_targets(&[other])?;
    let events = driver.answer(Answer::PayMana(PayChoice::Auto))?;
    let targets = events.iter().find_map(|event| match event {
        Event::SpellCast { choices, .. } => Some(choices.targets.clone()),
        _ => None,
    });
    assert_eq!(
        targets,
        Some(vec![vec![flag, other]]),
        "the specification holds both targets"
    );
    Ok(())
}
