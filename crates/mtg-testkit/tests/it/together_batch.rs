//! Goal: the moves of one resolving effect written as a `Together` batch (a loop that destroys each creature) are one batch for "whenever one or more creatures you control die", which fires once, while the same loop without the batch fires once per pass; at two to four seats, and the log replays to the same game.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::engine::Engine;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, free_creature, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

const EACH_DIES: &str = r#"{"RepeatEach":{"over":{"Each":[{"Type":"Creature"},{"ControlledBy":"You"}]},"as":"c","effects":[{"Destroy":{"what":{"Bound":"c"}}}]}}"#;

fn json(text: &str) -> Ability {
    serde_json::from_str(text).unwrap()
}

/// A sorcery that destroys each creature its caster controls, one pass for each, batched or not.
fn cull(together: bool) -> Arc<CardDefinition> {
    let effects = if together {
        format!(r#"{{"Together":{{"effects":[{EACH_DIES}]}}}}"#)
    } else {
        EACH_DIES.to_owned()
    };
    Arc::new(
        CardDefinition::new("Cull")
            .with_type(CardType::Sorcery)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(json(&format!(r#"{{"Spell":{{"effects":[{effects}]}}}}"#))),
    )
}

/// An artifact: "Whenever one or more creatures you control die, you gain 1 life."
fn altar() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Altar")
            .with_type(CardType::Artifact)
            .with_rules(json(
                r#"{"Triggered":{"when":{"Batch":{"ZoneChange":{"what":[{"Type":"Creature"},{"ControlledBy":"You"}],"from":"Battlefield","to":"Graveyard"}}},"effects":[{"GainLife":{"amount":1}}]}}"#,
            )),
    )
}

fn settle(driver: &mut Driver) {
    for _ in 0..80 {
        let idle = driver.state().stack().is_empty() && driver.state().resolving().is_none();
        if idle && matches!(driver.decision().unwrap().kind, DecisionKind::Priority(_)) {
            return;
        }
        driver.answer_passively().unwrap();
    }
    panic!("the stack never emptied");
}

/// Seat 0 casts Cull with an Altar and three creatures, every other seat has a creature too. Returns seat 0's life gain and the driver.
fn play(seats: usize, together: bool) -> (i32, Driver) {
    let mut scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .hand(0, [cull(together)])
        .battlefield(
            0,
            [
                ready(mountain()),
                ready(altar()),
                ready(free_creature("Bear", 2, 2)),
                ready(free_creature("Cat", 1, 1)),
                ready(free_creature("Ox", 2, 3)),
            ],
        );
    for seat in 1..seats {
        scenario = scenario.battlefield(seat, [ready(free_creature("Wolf", 2, 2))]);
    }
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let me = PlayerId::from_index(0);
    let before = driver.state().player(me).unwrap().life();
    driver.begin_cast("Cull").unwrap();
    driver.pay_tapping(&["Mountain"], 0).unwrap();
    driver.pass().unwrap();
    settle(&mut driver);
    let gained = driver.state().player(me).unwrap().life() - before;
    (gained, driver)
}

#[rstest]
fn a_batch_of_deaths_in_a_loop_triggers_once(#[values(2, 3, 4)] seats: usize) {
    let (gained, driver) = play(seats, true);
    assert_eq!(gained, 1, "one trigger for the three creatures");
    let engine = driver.engine();
    let rebuilt = Engine::resume(engine.setup().clone(), engine.events().to_vec()).unwrap();
    assert_eq!(rebuilt.state_hash(), engine.state_hash());
}

#[rstest]
fn the_same_loop_without_the_batch_triggers_per_pass(#[values(2, 3, 4)] seats: usize) {
    let (gained, _) = play(seats, false);
    assert_eq!(gained, 3);
}

#[rstest]
fn the_batch_plays_the_same_game_twice(#[values(2, 3, 4)] seats: usize) {
    let (_, first) = play(seats, true);
    let (_, second) = play(seats, true);
    assert_eq!(first.engine().events(), second.engine().events());
}

#[rstest]
fn a_game_rebuilt_in_the_middle_of_the_batch_still_triggers_once(#[values(2, 4)] seats: usize) {
    use mtg_engine::events::Event;
    let (_, driver) = play(seats, true);
    let engine = driver.engine();
    let events = engine.events();
    let begun = events
        .iter()
        .position(|e| matches!(e, Event::ResolutionBegan { .. }))
        .unwrap();
    let cuts: Vec<usize> = (begun..events.len())
        .filter(|at| matches!(events[*at], Event::StepDone { .. }))
        .map(|at| at + 1)
        .collect();
    assert!(cuts.len() >= 4, "the loop takes several steps");
    let me = PlayerId::from_index(0);
    // The last cut ends the batch, whose triggers the log holds after it: only a cut inside the batch is a stop of the game.
    let triggered = events
        .iter()
        .position(|e| matches!(e, Event::AbilityTriggered(_)))
        .unwrap();
    for cut in cuts.iter().filter(|cut| **cut < triggered) {
        let resumed = Engine::resume(engine.setup().clone(), events[..*cut].to_vec()).unwrap();
        let before = resumed.state().player(me).unwrap().life();
        let mut again = Driver::new(resumed).unwrap();
        settle(&mut again);
        let gained = again.state().player(me).unwrap().life() - before;
        assert_eq!(gained, 1, "rebuilt after event {cut}");
    }
}
