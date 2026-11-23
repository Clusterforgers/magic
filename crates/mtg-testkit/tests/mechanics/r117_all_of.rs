//! Goal: a side of an either-or that is several qualities (`Pred::R117AllOf`) holds only when each of them holds, at tables of two, three and four seats: a sweep of "creatures you control or modified creatures" takes the caster's creatures and an opponent's creature with a counter and leaves an opponent's plain creature, and the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_engine::replay;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{cost, forest, free_creature};
use mtg_testkit::{Driver, Scenario, ScriptError, ready, with_counters};
use rstest::rstest;

const R117_SWEEP: &str = r#"{"Spell":{"effects":[{"Destroy":{"what":{"Each":[{"AnyOf":[{"R117AllOf":[{"Type":"Creature"},{"ControlledBy":"You"}]},{"R117AllOf":[{"Type":"Creature"},"Modified"]}]}]}}}]}}"#;

fn r117_zone(driver: &Driver, name: &str) -> Zone {
    driver
        .state()
        .objects()
        .find(|object| object.card().name == name)
        .unwrap()
        .zone()
}

#[rstest]
fn r117_all_of_holds_only_when_each_quality_does(
    #[values(2, 3, 4)] seats: usize,
) -> Result<(), ScriptError> {
    let ability: Ability = serde_json::from_str(R117_SWEEP).unwrap();
    let sweep = Arc::new(
        CardDefinition::new("Sweep")
            .with_type(CardType::Instant)
            .with_mana_cost(cost(0, &[]))
            .with_rules(ability),
    );
    let counter = mtg_cards::value::CounterKind::PlusOnePlusOne;
    let scenario = (0..seats).fold(Scenario::new(seats), |scenario, seat| {
        scenario.library(seat, (0..8).map(|_| forest()))
    });
    let scenario = scenario
        .hand(0, [sweep])
        .battlefield(0, [ready(free_creature("Bear", 2, 2)), ready(forest())])
        .battlefield(
            seats - 1,
            [
                with_counters(ready(free_creature("Ogre", 3, 3)), counter, 1),
                ready(free_creature("Wolf", 2, 2)),
            ],
        );
    let mut driver = Driver::from_scenario(scenario)?;
    driver.cast("Sweep")?;
    for _ in 0..20 {
        if driver.state().stack().is_empty() && driver.state().resolving().is_none() {
            break;
        }
        driver.pass()?;
    }
    assert!(
        matches!(r117_zone(&driver, "Bear"), Zone::Graveyard(_)),
        "yours"
    );
    assert!(
        matches!(r117_zone(&driver, "Ogre"), Zone::Graveyard(_)),
        "modified"
    );
    assert_eq!(
        r117_zone(&driver, "Wolf"),
        Zone::Battlefield,
        "neither side"
    );
    let lands = driver
        .state()
        .objects()
        .filter(|object| object.card().name == "Forest" && object.zone() == Zone::Battlefield);
    assert_eq!(lands.count(), 1, "not a creature");
    let state = replay(driver.engine().setup(), driver.engine().events()).unwrap();
    assert_eq!(state.state_hash(), driver.state().state_hash());
    Ok(())
}
