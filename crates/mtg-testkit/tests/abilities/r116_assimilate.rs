//! Goal: assimilate puts a creature card from any graveyard, or one chosen among the cards exiled with the source, onto the battlefield under the controller's control, and takes control of a creature for good, each with a +1/+1 counter as a Borg artifact creature without its other creature types, at 2 to 4 seats; the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, Subtype};
use mtg_engine::characteristics;
use mtg_engine::characteristics::controller_of;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::{PresetPermanent, replay};
use mtg_testkit::cards::forest;
use mtg_testkit::corpus;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

/// A free instant that assimilates one target creature card in a graveyard, or one target creature.
fn assimilation(zone: &str) -> Arc<CardDefinition> {
    let what = if zone == "graveyard" {
        r#"[{"Type":"Creature"},{"In":"Graveyard"}]"#
    } else {
        r#"[{"Type":"Creature"}]"#
    };
    let json = format!(
        r#"{{"Spell":{{"targets":[{{"what":{{"Object":{what}}}}}],"effects":[{{"R116Assimilate":{{"what":{{"Target":0}}}}}}]}}}}"#
    );
    let rules: Ability = serde_json::from_str(&json).unwrap();
    Arc::new(
        CardDefinition::new("Assimilation")
            .with_type(CardType::Instant)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_rules(rules),
    )
}

/// An artifact with "{0}: Exile target creature card from a graveyard." and "{0}: Assimilate a creature card exiled
/// with it." (a choice among the cards it exiled, CR 607.2a).
fn assimilator() -> Arc<CardDefinition> {
    let exile: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":[],"targets":[{"what":{"Object":[{"Type":"Creature"},{"In":"Graveyard"}]}}],
            "effects":[{"Exile":{"what":{"Target":0}}}]}}"#,
    )
    .unwrap();
    let assimilate: Ability = serde_json::from_str(
        r#"{"Activated":{"cost":[],"effects":[
            {"Choose":{"from":{"Each":[{"Type":"Creature"},{"In":"Exile"},{"ExiledWith":"This"}]},"as":"assimilated"}},
            {"R116Assimilate":{"what":{"Bound":"assimilated"}}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Assimilator")
            .with_type(CardType::Artifact)
            .with_mana_cost(mtg_testkit::cards::cost(0, &[]))
            .with_rules(exile)
            .with_rules(assimilate),
    )
}

/// Grizzly Bears of the corpus, a Bear.
fn bears() -> Arc<CardDefinition> {
    corpus::card("Grizzly Bears").unwrap()
}

/// Seat 0 holds the instant (or controls the Assimilator); seat 1 has a Grizzly Bears on the battlefield or in its
/// graveyard.
fn game(seats: usize, zone: &str) -> Result<Driver, ScriptError> {
    let theirs: Vec<PresetPermanent> = if zone == "battlefield" {
        vec![ready(bears())]
    } else {
        Vec::new()
    };
    let mut scenario = (0..seats).fold(Scenario::new(seats), |scenario, seat| {
        scenario.library(seat, (0..8).map(|_| forest()))
    });
    scenario = if zone == "exiled" {
        scenario.battlefield(0, vec![ready(assimilator())])
    } else {
        scenario.hand(0, vec![assimilation(zone)])
    };
    scenario = scenario.battlefield(1, theirs);
    if zone != "battlefield" {
        scenario = scenario.graveyard(1, vec![bears()]);
    }
    Driver::from_scenario(scenario)
}

/// Answers the decisions until the stack is empty again, aiming at the Grizzly Bears.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..12 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseTargets(options) => {
                let bear = options
                    .legal
                    .iter()
                    .find(|target| {
                        target
                            .object()
                            .and_then(|object| driver.state().object(object.id))
                            .is_some_and(|object| object.card().name == "Grizzly Bears")
                    })
                    .copied()
                    .unwrap();
                driver.choose_targets(&[bear])?;
            }
            DecisionKind::PayMana(_) => {
                driver.answer(Answer::PayMana(PayChoice::Auto))?;
            }
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return Ok(()),
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

#[rstest]
fn r116_assimilate_takes_a_card_or_a_creature_as_a_borg(
    #[values("graveyard", "battlefield", "exiled")] zone: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, zone)?;
    if zone == "exiled" {
        driver.begin_ability("Assimilator", 0)?;
        settle(&mut driver)?;
        assert!(driver.id_of("Grizzly Bears").is_err());
        driver.begin_ability("Assimilator", 1)?;
    } else {
        driver.begin_cast("Assimilation")?;
    }
    settle(&mut driver)?;
    let id = driver.id_of("Grizzly Bears").unwrap();
    let state = driver.state();
    assert_eq!(controller_of(state, id), Some(pid(0)));
    let counters = state
        .object(id)
        .unwrap()
        .permanent()
        .unwrap()
        .counters_of(&CounterKind::PlusOnePlusOne);
    assert_eq!(counters, 1);
    let chars = characteristics(state, id).unwrap();
    assert!(chars.types.contains(CardType::Artifact));
    assert!(chars.types.contains(CardType::Creature));
    assert_eq!(chars.subtypes, [Subtype("Borg".to_owned())]);
    assert_eq!((chars.power, chars.toughness), (Some(3), Some(3)));
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
    Ok(())
}
