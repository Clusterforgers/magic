//! Goal: a mana ability the basic land types of a face give is dropped from a proposal in every shape the model writes it, and a land with a mana ability of its own, other mana, a restriction or another kind of ability keeps what it proposed.
use rstest::rstest;
use serde_json::{Value, json};

use super::super::{RepairClass, repair};
use crate::support::{PrintedCard, PrintedFace};

fn land(type_line: &str, text: &str) -> PrintedCard {
    PrintedCard {
        name: "Card".to_owned(),
        faces: vec![PrintedFace {
            name: "Card".to_owned(),
            type_line: type_line.to_owned(),
            oracle_text: text.to_owned(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn tap_for(mana: &Value) -> Value {
    json!({ "Activated": { "cost": ["Tap"], "effects": [{ "AddMana": { "mana": mana } }] } })
}

/// The abilities after repairing a proposal of those abilities, and the classes of the repairs.
fn run(printed: &PrintedCard, abilities: &Value) -> (Value, Vec<RepairClass>) {
    let mut value = json!({ "faces": [{ "abilities": abilities }] });
    let repairs = repair(&mut value, printed);
    let classes = repairs.iter().map(|repaired| repaired.class).collect();
    (value["faces"][0]["abilities"].clone(), classes)
}

const ISLAND_MOUNTAIN: &str = "Land \u{2014} Island Mountain";
const REMINDER: &str = "({T}: Add {U} or {R}.)";

#[rstest]
#[case::one_of(json!({ "OneOf": ["Blue", "Red"] }))]
#[case::one_of_in_the_other_order(json!({ "OneOf": ["Red", "Blue"] }))]
#[case::one_of_lists(json!({ "OneOfLists": [["Blue"], ["Red"]] }))]
#[case::one_of_its_two_types(json!({ "Fixed": ["Blue"] }))]
fn the_mana_of_the_land_types_is_dropped(#[case] mana: Value) {
    let (abilities, classes) = run(&land(ISLAND_MOUNTAIN, REMINDER), &json!([tap_for(&mana)]));
    assert_eq!(abilities, json!([]));
    assert_eq!(classes, [RepairClass::ImpliedMana]);
}

#[test]
fn a_creature_land_is_dropped_the_same() {
    let printed = land(
        "Land Creature \u{2014} Forest Dryad",
        "(This land isn't a spell, it's affected by summoning sickness, and it has \"{T}: Add {G}.\")",
    );
    let (abilities, _) = run(&printed, &json!([tap_for(&json!({ "Fixed": ["Green"] }))]));
    assert_eq!(abilities, json!([]));
}

#[test]
fn each_of_two_abilities_the_model_wrote_apart_is_dropped() {
    let both = json!([
        tap_for(&json!({ "Fixed": ["Blue"] })),
        tap_for(&json!({ "Fixed": ["Red"] }))
    ]);
    let (abilities, classes) = run(&land(ISLAND_MOUNTAIN, REMINDER), &both);
    assert_eq!(abilities, json!([]));
    assert_eq!(classes.len(), 2);
}

#[rstest]
#[case::both_mana_at_once(ISLAND_MOUNTAIN, REMINDER, json!({ "Fixed": ["Green", "Blue"] }))]
#[case::a_type_the_land_does_not_have(ISLAND_MOUNTAIN, REMINDER, json!({ "OneOf": ["Blue", "Black"] }))]
#[case::colorless(ISLAND_MOUNTAIN, REMINDER, json!({ "Fixed": ["Colorless"] }))]
#[case::any_color(ISLAND_MOUNTAIN, REMINDER, json!({ "AnyColor": { "Fixed": 1 } }))]
#[case::a_list_of_two_mana(ISLAND_MOUNTAIN, REMINDER, json!({ "OneOfLists": [["Blue", "Red"], ["Red"]] }))]
#[case::a_land_with_no_basic_type("Land", "{T}: Add {G}.", json!({ "Fixed": ["Green"] }))]
#[case::a_basic_type_but_no_land("Creature \u{2014} Forest Dryad", "", json!({ "Fixed": ["Green"] }))]
#[case::a_land_that_prints_its_mana("Land \u{2014} Forest", "{T}: Add {G}.", json!({ "Fixed": ["Green"] }))]
fn other_mana_stays(#[case] type_line: &str, #[case] text: &str, #[case] mana: Value) {
    let proposed = json!([tap_for(&mana)]);
    let (abilities, classes) = run(&land(type_line, text), &proposed);
    assert_eq!(abilities, proposed);
    assert!(!classes.contains(&RepairClass::ImpliedMana));
}

#[rstest]
#[case::a_sorcery_speed_ability(json!({ "Activated": { "cost": ["Tap"], "timing": "Sorcery", "effects": [{ "AddMana": { "mana": { "Fixed": ["Blue"] } } }] } }))]
#[case::a_keyword(json!({ "Keyword": "Trample" }))]
#[case::an_ability_that_does_not_parse(json!({ "Activated": 3 }))]
fn another_ability_stays(#[case] ability: Value) {
    let proposed = json!([ability]);
    let (abilities, classes) = run(&land(ISLAND_MOUNTAIN, REMINDER), &proposed);
    assert_eq!(abilities, proposed);
    assert!(!classes.contains(&RepairClass::ImpliedMana));
}

#[test]
fn a_proposal_without_the_mana_is_unchanged() {
    let printed = land(
        ISLAND_MOUNTAIN,
        "({T}: Add {U} or {R}.)\nThis land enters tapped.",
    );
    let proposed = json!([{ "Keyword": "Flash" }]);
    let (abilities, classes) = run(&printed, &proposed);
    assert_eq!(abilities, proposed);
    assert!(classes.is_empty());
}

#[test]
fn what_stands_beside_the_dropped_mana_stays_in_order() {
    let printed = land(
        ISLAND_MOUNTAIN,
        "({T}: Add {U} or {R}.)\nThis land enters tapped.",
    );
    let other = json!({ "Keyword": "Flash" });
    let proposed = json!([other, tap_for(&json!({ "OneOf": ["Blue", "Red"] })), other]);
    let (abilities, _) = run(&printed, &proposed);
    assert_eq!(abilities, json!([other, other]));
}
