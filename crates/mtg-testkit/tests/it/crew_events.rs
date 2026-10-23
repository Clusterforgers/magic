//! Goal: crewing a Vehicle and saddling a Mount log one `Crewed` event for the permanent with the creatures tapped to pay, the controller and whether it was a saddle, and trigger "becomes crewed" and "becomes saddled" abilities once for each activation or only the first time each turn, a crew never triggers a saddle ability and the other way round, and the log of the games replays to the live state, at tables of two, three and four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::Event;
use mtg_engine::choose::CardsChoice;
use mtg_engine::decision::{Answer, DecisionKind};
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::free_creature;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const ME: PlayerId = PlayerId(0);

/// A free creature `name` with `rules`, a Vehicle with Crew 1 when `vehicle`, a Mount with Saddle 1 otherwise.
fn rig(name: &str, vehicle: bool, trigger: &str) -> Arc<CardDefinition> {
    let rule = |json: &str| serde_json::from_str::<Ability>(json).unwrap();
    let watch = rule(&format!(
        r#"{{"Triggered":{{"when":{trigger},"effects":[{{"GainLife":{{"amount":1}}}}]}}}}"#
    ));
    let card = CardDefinition::new(name).with_power_toughness(4, 4);
    let card = if vehicle {
        card.with_subtype("Vehicle")
            .with_type(CardType::Artifact)
            .with_rules(rule(r#"{"Keyword":{"Crew":1}}"#))
    } else {
        card.with_subtype("Mount")
            .with_type(CardType::Creature)
            .with_rules(rule(r#"{"Keyword":{"Saddle":1}}"#))
    };
    Arc::new(card.with_rules(watch))
}

/// Answers the questions of an activation in progress (the first creatures it takes) and lets it resolve.
fn finish(driver: &mut Driver) -> Result<(), ScriptError> {
    for _ in 0..40 {
        match driver.decision()?.kind.clone() {
            DecisionKind::ChooseCards(options) => {
                let ids = options.from.iter().take(options.min as usize).copied();
                driver.answer(Answer::ChooseCards(CardsChoice(ids.collect())))?;
            }
            _ if driver.state().stack().is_empty() && driver.state().resolving().is_none() => {
                return Ok(());
            }
            _ => {
                driver.answer_passively()?;
            }
        }
    }
    Err(ScriptError::Stuck("the activation".into()))
}

/// Activates the only ability `name` offers (its crew or saddle) and lets it and what it triggers resolve.
fn crew(driver: &mut Driver, name: &str) -> Result<(), ScriptError> {
    let offered = driver.offered_abilities(name)?;
    let [index] = offered.as_slice() else {
        return Err(ScriptError::Stuck(format!("{name} offers {offered:?}")));
    };
    driver.begin_ability(name, *index)?;
    finish(driver)
}

fn crewed_events(driver: &Driver) -> Vec<Event> {
    driver
        .engine()
        .events()
        .iter()
        .filter(|event| matches!(event, Event::Crewed { .. }))
        .cloned()
        .collect()
}

fn life(driver: &Driver) -> i32 {
    driver.state().player(ME).unwrap().life()
}

fn assert_replays(driver: &Driver, scenario: &Scenario) {
    let replayed = replay(&scenario.clone().setup().unwrap(), driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.state().state_hash());
}

const CREWED: &str = r#"{"BecomesCrewed":{"what":["This"]}}"#;
const CREWED_FIRST: &str = r#"{"Nth":{"event":{"BecomesCrewed":{"what":["This"]}},"nth":1}}"#;
const SADDLED: &str = r#"{"BecomesSaddled":{"what":["This"]}}"#;
const SADDLED_FIRST: &str = r#"{"Nth":{"event":{"BecomesSaddled":{"what":["This"]}},"nth":1}}"#;

#[rstest]
#[case::every_crew(CREWED, 2)]
#[case::the_first_crew_of_the_turn(CREWED_FIRST, 1)]
#[case::a_saddle_pattern_never_answers_a_crew(SADDLED, 0)]
fn a_vehicle_becomes_crewed_by_the_creature_tapped_and_triggers_what_watches(
    #[case] trigger: &str,
    #[case] gained: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mine = vec![
        ready(rig("Skiff", true, trigger)),
        ready(free_creature("Rider A", 1, 1)),
        ready(free_creature("Rider B", 1, 1)),
    ];
    let scenario = Scenario::new(seats).battlefield(0, mine);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    let before = life(&driver);
    crew(&mut driver, "Skiff")?;
    let events = crewed_events(&driver);
    let [
        Event::Crewed {
            object,
            player,
            creatures,
            saddle: false,
        },
    ] = events.as_slice()
    else {
        panic!("one crew: {events:?}");
    };
    assert_eq!((*object, *player), (driver.id_of("Skiff")?, ME));
    assert_eq!(creatures.len(), 1, "the creature tapped to pay");
    assert!(
        [driver.id_of("Rider A")?, driver.id_of("Rider B")?].contains(&creatures[0]),
        "a Rider paid"
    );
    // The Vehicle is a creature and is crewed again by the other Rider, at instant speed.
    let first = life(&driver) - before;
    assert_eq!(first, i32::from(gained > 0));
    crew(&mut driver, "Skiff")?;
    assert_eq!(crewed_events(&driver).len(), 2);
    assert_eq!(life(&driver) - before, gained);
    assert_replays(&driver, &scenario);
    Ok(())
}

#[rstest]
#[case::every_saddle(SADDLED, 1)]
#[case::the_first_saddle_of_the_turn(SADDLED_FIRST, 1)]
#[case::a_crew_pattern_never_answers_a_saddle(CREWED, 0)]
fn a_mount_becomes_saddled_by_the_creature_tapped_and_triggers_what_watches(
    #[case] trigger: &str,
    #[case] gained: i32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mine = vec![
        ready(rig("Pony", false, trigger)),
        ready(free_creature("Rider", 1, 1)),
    ];
    let scenario = Scenario::new(seats).battlefield(0, mine);
    let mut driver = Driver::from_scenario(scenario.clone())?;
    driver.pass_until(Step::Main1)?;
    let before = life(&driver);
    crew(&mut driver, "Pony")?;
    let events = crewed_events(&driver);
    let [
        Event::Crewed {
            object,
            player,
            creatures,
            saddle: true,
        },
    ] = events.as_slice()
    else {
        panic!("one saddle: {events:?}");
    };
    assert_eq!((*object, *player), (driver.id_of("Pony")?, ME));
    assert_eq!(creatures, &[driver.id_of("Rider")?]);
    assert_eq!(life(&driver) - before, gained);
    assert_replays(&driver, &scenario);
    Ok(())
}
