//! Goal: a card that has flash as long as a condition holds is castable at instant speed from the hand while the condition holds and only at sorcery speed otherwise, at tables of two and three seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use crate::common::name_of;

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, forest};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

/// A creature for `{G}` that "has flash as long as you control a Desert".
fn wurm() -> Arc<CardDefinition> {
    let flash: Ability = serde_json::from_str(
        r#"{"Static":{"affected":{"Each":[{"In":"Hand"},"This"]},"mods":[{"AddKeyword":"Flash"}],"condition":{"Compare":{"left":{"Count":[{"Subtype":"Desert"},{"ControlledBy":"You"}]},"op":"Ge","right":1}},"from":"Hand"}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Desert Wurm")
            .with_type(CardType::Creature)
            .with_mana_cost(cost(0, &[Color::Green]))
            .with_power_toughness(2, 2)
            .with_rules(flash),
    )
}

/// A land with the subtype Desert.
fn desert() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Dry Flat")
            .with_type(CardType::Land)
            .with_subtype("Desert"),
    )
}

#[rstest]
#[case::with_a_desert(true, true)]
#[case::without_a_desert(false, false)]
fn flash_holds_while_a_desert_is_controlled(
    #[case] desert_in_play: bool,
    #[case] castable_now: bool,
    #[values(2, 3)] seats: usize,
) -> Result<(), ScriptError> {
    let holder = seats - 1;
    let mut lands = vec![ready(forest())];
    if desert_in_play {
        lands.push(ready(desert()));
    }
    let scenario = Scenario::new(seats)
        .at(1, 0, Step::Main1)
        .battlefield(holder, lands)
        .hand(holder, [wurm()]);
    let mut driver = Driver::from_scenario(scenario)?;
    for _ in 0..seats {
        if driver.decision()?.player.index() == holder {
            break;
        }
        driver.pass()?;
    }
    let decision = driver.decision()?;
    let DecisionKind::Priority(options) = &decision.kind else {
        panic!("the holder has priority");
    };
    let names: Vec<String> = options
        .castable
        .iter()
        .map(|id| name_of(driver.state(), *id))
        .collect();
    assert_eq!(names.iter().any(|name| name == "Desert Wurm"), castable_now);
    Ok(())
}
