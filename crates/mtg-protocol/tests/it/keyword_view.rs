//! Goal: an object in a view names the keywords it has, once each in the order of its rules, and the field is left out of the JSON while there are none so the wire form of a vanilla game is unchanged; an ability on the stack has none; haste excuses summoning sickness in the view; and the options of blocks and damage keep the JSON they had until they carry something new.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use mtg_cards::ability::Ability;
use mtg_cards::keyword::Keyword;
use mtg_core::ids::{ObjectId, PlayerId};
use mtg_engine::combat::DamageTarget;
use mtg_engine::decision::{BlockOptions, BlockerOption, DamageOptions};
use mtg_protocol::{ObjectView, StateView};
use mtg_testkit::cards::{bear, keyworded, mountain, pinger};
use mtg_testkit::{Driver, Scenario, ready, sick};
use serde_json::json;

fn view_of(driver: &Driver, id: ObjectId) -> ObjectView {
    StateView::for_player(driver.state(), PlayerId(0))
        .objects
        .into_iter()
        .find(|o| o.id == id)
        .unwrap()
}

#[test]
fn names_come_from_the_rules_of_the_card_once_each_in_order() {
    let angel = keyworded(
        "Angel",
        4,
        4,
        &[
            Keyword::Flying,
            Keyword::FirstStrike,
            Keyword::Flying,
            Keyword::DoubleStrike,
            Keyword::Vigilance,
        ],
    );
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(0, [ready(angel)])).unwrap();
    let id = driver.id_of("Angel").unwrap();
    assert_eq!(
        view_of(&driver, id).keywords,
        ["Flying", "First strike", "Double strike", "Vigilance"]
    );
}

#[test]
fn the_field_is_left_out_while_there_are_no_keywords() {
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(
        0,
        [
            ready(bear()),
            ready(keyworded("Flyer", 1, 1, &[Keyword::Flying])),
        ],
    ))
    .unwrap();
    let plain =
        serde_json::to_value(view_of(&driver, driver.id_of("Grizzly Bears").unwrap())).unwrap();
    assert!(plain.get("keywords").is_none(), "{plain}");
    let flyer = serde_json::to_value(view_of(&driver, driver.id_of("Flyer").unwrap())).unwrap();
    assert_eq!(flyer["keywords"], json!(["Flying"]));
    // A view without the field reads back as one without keywords.
    let back: ObjectView = serde_json::from_value(plain).unwrap();
    assert_eq!(back.keywords, Vec::<String>::new());
}

#[test]
fn an_ability_on_the_stack_has_no_keywords_of_its_own() {
    let mut card = (*pinger("Pinger", 1)).clone();
    card = card.with_rules(Ability::Keyword(Keyword::Deathtouch));
    let mut driver = Driver::from_scenario(
        Scenario::new(2).battlefield(0, [ready(std::sync::Arc::new(card)), ready(mountain())]),
    )
    .unwrap();
    let source = driver.id_of("Pinger").unwrap();
    driver.begin_activation("Pinger").unwrap();
    let at = driver.target_player(1);
    driver.choose_targets(&[at]).unwrap();
    let ability = *driver.state().stack().last().unwrap();
    assert_eq!(view_of(&driver, source).keywords, ["Deathtouch"]);
    assert_eq!(view_of(&driver, ability).keywords, Vec::<String>::new());
}

#[test]
fn haste_excuses_summoning_sickness_in_the_view() {
    let driver = Driver::from_scenario(Scenario::new(2).battlefield(
        0,
        [
            sick(keyworded("Hasty", 2, 2, &[Keyword::Haste])),
            sick(keyworded("Slow", 2, 2, &[Keyword::Vigilance])),
            ready(keyworded("Ready", 2, 2, &[])),
        ],
    ))
    .unwrap();
    let sickness = |name: &str| {
        view_of(&driver, driver.id_of(name).unwrap())
            .permanent
            .unwrap()
            .summoning_sick
    };
    assert_eq!(
        (sickness("Hasty"), sickness("Slow"), sickness("Ready")),
        (false, true, false)
    );
}

#[test]
fn the_options_of_blocks_and_damage_keep_their_json_until_they_carry_something_new() {
    let (a, b) = (ObjectId(1), ObjectId(2));
    let blockers = vec![BlockerOption {
        object: a,
        can_block: vec![b],
    }];
    assert_eq!(
        serde_json::to_value(BlockOptions {
            blockers: blockers.clone(),
            min_blockers: Vec::new(),
            required: Vec::new()
        })
        .unwrap(),
        json!({"blockers": [{"object": 1, "can_block": [2]}]})
    );
    assert_eq!(
        serde_json::to_value(BlockOptions {
            blockers,
            min_blockers: vec![(b, 2)],
            required: Vec::new()
        })
        .unwrap(),
        json!({"blockers": [{"object": 1, "can_block": [2]}], "min_blockers": [[2, 2]]})
    );
    let plain = DamageOptions {
        attacker: a,
        power: 3,
        blockers: vec![b],
        excess: None,
        lethal: Vec::new(),
    };
    assert_eq!(
        serde_json::to_value(&plain).unwrap(),
        json!({"attacker": 1, "power": 3, "blockers": [2]})
    );
    let trample = DamageOptions {
        excess: Some(DamageTarget::Player(PlayerId(1))),
        lethal: vec![2],
        ..plain
    };
    let value = serde_json::to_value(&trample).unwrap();
    assert_eq!(
        value,
        json!({"attacker": 1, "power": 3, "blockers": [2], "excess": {"Player": 1}, "lethal": [2]})
    );
    // An older sender's options read back without the new fields.
    let old: DamageOptions =
        serde_json::from_value(json!({"attacker": 1, "power": 3, "blockers": [2]})).unwrap();
    assert_eq!((old.excess, old.lethal), (None, Vec::new()));
    assert_eq!(
        serde_json::from_value::<DamageOptions>(value).unwrap(),
        trample
    );
}
