//! Goal: a token copy that enters tapped and attacking (Flamerush Rider copies another attacker and the copy is exiled at end of combat; Loki, the Deceiver copies a Villain that is not legendary and is an Illusion, sacrificed at the end step) and a copy of an artifact creature chosen first (March of Progress), at tables of two, three and four seats, replayed from the log.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::{CardType, Supertype};
use mtg_core::color::Color;
use mtg_core::ids::ObjectId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::object::ObjectKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{creature, island};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::copying::{replays, settle};

/// A creature of the bear's size with the extra type and subtype.
fn bear(name: &str, artifact: bool, subtype: Option<&str>, legendary: bool) -> Arc<CardDefinition> {
    let mut definition = (*creature(name, 1, &[Color::Red], 2, 2)).clone();
    if artifact {
        definition = definition.with_type(CardType::Artifact);
    }
    if let Some(subtype) = subtype {
        definition = definition.with_subtype(subtype);
    }
    if legendary {
        definition = definition.with_supertype(Supertype::Legendary);
    }
    Arc::new(definition)
}

/// The permanents of the battlefield named `name`, in id order.
fn named(driver: &Driver, name: &str) -> Vec<ObjectId> {
    let mut found: Vec<ObjectId> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| driver.state().object(*id).unwrap().card().name == name)
        .collect();
    found.sort();
    found
}

/// The token among the permanents named `name`.
fn token_named(driver: &Driver, name: &str) -> Option<ObjectId> {
    named(driver, name)
        .into_iter()
        .find(|id| driver.state().object(*id).unwrap().kind() == ObjectKind::Token)
}

/// Seat 0 attacks seat 1 with `attackers` (ready on the battlefield) and the trigger of the first takes `target`.
fn attack_with_trigger(
    seats: usize,
    attackers: &[Arc<CardDefinition>],
    target: &str,
) -> Result<(Driver, mtg_engine::GameSetup), ScriptError> {
    let names: Vec<String> = attackers
        .iter()
        .map(|definition| definition.name.clone())
        .collect();
    let build = || {
        let mut scenario =
            Scenario::new(seats).battlefield(0, attackers.iter().cloned().map(ready));
        for seat in 1..seats {
            scenario = scenario.battlefield(seat, [ready(bear("Blocker", false, None, false))]);
        }
        scenario
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.pass_until(Step::DeclareAttackers)?;
    let chosen: Vec<(&str, usize)> = names.iter().map(|name| (name.as_str(), 1)).collect();
    driver.attack(&chosen)?;
    if matches!(driver.decision()?.kind, DecisionKind::ChooseTargets(_)) {
        let target = driver.target_named(target)?;
        driver.choose_targets(&[target])?;
    }
    settle(&mut driver)?;
    Ok((driver, setup))
}

#[rstest]
fn flamerush_rider_copies_another_attacker_tapped_and_attacking_and_exiles_it_at_end_of_combat(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let rider = card("Flamerush Rider").unwrap();
    let friend = bear("Friend", false, None, false);
    let (mut driver, setup) = attack_with_trigger(seats, &[rider, friend], "Friend")?;
    let copy = token_named(&driver, "Friend").expect("a token copy of the other attacker");
    assert!(
        driver
            .state()
            .object(copy)
            .unwrap()
            .permanent()
            .unwrap()
            .tapped()
    );
    assert_eq!(
        driver.state().combat().attackers().len(),
        3,
        "the copy attacks with the two declared attackers"
    );
    assert!(token_named(&driver, "Flamerush Rider").is_none());
    replays(&driver, &setup);
    let turn = driver.state().turn().number();
    driver.idle_until(turn, Step::Main2)?;
    assert!(
        token_named(&driver, "Friend").is_none(),
        "exiled at end of combat"
    );
    assert_eq!(named(&driver, "Friend").len(), 1, "the original stays");
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn loki_copies_a_villain_that_is_not_legendary_and_an_illusion_and_sacrifices_it_at_the_end_step(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let loki = card("Loki, the Deceiver").unwrap();
    let villain = bear("Master Villain", false, Some("Villain"), true);
    let (mut driver, setup) = attack_with_trigger(seats, &[loki, villain], "Master Villain")?;
    let copy = token_named(&driver, "Master Villain").expect("a token copy of the Villain");
    let object = driver.state().object(copy).unwrap();
    assert!(object.permanent().unwrap().tapped());
    let values = object.card();
    assert!(!values.supertypes.contains(&Supertype::Legendary));
    assert!(
        values
            .subtypes
            .iter()
            .any(|subtype| subtype.0 == "Illusion")
    );
    assert!(values.subtypes.iter().any(|subtype| subtype.0 == "Villain"));
    replays(&driver, &setup);
    let turn = driver.state().turn().number();
    driver.idle_until(turn + 1, Step::Upkeep)?;
    assert!(
        token_named(&driver, "Master Villain").is_none(),
        "sacrificed at the end step"
    );
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn march_of_progress_copies_an_artifact_creature_the_caster_chose_and_no_other_creature(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let build = || {
        let mut scenario = Scenario::new(seats)
            .hand_named(0, ["March of Progress"])
            .battlefield(
                0,
                (0..3)
                    .map(|_| ready(island()))
                    .chain([
                        ready(bear("Own Robot", true, None, false)),
                        ready(bear("Own Bear", false, None, false)),
                    ])
                    .collect::<Vec<_>>(),
            );
        for seat in 1..seats {
            scenario =
                scenario.battlefield(seat, [ready(bear("Foreign Robot", true, None, false))]);
        }
        scenario
    };
    let setup = build().setup()?;
    let mut driver = Driver::from_scenario(build())?;
    driver.begin_cast("March of Progress")?;
    assert!(driver.target_named("Own Robot").is_ok());
    let robot = driver.target_named("Own Robot")?;
    driver.choose_targets(&[robot])?;
    driver.pay_tapping(&["Island", "Island", "Island"], 0)?;
    settle(&mut driver)?;
    assert_eq!(
        named(&driver, "Own Robot").len(),
        2,
        "the robot and its copy"
    );
    assert!(token_named(&driver, "Own Robot").is_some());
    assert_eq!(
        named(&driver, "Own Bear").len(),
        1,
        "a creature that is no artifact is no target"
    );
    assert_eq!(named(&driver, "Foreign Robot").len(), seats - 1);
    replays(&driver, &setup);
    Ok(())
}
