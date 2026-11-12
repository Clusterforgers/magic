//! Goal: a static change whose amount or condition reads `Affected` is worked out for each object it applies to (a creature counts
//! the creatures that share a type with it, the counters on itself, its own mana value), a spell is not counted in its own first-spell condition, and a spell remembers the cards exiled to cast it.
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

/// The first instant you cast each turn has lifelink (the shape of Elda's demonstrate): the spells cast before the
/// affected one are counted, not the affected spell itself.
const FIRST_INSTANT: &str = r#"{"Static":{"affected":{"Each":[{"Type":"Instant"},{"ControlledBy":"You"},{"In":"Stack"}]},"mods":[{"AddKeyword":"Lifelink"}],"condition":{"Compare":{"left":{"Tally":{"event":"SpellCast","who":"EachPlayer","filter":[{"Type":"Instant"},{"ControlledBy":"You"},{"Not":{"Is":"Affected"}}]}},"op":"Eq","right":0}}}}"#;

#[test]
fn only_the_first_spell_of_the_turn_has_the_keyword_while_it_is_cast() {
    use mtg_cards::keyword::Keyword;
    use mtg_engine::keywords::keywords_of;
    use mtg_testkit::cards::instant;

    let scenario = Scenario::new(2)
        .hand(0, [instant("First", 0, &[]), instant("Second", 0, &[])])
        .battlefield(0, [ready(source(FIRST_INSTANT))]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let lifelink = |driver: &Driver, name: &str| {
        let stacked = driver
            .state()
            .stack()
            .iter()
            .copied()
            .find(|id| driver.state().object(*id).unwrap().card().name == name)
            .unwrap();
        keywords_of(driver.state(), stacked).contains(&&Keyword::Lifelink)
    };
    driver.cast("First").unwrap();
    assert!(lifelink(&driver, "First"));
    driver.cast("Second").unwrap();
    assert!(!lifelink(&driver, "Second"));
}

/// A 4/4 that may exile a creature card from a graveyard as it is cast, and gets +X/+X for the power X of the card exiled
/// with it (Redemptor Dreadnought).
const EXILE_COST: &str =
    r#"{"AdditionalCost":{"cost":[{"ExileFromGraveyard":{"filter":[{"Type":"Creature"}]}}]}}"#;
const EXILED_POWER: &str = r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":{"Total":{"of":[{"ExiledWith":"This"},{"In":"Exile"}],"stat":"Power"}},"toughness":{"Total":{"of":[{"ExiledWith":"This"},{"In":"Exile"}],"stat":"Power"}}}}]}}"#;

#[test]
fn a_spell_remembers_the_cards_exiled_to_cast_it_as_exiled_with_it() {
    use mtg_engine::choose::CostsChoice;
    use mtg_engine::decision::{Answer, DecisionKind, PayChoice};

    let mut dreadnought = CardDefinition::new("Dread")
        .with_type(CardType::Creature)
        .with_power_toughness(4, 4)
        .with_mana_cost(ManaCost::parse("{0}").unwrap());
    for rule in [EXILE_COST, EXILED_POWER] {
        dreadnought = dreadnought.with_rules(serde_json::from_str::<Ability>(rule).unwrap());
    }
    let scenario = Scenario::new(2)
        .hand(0, [Arc::new(dreadnought)])
        .graveyard(0, [kind("Dead", "Bear")])
        .battlefield(0, []);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.begin_cast("Dread").unwrap();
    for _ in 0..30 {
        let decision = driver.decision().unwrap().clone();
        match decision.kind {
            DecisionKind::ChooseCosts(_) => driver
                .answer(Answer::ChooseCosts(CostsChoice::Costs(vec![0])))
                .map(|_| ())
                .unwrap(),
            DecisionKind::PayMana(_) => driver
                .answer(Answer::PayMana(PayChoice::Auto))
                .map(|_| ())
                .unwrap(),
            DecisionKind::Priority(_) if driver.state().stack().is_empty() => break,
            _ => {
                driver.answer_passively().map(|_| ()).unwrap();
            }
        }
    }
    assert_eq!(stats(&driver, "Dread"), (6, 6));
}
