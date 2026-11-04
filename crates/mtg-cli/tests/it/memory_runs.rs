//! Goal: the shapes the translator writes for remembered objects play through: a count of what a step moved reads the zone it went to, the controller of an exiled permanent is read as it was, and a delayed trigger keeps the creature its spell targeted.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::{Answer, DecisionKind, PriorityAction};
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, burn, cost, forest, mountain};
use mtg_testkit::{Driver, Scenario, ready};

/// A one-red-mana instant whose rules are `json`, as the translator writes them.
fn instant(name: &str, json: &str) -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(json).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(rules),
    )
}

/// Passes until the stack is empty again, answering every question with the first choice.
fn settle(driver: &mut Driver) {
    for _ in 0..200 {
        let decision = driver.decision().unwrap().clone();
        let answer = match &decision.kind {
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => return,
            DecisionKind::Priority(_) => Answer::Priority(PriorityAction::Pass),
            _ => Driver::passive_answer(&decision),
        };
        driver.answer(answer).unwrap();
    }
    panic!("the game does not settle");
}

fn in_hand(driver: &Driver, seat: usize) -> usize {
    driver
        .state()
        .zone(Zone::Hand(PlayerId::from_index(seat)))
        .len()
}

const EVERY_ZONE: &str = r#"{"AnyOf":[{"In":"Battlefield"},{"In":"Hand"},{"In":"Library"},{"In":"Graveyard"},{"In":"Exile"},{"In":"Stack"}]}"#;

#[test]
fn the_count_of_what_a_destroy_moved_reads_the_graveyard() {
    let wipe = instant(
        "Wipe",
        &format!(
            r#"{{"Spell":{{"effects":[
            {{"Destroy":{{"as":"destroyed","what":{{"Each":[{{"Type":"Creature"}}]}}}}}},
            {{"Draw":{{"count":{{"Count":[{{"Is":{{"Bound":"destroyed"}}}},{EVERY_ZONE}]}}}}}}]}}}}"#
        ),
    );
    let scenario = Scenario::new(2)
        .hand(0, [wipe])
        .battlefield(0, [ready(mountain()), ready(bear()), ready(bear())])
        .battlefield(1, [ready(bear())])
        .library(0, [forest(), forest(), forest(), forest()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();

    driver.cast("Wipe").unwrap();
    settle(&mut driver);

    assert_eq!(
        in_hand(&driver, 0),
        3,
        "one card for each of the three creatures destroyed"
    );
}

#[test]
fn the_controller_of_an_exiled_permanent_draws_the_card() {
    let exile = instant(
        "Exile",
        r#"{"Spell":{"effects":[
            {"Exile":{"as":"exiled","what":{"Target":0}}},
            {"Draw":{"who":{"ControllerOf":{"Target":0}}}}],
          "targets":[{"what":{"Object":[{"Not":{"Type":"Land"}}]}}]}}"#,
    );
    let scenario = Scenario::new(2)
        .hand(0, [exile])
        .battlefield(0, [ready(mountain())])
        .battlefield(1, [ready(bear())])
        .library(0, [forest(), forest()])
        .library(1, [forest(), forest()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();

    driver
        .cast_at("Exile", &[driver.target_named("Grizzly Bears").unwrap()])
        .unwrap();
    settle(&mut driver);

    assert_eq!(in_hand(&driver, 1), 1, "its controller draws");
    assert_eq!(in_hand(&driver, 0), 0, "and its caster does not");
}

#[test]
fn a_delayed_trigger_fires_for_the_creature_its_spell_targeted() {
    let pump = instant(
        "Last Stand",
        r#"{"Spell":{"effects":[
            {"DelayedTrigger":{"effects":[{"Draw":{}}],"this_turn":true,
             "when":{"ZoneChange":{"from":"Battlefield","to":"Graveyard","what":[{"Is":{"Target":0}}]}}}}],
          "targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}]}}"#,
    );
    let scenario = Scenario::new(2)
        .hand(0, [pump, burn("Bolt", 3)])
        .battlefield(
            0,
            [
                ready(mountain()),
                ready(mountain()),
                ready(bear()),
                ready(bear()),
            ],
        )
        .library(0, [forest(), forest(), forest()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let bears = driver.state().zone(Zone::Battlefield).to_vec();
    let bear_id = bears
        .into_iter()
        .find(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.card().name == "Grizzly Bears")
        })
        .unwrap();

    driver
        .cast_at(
            "Last Stand",
            &[mtg_engine::choose::Target::Object(
                driver.state().object(bear_id).unwrap().reference(),
            )],
        )
        .unwrap();
    settle(&mut driver);
    driver
        .cast_at(
            "Bolt",
            &[mtg_engine::choose::Target::Object(
                driver.state().object(bear_id).unwrap().reference(),
            )],
        )
        .unwrap();
    settle(&mut driver);

    assert_eq!(
        in_hand(&driver, 0),
        1,
        "the creature died, so the delayed trigger drew"
    );
}
