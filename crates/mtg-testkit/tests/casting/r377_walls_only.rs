//! Goal: "can't be the target of spells that can target only Walls" (Wall of Shadows) keeps a spell that targets only
//! Walls from targeting it but not from targeting another Wall, and lets a spell that can target any creature or any
//! target take it, at two to four seats; each log replays.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_testkit::cards::{cost, free_creature};
use mtg_testkit::corpus::card;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// A free instant that deals 1 damage to a target of `what` (JSON of a `Targetable`).
fn zap(name: &str, what: &str) -> Arc<CardDefinition> {
    let json = format!(
        r#"{{"Spell":{{"targets":[{{"what":{what}}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":1}}}}]}}}}"#
    );
    let rule: Ability = serde_json::from_str(&json).unwrap();
    Arc::new(
        CardDefinition::new(name)
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(rule),
    )
}

/// A plain Wall.
fn wall() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::clone(&free_creature("Brick Wall", 0, 4)).with_subtype("Wall"),
    )
}

#[rstest]
fn r377_only_walls_cannot_target_it(
    #[values(2, 3, 4)] seats: usize,
    #[values(
        (r#"{"Object":[{"Subtype":"Wall"}]}"#, "Wall of Shadows", false),
        (r#"{"Object":[{"Subtype":"Wall"}]}"#, "Brick Wall", true),
        (r#"{"Object":[{"Type":"Creature"}]}"#, "Wall of Shadows", true),
        (r#""AnyTarget""#, "Wall of Shadows", true),
        (r#"{"Either":[{"Object":[{"Subtype":"Wall"}]},{"Object":[{"Subtype":"Wall"},{"Type":"Creature"}]}]}"#, "Wall of Shadows", false)
    )]
    case: (&str, &str, bool),
) {
    let (what, aim, legal) = case;
    let scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(1, [ready(card("Wall of Shadows").unwrap()), ready(wall())])
        .hand(0, [zap("Zap", what)]);
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.pass_until(Step::Main1).unwrap();
    let at = driver.target_named(aim).unwrap();
    let cast = driver.cast_at("Zap", &[at]);
    assert_eq!(cast.is_ok(), legal, "{what} at {aim}");
    if legal {
        while !driver.state().stack().is_empty() {
            driver.pass().unwrap();
        }
        let setup = scenario.setup().unwrap();
        let replayed = replay(&setup, driver.engine().events()).unwrap();
        assert_eq!(replayed.state_hash(), driver.engine().state_hash());
    }
}
