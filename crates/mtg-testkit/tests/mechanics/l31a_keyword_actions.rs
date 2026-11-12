//! Goal: investigate, discover, manifest dread and a gift log who performed them at tables of two to four seats, and
//! the cards that wait for them play as they read: Erdwal Illuminator investigates again for the first investigation
//! of a turn only (never for a Clue token made without investigating, nor for an opponent's), Curator of Sun's Creation
//! discovers again for the same value once each turn, Paranormal Analyst returns the card manifest dread put into the
//! graveyard, and Jolly Gerbils draws when its controller gives a gift.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::effect::discover;
use mtg_cards::value::Amount;
use mtg_core::card::CardType;
use mtg_engine::choose::{CostsChoice, Target};
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::events::Event;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{forest, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::casting::lands;
use crate::common::{name_of, pid};

/// An artifact named `name` with "{0}: <steps>".
fn tool(name: &str, steps: &str) -> Arc<CardDefinition> {
    let rule = format!(r#"{{"Activated":{{"cost":[],"effects":{steps}}}}}"#);
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_rules(serde_json::from_str::<Ability>(&rule).unwrap()),
    )
}

/// "{0}: Investigate."
fn magnifier() -> Arc<CardDefinition> {
    tool(
        "Magnifier",
        r#"[{"CreateToken":{"token":{"Named":"clue"}}},{"Performed":{"action":"Investigated"}}]"#,
    )
}

/// "{0}: Create a Clue token."
fn clue_maker() -> Arc<CardDefinition> {
    tool(
        "Clue Maker",
        r#"[{"CreateToken":{"token":{"Named":"clue"}}}]"#,
    )
}

/// "{0}: Discover 2."
fn seeker() -> Arc<CardDefinition> {
    tool(
        "Seeker",
        &serde_json::to_string(&discover(Amount::Fixed(2))).unwrap(),
    )
}

/// "{0}: Manifest dread."
fn dreader() -> Arc<CardDefinition> {
    tool(
        "Dreader",
        r#"[{"LookAtTop":{"count":2,"as":"looked"}},{"Choose":{"from":{"Bound":"looked"},"as":"kept","rest_as":"rest"}},{"Move":{"what":{"Bound":"rest"},"to":"Graveyard"}},{"Manifest":{}},{"Performed":{"action":"ManifestedDread","card":{"Bound":"rest"}}}]"#,
    )
}

/// Answers passively until nothing is on the stack and nothing waits.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..120 {
        if driver.state().stack().is_empty()
            && driver.state().resolving().is_none()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        driver.answer_passively()?;
    }
    Err(ScriptError::Stuck("the stack".into()))
}

/// The events of the log that match.
fn logged(driver: &Driver, wanted: impl Fn(&Event) -> bool) -> usize {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| wanted(event))
        .count()
}

/// The permanents named `name` that seat `seat` controls.
fn controlled(driver: &Driver, seat: usize, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| {
            name_of(state, **id) == name
                && state
                    .object(**id)
                    .is_some_and(|object| object.controller() == pid(seat))
        })
        .count()
}

/// Seat `by` activates `tool` `times` times; seat 0 controls Erdwal Illuminator.
#[rstest]
#[case::once(0, "Magnifier", 1, 2, 1)]
#[case::twice_a_turn(0, "Magnifier", 2, 3, 1)]
#[case::a_clue_made_otherwise(0, "Clue Maker", 1, 1, 0)]
#[case::an_opponent_investigates(1, "Magnifier", 1, 0, 0)]
fn erdwal_investigates_again_for_the_first_investigation_of_a_turn(
    #[case] by: usize,
    #[case] tool: &str,
    #[case] times: usize,
    #[case] clues: usize,
    #[case] extra: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let tools = || vec![ready(magnifier()), ready(clue_maker())];
    let mut mine = tools();
    mine.push(ready(card("Erdwal Illuminator").unwrap()));
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .battlefield(0, mine)
            .battlefield(1, tools()),
    )?;
    if by == 1 {
        driver.pass()?;
    }
    for _ in 0..times {
        driver.begin_ability(tool, 0)?;
        settle(&mut driver)?;
    }
    assert_eq!(controlled(&driver, 0, "Clue"), clues);
    let investigations = logged(
        &driver,
        |event| matches!(event, Event::Investigated { player } if *player == pid(by)),
    );
    let investigated = if tool == "Magnifier" { times } else { 0 };
    assert_eq!(investigations, investigated + extra);
    Ok(())
}

