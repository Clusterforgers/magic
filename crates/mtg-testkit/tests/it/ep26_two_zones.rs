//! Goal: "You may cast ~ from your graveyard or from exile" (one permission of the card for each zone, each working from its own zone) makes the card castable from the graveyard and from exile, never from the library, and the cast puts it onto the battlefield; the log replays to the live state.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use crate::common::copying::{replays, run};
use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::mana::ManaCost;
use mtg_engine::decision::DecisionKind;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::{Driver, Scenario, ScriptError, ready};
use rstest::rstest;

const FROM_GRAVEYARD: &str =
    r#"{"Rule":{"from":"Graveyard","rule":{"MayCast":{"from":"Graveyard","what":["This"]}}}}"#;
const FROM_EXILE: &str =
    r#"{"Rule":{"from":"Exile","rule":{"MayCast":{"from":"Exile","what":["This"]}}}}"#;
const BANISH: &str = r#"{"Activated":{"cost":[],"targets":[{"what":{"Object":[{"Type":"Creature"}]}}],"effects":[{"Exile":{"what":{"Target":0}}}]}}"#;

fn rules(json: &str) -> Ability {
    serde_json::from_str(json).unwrap()
}

fn goblin() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Goblin")
            .with_type(CardType::Creature)
            .with_power_toughness(2, 1)
            .with_mana_cost(ManaCost::parse("{0}").unwrap())
            .with_rules(rules(FROM_GRAVEYARD))
            .with_rules(rules(FROM_EXILE)),
    )
}

fn banisher() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new("Banisher")
            .with_type(CardType::Artifact)
            .with_rules(rules(BANISH)),
    )
}

fn castable(driver: &Driver) -> bool {
    match driver.decision().map(|d| &d.kind) {
        Ok(DecisionKind::Priority(options)) => options.castable.iter().any(|id| {
            driver
                .state()
                .object(*id)
                .is_some_and(|o| o.card().name == "Goblin")
        }),
        _ => false,
    }
}

fn on_battlefield(driver: &Driver) -> bool {
    driver.state().zone(Zone::Battlefield).iter().any(|id| {
        driver
            .state()
            .object(*id)
            .is_some_and(|o| o.card().name == "Goblin")
    })
}

#[rstest]
#[case::from_the_graveyard("graveyard", true)]
#[case::from_exile("exile", true)]
#[case::not_from_the_library("library", false)]
fn the_card_is_cast_from_either_zone(
    #[case] zone: &str,
    #[case] allowed: bool,
) -> Result<(), ScriptError> {
    let scenario = || {
        let s = Scenario::new(2)
            .at(1, 0, Step::Main1)
            .battlefield(0, vec![ready(banisher())]);
        match zone {
            "graveyard" => s.graveyard(0, [goblin()]),
            "library" => s.library(0, [goblin()]),
            _ => s.battlefield(0, vec![ready(banisher()), ready(goblin())]),
        }
    };
    let setup = scenario().setup().unwrap();
    let mut driver = Driver::from_scenario(scenario())?;
    if zone == "exile" {
        let target = driver.target_named("Goblin")?;
        driver.begin_activation("Banisher")?;
        driver.choose_targets(&[target])?;
        run(&mut driver, 2)?;
        assert!(!on_battlefield(&driver));
    }
    assert_eq!(castable(&driver), allowed);
    if allowed {
        driver.cast("Goblin")?;
        run(&mut driver, 2)?;
        assert!(on_battlefield(&driver));
    }
    replays(&driver, &setup);
    Ok(())
}
