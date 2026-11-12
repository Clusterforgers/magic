//! Goal: a card that would be put into a graveyard goes into its owner's library instead, at tables of two to four
//! seats: destroyed, discarded or milled it is shuffled in (and stays in the library when milled), a creature that would
//! die goes on top or on the bottom, an instant that resolves is shuffled in and the game goes on, and each log replays to
//! the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::choose::{Target, TargetChoice};
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::zones::Zone;
use mtg_engine::{GameSetup, replay};
use mtg_testkit::cards::{bear, instant};
use mtg_testkit::trigger_cards::artifact_with;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

use crate::common::pid;

fn ability(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// "If ~ would be put into a graveyard from anywhere, reveal ~ and shuffle it into its owner's library instead."
const SHUFFLED: &str = r#"{"Replacement":{"by":{"IntoLibrary":{"place":"Shuffled","reveal":true}},"when":{"ZoneChange":{"to":"Graveyard","what":["This"]}}}}"#;

/// "If ~ would die, put it on top of (or on the bottom of) its owner's library instead."
fn dies_into(place: &str) -> String {
    format!(
        r#"{{"Replacement":{{"by":{{"IntoLibrary":{{"place":"{place}"}}}},"when":{{"ZoneChange":{{"from":"Battlefield","to":"Graveyard","what":["This"]}}}}}}}}"#
    )
}

/// A 2/2 creature named `Titan` with the replacement.
fn titan(rule: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Titan")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_rules(ability(rule)),
    )
}

/// An artifact with "{T}: `effects`" aimed at `targets`.
fn rig(effects: &str, targets: &str) -> Arc<CardDefinition> {
    artifact_with(
        "Rig",
        vec![ability(&format!(
            r#"{{"Activated":{{"cost":["Tap"],"effects":{effects},"targets":{targets}}}}}"#
        ))],
    )
}

/// Activates the rig, aims at `aim` when asked, and passes until the stack is empty.
fn run(driver: &mut Driver, aim: Option<Target>) -> Result<(), ScriptError> {
    driver.begin_activation("Rig")?;
    settle(driver, aim)
}

fn settle(driver: &mut Driver, aim: Option<Target>) -> Result<(), ScriptError> {
    for _ in 0..60 {
        let decision = driver.decision()?.clone();
        if matches!(decision.kind, DecisionKind::Priority(_))
            && driver.state().stack().is_empty()
            && driver.state().pending_triggers().is_empty()
        {
            return Ok(());
        }
        let answer = match (&decision.kind, aim) {
            (DecisionKind::ChooseTargets(_), Some(aim)) => {
                Answer::ChooseTargets(TargetChoice::Targets(vec![aim]))
            }
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer)?;
    }
    Err(ScriptError::Stuck("a finished resolution".to_owned()))
}

fn names_in(driver: &Driver, zone: Zone) -> Vec<String> {
    driver
        .state()
        .zone(zone)
        .iter()
        .map(|id| driver.state().object(*id).unwrap().card().name.clone())
        .collect()
}

fn replays(driver: &Driver, setup: &GameSetup) {
    let replayed = replay(setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

/// How the card would reach the graveyard.
#[derive(Debug, Clone, Copy)]
enum Way {
    Destroyed,
    Discarded,
    Milled,
}

#[rstest]
fn a_card_from_anywhere_is_shuffled_in(
    #[values(2, 3, 4)] seats: usize,
    #[values(Way::Destroyed, Way::Discarded, Way::Milled)] way: Way,
) -> Result<(), ScriptError> {
    let library: Vec<_> = (0..5).map(|_| bear()).collect();
    let mut scenario = Scenario::new(seats);
    let mut mine = Vec::new();
    let (effects, targets) = match way {
        Way::Destroyed => {
            mine.push(ready(titan(SHUFFLED)));
            (
                r#"[{"Destroy":{"what":{"Target":0}}}]"#,
                r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
            )
        }
        Way::Discarded => {
            scenario = scenario.hand(0, vec![titan(SHUFFLED)]);
            (r#"[{"Discard":{"count":1}}]"#, "[]")
        }
        Way::Milled => {
            let mut cards = vec![titan(SHUFFLED)];
            cards.extend(library.iter().cloned());
            scenario = scenario.library(0, cards);
            (r#"[{"Mill":{"count":3}}]"#, "[]")
        }
    };
    if !matches!(way, Way::Milled) {
        scenario = scenario.library(0, library);
    }
    mine.push(ready(rig(effects, targets)));
    let scenario = scenario.battlefield(0, mine);
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let aim = match way {
        Way::Destroyed => Some(driver.target_named("Titan")?),
        _ => None,
    };
    run(&mut driver, aim)?;
    let me = pid(0);
    assert!(
        !names_in(&driver, Zone::Graveyard(me)).contains(&"Titan".to_owned()),
        "{way:?}"
    );
    let in_library = names_in(&driver, Zone::Library(me));
    assert_eq!(
        in_library.iter().filter(|name| *name == "Titan").count(),
        1,
        "{way:?}"
    );
    let expected = match way {
        // Six cards, three milled, and the milled card that stays.
        Way::Milled => 4,
        _ => 6,
    };
    assert_eq!(in_library.len(), expected, "{way:?}");
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn a_creature_that_would_die_goes_on_top_or_bottom(
    #[values(2, 3, 4)] seats: usize,
    #[values("Top", "Bottom")] place: &str,
) -> Result<(), ScriptError> {
    let scenario = Scenario::new(seats)
        .library(0, (0..4).map(|_| bear()))
        .battlefield(
            0,
            vec![
                ready(titan(&dies_into(place))),
                ready(rig(
                    r#"[{"Destroy":{"what":{"Target":0}}}]"#,
                    r#"[{"what":{"Object":[{"Type":"Creature"}]}}]"#,
                )),
            ],
        );
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    let aim = driver.target_named("Titan")?;
    run(&mut driver, Some(aim))?;
    // The library lies bottom to top.
    let library = names_in(&driver, Zone::Library(pid(0)));
    let at = library.iter().position(|name| name == "Titan");
    let wanted = if place == "Top" { library.len() - 1 } else { 0 };
    assert_eq!(at, Some(wanted), "{library:?}");
    replays(&driver, &setup);
    Ok(())
}

#[rstest]
fn an_instant_that_resolves_is_shuffled_in(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let card = (*instant("Fate", 0, &[]))
        .clone()
        .with_rules(ability(
            r#"{"Spell":{"effects":[{"GainLife":{"amount":1}}]}}"#,
        ))
        .with_rules(ability(SHUFFLED));
    let scenario = Scenario::new(seats)
        .hand(0, vec![Arc::new(card)])
        .library(0, (0..4).map(|_| bear()));
    let setup = scenario.clone().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Fate")?;
    settle(&mut driver, None)?;
    let me = pid(0);
    assert!(names_in(&driver, Zone::Library(me)).contains(&"Fate".to_owned()));
    assert_eq!(names_in(&driver, Zone::Graveyard(me)), Vec::<String>::new());
    assert_eq!(driver.state().stack().len(), 0);
    replays(&driver, &setup);
    Ok(())
}