/// Seat 0 controls Curator of Sun's Creation and discovers 2 `times` times in one turn.
#[rstest]
#[case::once(1, 2)]
#[case::twice(2, 3)]
fn curator_discovers_again_for_the_same_value_once_each_turn(
    #[case] times: usize,
    #[case] discovers: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .library(
                0,
                (0..10).map(|n| free_creature(&format!("Bear {n}"), 2, 2)),
            )
            .battlefield(
                0,
                [
                    ready(seeker()),
                    ready(card("Curator of Sun's Creation").unwrap()),
                ],
            ),
    )?;
    for _ in 0..times {
        driver.begin_ability("Seeker", 0)?;
        settle(&mut driver)?;
    }
    let values: Vec<u32> = driver
        .engine()
        .events()
        .iter()
        .filter_map(|event| match event {
            Event::Discovered { player, value, .. } if *player == pid(0) => Some(*value),
            _ => None,
        })
        .collect();
    assert_eq!(values, vec![2; discovers], "the same value each time");
    Ok(())
}

/// Seat 0 controls Paranormal Analyst (or not) and manifests dread from a library of three cards.
#[rstest]
#[case::with_the_analyst(true, 1, 0)]
#[case::without(false, 0, 1)]
fn paranormal_analyst_returns_the_card_manifest_dread_put_into_the_graveyard(
    #[case] analyst: bool,
    #[case] drawn: usize,
    #[case] buried: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut mine = vec![ready(dreader())];
    if analyst {
        mine.push(ready(card("Paranormal Analyst").unwrap()));
    }
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .library(0, (0..3).map(|n| free_creature(&format!("Bear {n}"), 2, 2)))
            .battlefield(0, mine),
    )?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.begin_ability("Dreader", 0)?;
    settle(&mut driver)?;
    assert_eq!(driver.state().zone(Zone::Hand(pid(0))).len() - hand, drawn);
    assert_eq!(driver.state().zone(Zone::Graveyard(pid(0))).len(), buried);
    let card = driver
        .engine()
        .events()
        .iter()
        .find_map(|event| match event {
            Event::DreadManifested { player, card } if *player == pid(0) => Some(*card),
            _ => None,
        });
    assert!(
        matches!(card, Some(Some(_))),
        "the dread names the card it buried"
    );
    Ok(())
}

/// Seat 0 controls Jolly Gerbils and casts Wear Down on two artifacts with the gift promised to seat 1, or on one
/// without the promise.
#[rstest]
#[case::promised(true, 1)]
#[case::not_promised(false, 0)]
fn jolly_gerbils_draws_when_its_controller_gives_a_gift(
    #[case] promised: bool,
    #[case] drawn: usize,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut permanents = lands(2, forest);
    permanents.push(ready(card("Bonesplitter").unwrap()));
    permanents.push(ready(card("Jolly Gerbils").unwrap()));
    let scenario = (0..seats).fold(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand_named(0, ["Wear Down"])
            .battlefield(0, permanents)
            .battlefield(1, [ready(card("Bonesplitter").unwrap())]),
        |scenario, seat| scenario.library(seat, (0..8).map(|_| forest())),
    );
    let mut driver = Driver::from_scenario(scenario)?;
    let hand = driver.state().zone(Zone::Hand(pid(0))).len();
    driver.begin_cast("Wear Down")?;
    let DecisionKind::ChooseCosts(options) = &driver.decision()?.kind else {
        panic!("the gift is asked");
    };
    let costs = if promised {
        let offered = options
            .optional
            .iter()
            .find(|cost| cost.label.ends_with("to player 2"))
            .unwrap();
        vec![offered.index]
    } else {
        Vec::new()
    };
    driver.answer(Answer::ChooseCosts(CostsChoice::Costs(costs)))?;
    let artifacts: Vec<Target> = driver
        .state()
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| name_of(driver.state(), **id) == "Bonesplitter")
        .map(|id| Target::Object(driver.state().object(*id).unwrap().reference()))
        .take(if promised { 2 } else { 1 })
        .collect();
    for pick in &artifacts {
        driver.choose_targets(&[*pick])?;
    }
    driver.expect_decision(0, "pay-mana")?;
    driver.answer(Answer::PayMana(PayChoice::Auto))?;
    settle(&mut driver)?;
    // Wear Down left the hand; the gerbils drew for the gift.
    assert_eq!(
        driver.state().zone(Zone::Hand(pid(0))).len() + 1 - hand,
        drawn
    );
    let gifts = logged(
        &driver,
        |event| matches!(event, Event::GiftGiven { player, to } if *player == pid(0) && *to == pid(1)),
    );
    assert_eq!(gifts, drawn);
    Ok(())
}
