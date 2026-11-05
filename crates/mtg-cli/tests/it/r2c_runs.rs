//! Goal: a number a source remembers is read back by the steps after it: the mana value of a countered spell is read once the spell is gone.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost, elf, forest, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

use crate::r3p3_runs::settle;

/// A red instant that counters a spell and draws as many cards as the spell's mana value, through the memory of the source.
fn hush() -> Arc<CardDefinition> {
    let rules: Ability = serde_json::from_str(
        r#"{"Spell":{"effects":[
            {"Counter":{"what":{"Target":0}}},
            {"RememberValue":{"value":{"Stat":{"of":{"Target":0},"stat":"ManaValue"}},"slot":"remembered_number"}},
            {"Draw":{"count":{"Remembered":{"on":"This","slot":"remembered_number"}}}}],
          "targets":[{"what":{"Object":["Spell"]}}]}}"#,
    )
    .unwrap();
    Arc::new(
        CardDefinition::new("Hush")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[Color::Red]))
            .with_rules(rules),
    )
}

#[rstest]
#[case::two(bear(), 2)]
#[case::one(elf(), 1)]
fn the_mana_value_of_a_countered_spell_is_drawn(
    #[case] spell: Arc<CardDefinition>,
    #[case] drawn: usize,
) {
    let name = spell.name.clone();
    let scenario = Scenario::new(2)
        .hand(0, [spell, hush()])
        .battlefield(
            0,
            [
                ready(forest()),
                ready(forest()),
                ready(mountain()),
                ready(mountain()),
            ],
        )
        .library(0, [forest(), forest(), forest(), forest()]);
    let mut driver = Driver::from_scenario(scenario).unwrap();

    driver.cast(&name).unwrap();
    let on_stack = *driver.state().stack().last().unwrap();
    let target =
        mtg_engine::choose::Target::Object(driver.state().object(on_stack).unwrap().reference());
    driver.cast_at("Hush", &[target]).unwrap();
    settle(&mut driver);

    let hand = driver
        .state()
        .zone(Zone::Hand(PlayerId::from_index(0)))
        .len();
    assert_eq!(hand, drawn);
}
