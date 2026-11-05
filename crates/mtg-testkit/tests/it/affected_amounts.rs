//! Goal: a static change whose amount reads `Affected` is worked out for each object it applies to, so a creature counts
//! the creatures that share a type with it, the counters on itself or its own mana value, and not those of the source.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cards::value::CounterKind;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::characteristics::characteristics;
use mtg_testkit::{Driver, Scenario, ready, with_counters};

/// Each creature gets +1/+1 for each other creature that shares a creature type with it (Coat of Arms).
const SHARED: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"ModifyPT":{"power":{"Count":[{"Type":"Creature"},{"SharesSubtype":"Affected"},{"Not":{"Is":"Affected"}}]},"toughness":{"Count":[{"Type":"Creature"},{"SharesSubtype":"Affected"},{"Not":{"Is":"Affected"}}]}}}]}}"#;
/// Each creature gets +1/+1 for each spore counter on it (Thelon of Havenwood).
const SPORES: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"ModifyPT":{"power":{"Counters":{"on":"Affected","counter":{"Other":"spore"}}},"toughness":{"Counters":{"on":"Affected","counter":{"Other":"spore"}}}}}]}}"#;
/// Each noncreature artifact is a creature with power and toughness equal to its mana value (March of the Machines).
const MACHINES: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Artifact"},{"Not":{"Type":"Creature"}}]},"mods":[{"SetPT":{"power":{"Stat":{"of":"Affected","stat":"ManaValue"}},"toughness":{"Stat":{"of":"Affected","stat":"ManaValue"}}}},{"AddType":"Creature"}]}}"#;

fn source(rules: &str) -> Arc<CardDefinition> {
    let rule: Ability = serde_json::from_str(rules).unwrap();
    Arc::new(
        CardDefinition::new("Source")
            .with_type(CardType::Enchantment)
            .with_rules(rule),
    )
}

fn kind(name: &str, subtype: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Creature)
            .with_subtype(subtype)
            .with_power_toughness(2, 2),
    )
}

fn artifact(name: &str, cost: &str) -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Artifact)
            .with_mana_cost(ManaCost::parse(cost).unwrap()),
    )
}

fn stats(driver: &Driver, name: &str) -> (i32, i32) {
    let chars = characteristics(driver.state(), driver.id_of(name).unwrap()).unwrap();
    (chars.power.unwrap(), chars.toughness.unwrap())
}

#[test]
fn a_creature_counts_the_others_that_share_its_type() {
    let scenario = Scenario::new(2).battlefield(
        0,
        [
            ready(source(SHARED)),
            ready(kind("A", "Bear")),
            ready(kind("B", "Bear")),
            ready(kind("M", "Merfolk")),
        ],
    );
    let driver = Driver::from_scenario(scenario).unwrap();
    assert_eq!(stats(&driver, "A"), (3, 3));
    assert_eq!(stats(&driver, "B"), (3, 3));
    assert_eq!(stats(&driver, "M"), (2, 2));
}

#[test]
fn a_creature_counts_the_counters_on_itself() {
    let spored = |name, counters| {
        with_counters(
            ready(kind(name, "Bear")),
            CounterKind::Other("spore".into()),
            counters,
        )
    };
    let scenario =
        Scenario::new(2).battlefield(0, [ready(source(SPORES)), spored("A", 2), spored("B", 0)]);
    let driver = Driver::from_scenario(scenario).unwrap();
    assert_eq!(stats(&driver, "A"), (4, 4));
    assert_eq!(stats(&driver, "B"), (2, 2));
}

#[test]
fn an_object_takes_its_own_mana_value() {
    let scenario = Scenario::new(2).battlefield(
        0,
        [
            ready(source(MACHINES)),
            ready(artifact("One", "{1}")),
            ready(artifact("Three", "{3}")),
        ],
    );
    let driver = Driver::from_scenario(scenario).unwrap();
    assert_eq!(stats(&driver, "One"), (1, 1));
    assert_eq!(stats(&driver, "Three"), (3, 3));
}
