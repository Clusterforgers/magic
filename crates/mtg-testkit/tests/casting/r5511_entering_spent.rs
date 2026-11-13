//! Goal: a replacement that gives each entering creature a +1/+1 counter for each mana from a Treasure or an artifact
//! source spent to cast it (Kalain, Coin of Mastery) counts the sources that paid for that creature, not for its own
//! source, gives none to a creature cast without them and none to the source itself when it says "other", at 2 to 4
//! seats, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, island};
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

/// An artifact Treasure: "{T}, Sacrifice this artifact: Add one mana of any color."
fn treasure() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Treasure")
            .with_type(CardType::Artifact)
            .with_subtype("Treasure")
            .with_rules(rules(
                r#"{"Activated":{"cost":["Tap","SacrificeSelf"],"effects":[{"AddMana":{"mana":{"AnyColor":1}}}]}}"#,
            )),
    )
}

/// A mana rock that is no Treasure: "{T}: Add {U}."
fn rock() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Rock")
            .with_type(CardType::Artifact)
            .with_rules(rules(
                r#"{"Activated":{"cost":["Tap"],"effects":[{"AddMana":{"mana":{"Fixed":["Blue"]}}}]}}"#,
            )),
    )
}

/// A creature with the replacement for the stat named.
fn painter(stat: &str) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"Replacement":{{"when":{{"Enters":{{"what":[{{"Type":"Creature"}},"Other",{{"ControlledBy":"You"}}]}}}},"by":{{"WithCounters":{{"counter":"+1/+1","count":{{"Stat":{{"of":"Affected","stat":"{stat}"}}}}}}}}}}}}"#
    );
    Arc::new(
        CardDefinition::new("Painter")
            .with_type(CardType::Creature)
            .with_power_toughness(1, 1)
            .with_rules(rules(&json)),
    )
}

/// A vanilla creature for `{1}{U}`.
fn bear() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Bear")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 2)
            .with_mana_cost(cost(1, &[Color::Blue])),
    )
}

fn counters_on(driver: &Driver, name: &str) -> u32 {
    driver
        .state()
        .objects()
        .find(|object| {
            object.card().name == name && object.zone() == mtg_engine::zones::Zone::Battlefield
        })
        .and_then(|object| object.permanent())
        .map_or(0, |permanent| {
            permanent.counters_of(&CounterKind::PlusOnePlusOne)
        })
}

#[rstest]
#[case::treasures_none("TreasureManaSpent", &["Island", "Island"], 0)]
#[case::treasures_one("TreasureManaSpent", &["Treasure", "Island"], 1)]
#[case::treasures_not_a_rock("TreasureManaSpent", &["Rock", "Island"], 0)]
#[case::treasures_both("TreasureManaSpent", &["Treasure", "Treasure"], 2)]
#[case::artifacts_rock("ArtifactManaSpent", &["Rock", "Island"], 1)]
#[case::artifacts_rock_and_treasure("ArtifactManaSpent", &["Rock", "Treasure"], 2)]
#[case::artifacts_none("ArtifactManaSpent", &["Island", "Island"], 0)]
fn r5511_the_entering_creature_counts_its_own_mana(
    #[case] stat: &str,
    #[case] paying: &[&str],
    #[case] counters: u32,
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let mut driver = Driver::from_scenario(
        Scenario::new(seats)
            .at(1, 0, Step::Main1)
            .hand(0, vec![bear()])
            .battlefield(
                0,
                [
                    ready(painter(stat)),
                    ready(treasure()),
                    ready(treasure()),
                    ready(rock()),
                    ready(island()),
                    ready(island()),
                ],
            ),
    )?;
    driver.cast_tapping("Bear", paying)?;
    while !driver.state().stack().is_empty() {
        driver.pass()?;
    }
    assert_eq!(counters_on(&driver, "Bear"), counters);
    assert_eq!(counters_on(&driver, "Painter"), 0, "other creatures only");
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
