//! Goal: the table view and the lobby row carry the house rules that are set and leave the field out when none is, the same for every viewer, in the wire shape the clients read, and the tax a commander carries in the view is the tax of the rule.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use mtg_core::house::{CommanderDamageLimit, DeckRules, HandLimit, HouseRules, SkipFirstDraw};
use mtg_core::ids::PlayerId;
use mtg_engine::Engine;
use mtg_engine::choose::YesNoChoice;
use mtg_engine::decision::Answer;
use mtg_engine::format::Commander;
use mtg_engine::manual::{ManualAction, ManualZone};
use mtg_protocol::StateView;
use mtg_protocol::rest::LobbyGame;
use mtg_testkit::commander_cards::legend;
use mtg_testkit::{Driver, Scenario, fuzz_setup};
use rstest::rstest;
use serde_json::{Value, json};

fn rules(set: impl FnOnce(&mut HouseRules)) -> HouseRules {
    let mut rules = HouseRules::default();
    set(&mut rules);
    rules
}

#[rstest]
#[case::none(HouseRules::default(), None)]
#[case::life(
    rules(|r| r.starting_life = Some(40)),
    Some(json!({ "starting_life": 40 }))
)]
#[case::a_value_equal_to_the_formats(
    rules(|r| r.starting_life = Some(20)),
    Some(json!({ "starting_life": 20 }))
)]
#[case::several(
    rules(|r| {
        r.opening_hand_size = Some(5);
        r.skip_first_draw = Some(SkipFirstDraw::Always);
        r.max_hand_size = Some(HandLimit::Cards(9));
    }),
    Some(json!({
        "opening_hand_size": 5,
        "skip_first_draw": "always",
        "max_hand_size": { "cards": 9 },
    }))
)]
#[case::unlimited(
    rules(|r| r.max_hand_size = Some(HandLimit::Unlimited)),
    Some(json!({ "max_hand_size": "unlimited" }))
)]
#[case::commander_tax(
    rules(|r| r.commander_tax = Some(0)),
    Some(json!({ "commander_tax": 0 }))
)]
#[case::commander_damage(
    rules(|r| r.commander_damage = Some(CommanderDamageLimit::Lethal(15))),
    Some(json!({ "commander_damage": { "lethal": 15 } }))
)]
#[case::no_commander_damage(
    rules(|r| r.commander_damage = Some(CommanderDamageLimit::Off)),
    Some(json!({ "commander_damage": "off" }))
)]
#[case::the_deck_check(
    rules(|r| {
        r.deck = DeckRules { singleton: Some(false), colour_identity: Some(false), deck_size: Some(60), max_copies: Some(4) };
        r.commanders_per_seat = Some(2);
    }),
    Some(json!({
        "deck": { "singleton": false, "colour_identity": false, "deck_size": 60, "max_copies": 4 },
        "commanders_per_seat": 2,
    }))
)]
#[case::an_empty_deck_check_is_no_rule(
    rules(|r| r.deck = DeckRules::default()),
    None
)]
fn every_viewer_is_told_the_rules_that_are_set(
    #[case] house: HouseRules,
    #[case] wire: Option<Value>,
    #[values(0, 1, 2)] seat: u64,
) {
    let engine = Engine::new(fuzz_setup(3, 2).with_house_rules(house.clone())).unwrap();
    let view = StateView::for_player(engine.state(), PlayerId(seat));
    assert_eq!(
        view.table.house_rules,
        wire.as_ref().map(|_| house.clone()),
        "the view"
    );
    let value = serde_json::to_value(&view).unwrap();
    assert_eq!(value.get("house_rules"), wire.as_ref());
    let back: StateView = serde_json::from_value(value).unwrap();
    assert_eq!(back.table.house_rules, view.table.house_rules);
}

#[test]
fn a_view_from_before_house_rules_still_loads() {
    let engine = Engine::new(fuzz_setup(2, 2)).unwrap();
    let mut value =
        serde_json::to_value(StateView::for_player(engine.state(), PlayerId(0))).unwrap();
    value.as_object_mut().unwrap().remove("house_rules");
    let view: StateView = serde_json::from_value(value).unwrap();
    assert_eq!(view.table.house_rules, None);
}

fn row(house_rules: Option<HouseRules>) -> LobbyGame {
    LobbyGame {
        game: "6f1e0c1a-3b1f-4d9e-8a3e-0d3c2d1b7a55".into(),
        host: "Ann".into(),
        format: "normal".into(),
        house_rules,
        manual_play: false,
        filled: 1,
        total: 4,
        created_at: "2026-10-19T12:30:00Z".into(),
        age_secs: 5,
    }
}

#[test]
fn a_lobby_row_lists_the_rules_when_they_are_set_and_only_then() {
    let plain = serde_json::to_value(row(None)).unwrap();
    assert!(plain.get("house_rules").is_none());

    let set = row(Some(rules(|r| r.mulligan_cap = Some(2))));
    let value = serde_json::to_value(&set).unwrap();
    assert_eq!(value["house_rules"], json!({ "mulligan_cap": 2 }));
    assert_eq!(serde_json::from_value::<LobbyGame>(value).unwrap(), set);
    assert_eq!(
        serde_json::from_value::<LobbyGame>(plain).unwrap(),
        row(None)
    );
}

#[test]
fn a_lobby_row_lists_the_rules_for_commander() {
    let set = row(Some(rules(|r| {
        r.commander_tax = Some(1);
        r.commander_damage = Some(CommanderDamageLimit::Off);
        r.deck.deck_size = Some(60);
    })));
    let value = serde_json::to_value(&set).unwrap();
    assert_eq!(
        value["house_rules"],
        json!({ "commander_tax": 1, "commander_damage": "off", "deck": { "deck_size": 60 } })
    );
    assert_eq!(serde_json::from_value::<LobbyGame>(value).unwrap(), set);
}

#[rstest]
#[case::the_formats(None, 2)]
#[case::none(Some(0), 0)]
#[case::five(Some(5), 5)]
fn the_tax_in_the_view_is_the_tax_of_the_rule(
    #[case] tax: Option<u32>,
    #[case] per_cast: u32,
    #[values(2, 4)] seats: usize,
) {
    let scenario = Scenario::new(seats)
        .format(Arc::new(Commander))
        .house_rules(rules(|r| r.commander_tax = tax))
        .commanders(0, [legend("Cmd", 0, &[], 2, 2)]);
    let mut driver = Driver::from_scenario(scenario).unwrap();
    driver.cast("Cmd").unwrap();
    for _ in 0..seats {
        driver.pass().unwrap();
    }
    // Back in the command zone, the commander has been cast once and carries the tax of one cast.
    let object = driver.state().commanders().next().unwrap();
    driver
        .manual(ManualAction::Move {
            object,
            to: ManualZone::Graveyard,
        })
        .unwrap();
    driver.answer(Answer::YesNo(YesNoChoice(true))).unwrap();
    let view = StateView::for_player(driver.state(), PlayerId(1));
    let commander = &view.table.players[0].commanders[0];
    assert_eq!((commander.casts, commander.tax), (1, per_cast));
    assert_eq!(view.table.house_rules.is_some(), tax.is_some());
}
