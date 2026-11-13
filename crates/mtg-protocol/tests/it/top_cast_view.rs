//! Goal: under a rule that lets a seat look at and cast the top card of its library (Sigarda, Font of Blessings), the owner's view names that card in `known_top` and its priority decision offers it, while no other seat and no spectator learns it, at two to four seats.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_core::card::CardType;
use mtg_core::color::Color;
use mtg_core::ids::PlayerId;
use mtg_engine::decision::DecisionKind;
use mtg_engine::zones::Zone;
use mtg_protocol::StateView;
use mtg_testkit::cards::{cost, forest, plains};
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;
use serde_json::Value;

const TOP: &str = "Secret Angel";

/// Sigarda's two library rules on an enchantment.
fn sigarda_rules() -> Arc<CardDefinition> {
    let rules = r#"[
        {"Rule":{"rule":{"LookAtTopAnyTime":"You"}}},
        {"Rule":{"rule":{"MayCast":{"what":[{"AnyOf":[{"Subtype":"Angel"},{"Subtype":"Human"}]},"TopOfLibrary",{"Not":{"Type":"Land"}}],"from":"Library"}}}}
    ]"#;
    let mut card = CardDefinition::new("Holder").with_type(CardType::Enchantment);
    for ability in serde_json::from_str::<Vec<Value>>(rules).unwrap() {
        card = card.with_rules(serde_json::from_value(ability).unwrap());
    }
    Arc::new(card)
}

fn angel() -> Arc<CardDefinition> {
    Arc::new(
        CardDefinition::new(TOP)
            .with_type(CardType::Creature)
            .with_subtype("Angel")
            .with_mana_cost(cost(0, &[Color::White]))
            .with_power_toughness(2, 2),
    )
}

fn names(value: &Value, name: &str) -> bool {
    match value {
        Value::String(text) => text == name,
        Value::Array(items) => items.iter().any(|item| names(item, name)),
        Value::Object(map) => map.values().any(|item| names(item, name)),
        _ => false,
    }
}

#[rstest]
fn only_the_owner_sees_and_may_cast_the_top_card(#[values(2, 3, 4)] seats: usize) {
    let mut scenario = Scenario::new(seats)
        .library(0, [angel(), forest()])
        .battlefield(0, [ready(plains()), ready(sigarda_rules())]);
    for other in 1..seats {
        scenario = scenario.library(other, vec![forest(); 2]);
    }
    let driver = Driver::from_scenario(scenario).unwrap();
    let state = driver.state();
    let owner = PlayerId::from_index(0);
    let top = *state.zone(Zone::Library(owner)).last().unwrap();
    let mine = StateView::for_player(state, owner);
    assert_eq!(mine.table.players[0].known_top, vec![top]);
    assert!(
        mine.objects
            .iter()
            .any(|object| object.id == top && object.name == TOP)
    );
    let DecisionKind::Priority(options) = &driver.decision().unwrap().kind else {
        panic!("priority expected");
    };
    assert_eq!(driver.decision().unwrap().player, owner);
    assert!(options.castable.contains(&top));
    let others = (1..seats).map(|seat| StateView::for_player(state, PlayerId::from_index(seat)));
    for view in others.chain([StateView::for_spectator(state)]) {
        assert_eq!(view.table.players[0].known_top, Vec::new());
        assert!(
            !names(&serde_json::to_value(&view).unwrap(), TOP),
            "{TOP} leaked"
        );
    }
}
