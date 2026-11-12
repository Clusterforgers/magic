//! Goal: a rule that lets its controller look at face-down creatures they don't control (`GameRule::LookAtFaceDown`,
//! Keeper of the Lens) shows each opponent's face-down creature to that controller only, at two to four seats and
//! replayed to the live state; without it a face-down creature stays hidden from every seat but its controller.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_cards::CardDefinition;
use mtg_cards::ability::Ability;
use mtg_core::card::CardType;
use mtg_core::ids::PlayerId;
use mtg_engine::replay;
use mtg_engine::turn::Step;
use mtg_engine::zones::Zone;
use mtg_testkit::{Driver, Scenario, ready};
use rstest::rstest;

fn card(name: &str, kind: CardType, rules: &[&str]) -> Arc<CardDefinition> {
    let mut card = CardDefinition::new(name).with_type(kind);
    if kind == CardType::Creature {
        card = card.with_power_toughness(1, 1);
    }
    if kind == CardType::Sorcery {
        card = card.with_mana_cost(mtg_core::mana::ManaCost::parse("{0}").unwrap());
    }
    for rule in rules {
        let ability: Ability = serde_json::from_str(rule).unwrap();
        card = card.with_rules(ability);
    }
    Arc::new(card)
}

const KEEPER: &str = r#"{"Rule":{"rule":{"LookAtFaceDown":{"what":[{"Type":"Creature"},{"Not":{"ControlledBy":"You"}}]}}}}"#;
const MANIFEST_ALL: &str = r#"{"Spell":{"effects":[{"Manifest":{"who":"EachPlayer"}}]}}"#;

#[rstest]
fn r287_face_down_creatures_you_dont_control_are_shown_to_you(
    #[values(2, 3, 4)] seats: usize,
    #[values(true, false)] keeper: bool,
) {
    let rules: &[&str] = if keeper { &[KEEPER] } else { &[] };
    let mut scenario = Scenario::new(seats)
        .at(3, 0, Step::Main1)
        .battlefield(0, [ready(card("Keeper", CardType::Creature, rules))])
        .hand(0, [card("Act", CardType::Sorcery, &[MANIFEST_ALL])]);
    for seat in 0..seats {
        scenario = scenario.library(seat, [card("Hidden", CardType::Creature, &[])]);
    }
    let mut driver = Driver::from_scenario(scenario.clone()).unwrap();
    driver.cast("Act").unwrap();
    while !driver.state().stack().is_empty() {
        driver.pass().unwrap();
    }
    let state = driver.state();
    let hidden: Vec<_> = state
        .zone(Zone::Battlefield)
        .iter()
        .copied()
        .filter(|id| state.object(*id).is_some_and(|o| o.face_down().is_some()))
        .collect();
    assert_eq!(hidden.len(), seats, "each player manifested a card");
    for id in hidden {
        let controller = state.object(id).unwrap().controller();
        for seat in 0..seats {
            let viewer = PlayerId::from_index(seat);
            let sees = viewer == controller || (keeper && seat == 0);
            assert_eq!(state.may_look_at(viewer, id), sees, "seat {seat}");
        }
    }
    let setup = scenario.setup().unwrap();
    let replayed = replay(&setup, driver.engine().events()).unwrap();
    assert_eq!(replayed.state_hash(), driver.engine().state_hash());
}
