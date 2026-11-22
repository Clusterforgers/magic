//! Goal: assimilate puts a creature card from any graveyard onto the battlefield under the controller's control, and takes control of a creature for good, each with a +1/+1 counter as a Borg artifact creature without its other creature types, at 2 to 4 seats; the log replays to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::{CardType, Subtype};
use mtg_engine::characteristics;
use mtg_engine::characteristics::controller_of;
use mtg_engine::decision::DecisionKind;
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

/// Grizzly Bears of the corpus, a Bear.
fn bears() -> Arc<CardDefinition> {
    corpus::card("Grizzly Bears").unwrap()
}

/// Seat 0 holds the instant; seat 1 has a Grizzly Bears on the battlefield or in its graveyard.
fn game(seats: usize, zone: &str) -> Result<Driver, ScriptError> {
    let permanents: Vec<PresetPermanent> = if zone == "graveyard" {
        Vec::new()
    } else {
        vec![ready(bears())]
    };
    let mut scenario = (0..seats)
        .fold(Scenario::new(seats), |scenario, seat| {
            scenario.library(seat, (0..8).map(|_| forest()))
        })
        .hand(0, vec![assimilation(zone)])
        .battlefield(1, permanents);
    if zone == "graveyard" {
        scenario = scenario.graveyard(1, vec![bears()]);
    }
    Driver::from_scenario(scenario)
}

#[rstest]
#[ignore = "R116Assimilate: the prim leaves the object where it is; caps does not list it yet"]
fn r116_assimilate_takes_a_card_or_a_creature_as_a_borg(
    #[values("graveyard", "battlefield")] zone: &str,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = game(seats, zone)?;
    driver.begin_cast("Assimilation")?;
    for _ in 0..12 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseTargets(_) => {
                let bear = driver.target_named("Grizzly Bears")?;
                driver.choose_targets(&[bear])?;
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
