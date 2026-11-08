//! Goal: a spell that acts on its target and on the cards sharing the target's name plays through: the named cards go, the other cards stay, and a delayed trigger of a target waits for that target.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_cli::forge::script::Script;
use mtg_cli::translate::card::translate_script;
use mtg_cli::translate::ctx::World;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_engine::zones::Zone;
use mtg_testkit::cards::{bear, cost, giant, island, mountain};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

/// The card a Forge script gives, with the rules the translator read.
fn card(text: &str, mana: (u32, &[Color])) -> std::sync::Arc<CardDefinition> {
    let out = translate_script(&World::new(&[], BTreeMap::new()), &Script::parse(text));
    let face = &out.faces[0];
    assert!(face.gaps.is_empty(), "{:?}", face.gaps);
    let mut def = CardDefinition::new(&out.name)
        .with_type(CardType::Instant)
        .with_mana_cost(cost(mana.0, mana.1));
    for rules in &face.abilities {
        let ability: Ability = serde_json::from_value(rules.clone()).unwrap();
        def = def.with_rules(ability);
    }
    std::sync::Arc::new(def)
}

fn on_battlefield(driver: &Driver, name: &str) -> usize {
    let state = driver.state();
    state
        .zone(Zone::Battlefield)
        .iter()
        .filter(|id| state.object(**id).is_some_and(|o| o.card().name == name))
        .count()
}

const ECHOING_TRUTH: &str = "Name:Echoing Test\nManaCost:1 U\nTypes:Instant\nA:SP$ Pump | ValidTgts$ Permanent.nonLand | TgtPrompt$ Select target nonland permanent | SubAbility$ All\nSVar:All:DB$ ChangeZoneAll | Origin$ Battlefield | Destination$ Hand | ChangeType$ TargetedCard.Self,Permanent.NotDefinedTargeted+sharesNameWith Targeted\n";

#[rstest]
#[case::the_target_and_its_namesakes_return("Grizzly Bears", 0, 2)]
#[case::another_name_leaves_the_bears("Craw Wurm", 3, 0)]
fn the_target_and_the_cards_with_its_name_return(
    #[case] target: &str,
    #[case] bears_left: usize,
    #[case] wurms_left: usize,
) {
    let spell = card(ECHOING_TRUTH, (1, &[Color::Blue]));
    let name = spell.name.clone();
    let scenario = Scenario::new(2)
        .hand(0, [spell])
        .battlefield(
            0,
            [
                ready(island()),
                ready(island()),
                ready(bear()),
                ready(mountain()),
            ],
        )
        .battlefield(
            1,
            [ready(bear()), ready(bear()), ready(giant()), ready(giant())],
        );
    let mut driver = Driver::from_scenario(scenario).unwrap();
    let at = driver.target_named(target).unwrap();
    driver.cast_at(&name, &[at]).unwrap();
    for _ in 0..10 {
        if driver.state().stack().is_empty() {
            break;
        }
        driver.pass().unwrap();
    }
    assert_eq!(on_battlefield(&driver, "Grizzly Bears"), bears_left);
    assert_eq!(on_battlefield(&driver, "Craw Wurm"), wurms_left);
}
