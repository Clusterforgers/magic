//! Goal: pack e17b's shapes play whole at two, three and four seats: exiling until a land with the card found and the
//! rest exiled under names of their own puts the land onto the battlefield (or leaves it in exile) and only the other
//! cards on the bottom of the library; a return whose card enters with counters when of a kind gives a creature its
//! counters and an artifact none; and the game replays from its log to the same state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_engine::choose::Target;
use mtg_engine::decision::{Answer, DecisionKind, PayChoice};
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, creature, forest};
use mtg_testkit::{Driver, Scenario, ScriptError};
use rstest::rstest;

use crate::common::{names, pid};

/// A free sorcery with these effects and targets.
fn sorcery(name: &str, effects: &str, targets: &str) -> Arc<CardDefinition> {
    let json = format!(r#"{{"Spell":{{"effects":{effects}{targets}}}}}"#);
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[]))
            .with_rules(serde_json::from_str(&json).unwrap()),
    )
}

const DIG: &str = r#"{"RevealUntil":{"as":"found","filter":[{"Type":"Land"}],"revealed_as":"revealed2"}},{"Exile":{"as":"found_moved3","what":{"Bound":"found"}}},{"Exile":{"as":"rest_moved4","what":{"Bound":"revealed2"}}}"#;
const ONTO: &str = r#"{"Move":{"to":{"Battlefield":{}},"what":{"Bound":"found_moved3"}}}"#;
const REST: &str = r#"{"Move":{"to":{"Library":{"order":"Random","place":"Bottom"}},"what":{"Each":[{"Is":{"Bound":"rest_moved4"}},{"In":"Exile"}]}}}"#;

/// A library from a spec, top first: `l` is a Forest, `c` a creature named for its place.
fn library_of(spec: &str) -> Vec<Arc<CardDefinition>> {
    spec.chars()
        .enumerate()
        .map(|(at, kind)| match kind {
            'l' => forest(),
            _ => creature(&format!("Creature {at}"), 9, &[], 1, 1),
        })
        .collect()
}

/// Passes until the stack is empty, answering every other question passively.
fn settle(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..100 {
        match driver.decision()?.kind {
            DecisionKind::Priority(_)
                if driver.state().stack().is_empty() && driver.state().resolving().is_none() =>
            {
                return Ok(());
            }
            DecisionKind::Priority(_) => {
                driver.pass()?;
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

fn replays(driver: &Driver) {
    let replayed = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

fn count(names: &[String], name: &str) -> usize {
    names.iter().filter(|held| *held == name).count()
}

#[rstest]
#[case::two_before_the_land(2, "ccl", true)]
#[case::the_first_card(3, "lcc", true)]
#[case::kept_in_exile(4, "cclc", false)]
#[case::no_land(2, "ccc", true)]
fn the_card_found_and_the_rest_go_their_own_ways(
    #[case] seats: usize,
    #[case] spec: &str,
    #[case] onto: bool,
) -> Result<(), ScriptError> {
    let effects = if onto {
        format!("[{DIG},{ONTO},{REST}]")
    } else {
        format!("[{DIG},{REST}]")
    };
    let table = Scenario::new(seats)
        .hand(0, [sorcery("Dig", &effects, "")])
        .library(0, library_of(spec));
    let mut driver = Driver::from_scenario(table)?;
    let lands_before = count(&names(driver.state(), Zone::Battlefield), "Forest");
    driver.cast("Dig")?;
    settle(&mut driver)?;
    let state = driver.state();
    let found = spec.find('l');
    let revealed = found.map_or(spec.len(), |at| at + 1);
    let library = names(state, Zone::Library(pid(0)));
    let exile = names(state, Zone::Exile);
    let battlefield = count(&names(state, Zone::Battlefield), "Forest");
    match (found, onto) {
        (Some(_), true) => {
            assert_eq!(battlefield, lands_before + 1, "the land found enters");
            assert!(exile.is_empty(), "nothing stays in exile");
        }
        (Some(_), false) => {
            assert_eq!(
                exile,
                vec!["Forest".to_owned()],
                "only the card found stays exiled"
            );
        }
        (None, _) => assert_eq!(exile, Vec::<String>::new()),
    }
    assert_eq!(
        count(&library, "Forest"),
        0,
        "the card found never goes to the bottom"
    );
    let others = spec[..revealed].chars().filter(|kind| *kind != 'l').count();
    assert_eq!(library.len(), spec.len() - revealed + others);
    replays(&driver);
    Ok(())
}

const RETURNED: &str = r#"[{"If":{"condition":{"CountOf":{"op":"Ge","right":1,"selector":[{"Type":"Creature"}],"subject":{"Target":0}}},"otherwise":[{"Move":{"to":{"Battlefield":{}},"what":{"Target":0}}}],"then":[{"Move":{"to":{"Battlefield":{"counters":[["+1/+1",2]]}},"what":{"Target":0}}}]}}]"#;
const FROM_GRAVEYARD: &str = r#","targets":[{"what":{"Object":[{"AnyOf":[{"Type":"Artifact"},{"Type":"Creature"}]},{"OwnedBy":"You"},{"In":"Graveyard"}]}}]"#;

fn relic() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Relic")
            .with_type(CardType::Artifact)
            .with_mana_cost(cost(1, &[])),
    )
}

#[rstest]
#[case::a_creature(2, "Bear", 2)]
#[case::an_artifact(3, "Relic", 0)]
#[case::at_four_seats(4, "Bear", 2)]
fn a_returned_creature_gets_its_counters(
    #[case] seats: usize,
    #[case] name: &str,
    #[case] counters: u32,
) -> Result<(), ScriptError> {
    let table = Scenario::new(seats)
        .hand(0, [sorcery("Return", RETURNED, FROM_GRAVEYARD)])
        .graveyard(0, [creature("Bear", 2, &[], 2, 2), relic()]);
    let mut driver = Driver::from_scenario(table)?;
    let id = driver
        .state()
        .zone(Zone::Graveyard(pid(0)))
        .iter()
        .copied()
        .find(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.card().name == name)
        })
        .unwrap();
    let target = Target::Object(driver.state().object(id).unwrap().reference());
    driver.begin_cast("Return")?;
    driver.choose_targets(&[target])?;
    settle(&mut driver)?;
    let state = driver.state();
    let returned = state
        .zone(Zone::Battlefield)
        .iter()
        .filter_map(|id| state.object(*id))
        .find(|object| object.card().name == name)
        .unwrap_or_else(|| {
            panic!(
                "the card returned: battlefield {:?}, graveyard {:?}, stack {:?}",
                names(state, Zone::Battlefield),
                names(state, Zone::Graveyard(pid(0))),
                names(state, Zone::Stack)
            )
        });
    assert_eq!(
        returned
            .permanent()
            .map_or(0, |p| p.counters_of(&CounterKind::PlusOnePlusOne)),
        counters
    );
    replays(&driver);
    Ok(())
}
