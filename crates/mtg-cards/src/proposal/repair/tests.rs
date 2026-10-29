//! Goal: each repair of the leftover slips (an undeclared target, a target no effect uses, an invented `X`, the controller of a player) changes a proposal only when the file and the printed text leave one reading, and leaves every other shape as it is.
use rstest::rstest;
use serde_json::{Value, json};

use super::{RepairClass, repair};
use crate::support::{PrintedCard, PrintedFace};

/// A one-face card as imported.
fn card(type_line: &str, mana_cost: &str, text: &str) -> PrintedCard {
    PrintedCard {
        name: "Card".to_owned(),
        faces: vec![PrintedFace {
            name: "Card".to_owned(),
            type_line: type_line.to_owned(),
            mana_cost: mana_cost.to_owned(),
            oracle_text: text.to_owned(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// The abilities of the face and the classes of the repairs after repairing a proposal of those abilities.
fn run(printed: &PrintedCard, abilities: &Value) -> (Value, Vec<RepairClass>) {
    let mut value = json!({ "faces": [{ "abilities": abilities }] });
    let repairs = repair(&mut value, printed);
    let classes = repairs
        .iter()
        .map(|repaired| repaired.class)
        .filter(|class| {
            matches!(
                class,
                RepairClass::TargetDeclared
                    | RepairClass::TargetUnused
                    | RepairClass::InventedX
                    | RepairClass::ControllerOfPlayer
            )
        })
        .collect();
    (value["faces"][0]["abilities"].clone(), classes)
}

fn creature() -> Value {
    json!({ "what": { "Object": [{ "Type": "Creature" }] } })
}

#[rstest]
#[case::spell(
    "Instant",
    "Counter target spell.",
    json!([{ "Spell": { "effects": [{ "Counter": { "what": { "Target": 0 } } }] } }]),
    json!([{ "Spell": {
        "targets": [{ "what": { "Object": ["Spell"] } }],
        "effects": [{ "Counter": { "what": { "Target": 0 } } }]
    } }])
)]
#[case::activated_after_the_cost(
    "Artifact",
    "You may pay {1} less.\n{1}, {T}: Untap target creature.",
    json!([{ "Activated": { "cost": ["Tap"], "effects": [{ "Untap": { "what": { "Target": 0 } } }] } }]),
    json!([{ "Activated": {
        "cost": ["Tap"],
        "targets": [creature()],
        "effects": [{ "Untap": { "what": { "Target": 0 } } }]
    } }])
)]
#[case::triggered_after_the_event(
    "Creature \u{2014} Elf",
    "When ~ enters, destroy target creature.",
    json!([{ "Triggered": { "effects": [{ "Destroy": { "what": { "Target": 0 } } }] } }]),
    json!([{ "Triggered": {
        "targets": [creature()],
        "effects": [{ "Destroy": { "what": { "Target": 0 } } }]
    } }])
)]
#[case::after_an_ability_word(
    "Creature \u{2014} Elf",
    "Landfall \u{2014} Whenever a land you control enters, destroy target creature.",
    json!([{ "Triggered": { "effects": [{ "Destroy": { "what": { "Target": 0 } } }] } }]),
    json!([{ "Triggered": {
        "targets": [creature()],
        "effects": [{ "Destroy": { "what": { "Target": 0 } } }]
    } }])
)]
#[case::two_targets(
    "Creature \u{2014} Elf",
    "When ~ enters, destroy target creature and exile target artifact.",
    json!([{ "Triggered": { "effects": [
        { "Destroy": { "what": { "Target": 0 } } },
        { "Exile": { "what": { "Target": 1 } } }
    ] } }]),
    json!([{ "Triggered": {
        "targets": [creature(), { "what": { "Object": [{ "Type": "Artifact" }] } }],
        "effects": [
            { "Destroy": { "what": { "Target": 0 } } },
            { "Exile": { "what": { "Target": 1 } } }
        ]
    } }])
)]
#[case::the_declared_first_stays(
    "Creature \u{2014} Elf",
    "When ~ enters, destroy target creature and exile target artifact.",
    json!([{ "Triggered": {
        "targets": [creature()],
        "effects": [
            { "Destroy": { "what": { "Target": 0 } } },
            { "Exile": { "what": { "Target": 1 } } }
        ]
    } }]),
    json!([{ "Triggered": {
        "targets": [creature(), { "what": { "Object": [{ "Type": "Artifact" }] } }],
        "effects": [
            { "Destroy": { "what": { "Target": 0 } } },
            { "Exile": { "what": { "Target": 1 } } }
        ]
    } }])
)]
fn an_undeclared_target_is_declared_from_the_printed_phrase(
    #[case] type_line: &str,
    #[case] text: &str,
    #[case] before: Value,
    #[case] after: Value,
) {
    let (abilities, classes) = run(&card(type_line, "{1}", text), &before);
    assert_eq!(abilities, after);
    assert_eq!(classes, [RepairClass::TargetDeclared]);
}

#[rstest]
#[case::declared_already(
    "Instant",
    "Counter target spell.",
    json!([{ "Spell": {
        "targets": [{ "what": { "Object": ["Spell"] } }],
        "effects": [{ "Counter": { "what": { "Target": 0 } } }]
    } }])
)]
#[case::no_phrase_in_the_text(
    "Instant",
    "Counter that spell.",
    json!([{ "Spell": { "effects": [{ "Counter": { "what": { "Target": 0 } } }] } }])
)]
#[case::more_phrases_than_targets_used(
    "Instant",
    "Destroy target creature. Exile target artifact.",
    json!([{ "Spell": { "effects": [{ "Destroy": { "what": { "Target": 0 } } }] } }])
)]
#[case::fewer_phrases_than_targets_used(
    "Instant",
    "Destroy target creature.",
    json!([{ "Spell": { "effects": [
        { "Destroy": { "what": { "Target": 0 } } },
        { "Exile": { "what": { "Target": 1 } } }
    ] } }])
)]
#[case::a_gap_in_the_uses(
    "Creature \u{2014} Elf",
    "When ~ enters, destroy target creature and exile target artifact.",
    json!([{ "Triggered": { "effects": [{ "Exile": { "what": { "Target": 1 } } }] } }])
)]
#[case::the_declared_target_differs_from_the_text(
    "Creature \u{2014} Elf",
    "When ~ enters, destroy target creature and exile target artifact.",
    json!([{ "Triggered": {
        "targets": [{ "what": "Player" }],
        "effects": [
            { "Destroy": { "what": { "Target": 0 } } },
            { "Exile": { "what": { "Target": 1 } } }
        ]
    } }])
)]
#[case::a_static_ability(
    "Artifact",
    "Target creature has flying.",
    json!([{ "Static": { "affected": { "Target": 0 }, "mods": [{ "AddKeyword": "Flying" }] } }])
)]
#[case::the_permanent_has_no_spell_text(
    "Enchantment",
    "Counter target spell.",
    json!([{ "Spell": { "effects": [{ "Counter": { "what": { "Target": 0 } } }] } }])
)]
#[case::two_triggers_and_one_line(
    "Creature \u{2014} Elf",
    "When ~ enters, destroy target creature.",
    json!([
        { "Triggered": { "effects": [{ "Destroy": { "what": { "Target": 0 } } }] } },
        { "Triggered": { "effects": [{ "Destroy": { "what": { "Target": 0 } } }] } }
    ])
)]
#[case::a_modal_spell(
    "Instant",
    "Choose one \u{2014}\n\u{2022} Counter target spell.\n\u{2022} Draw a card.",
    json!([{ "Spell": { "effects": [{ "Modal": { "modes": [
        { "effects": [{ "Counter": { "what": { "Target": 0 } } }] },
        { "effects": [{ "Draw": { "count": 1 } }] }
    ] } }] } }])
)]
fn an_undeclared_target_is_left_when_the_text_does_not_settle_it(
    #[case] type_line: &str,
    #[case] text: &str,
    #[case] before: Value,
) {
    let (abilities, classes) = run(&card(type_line, "{1}", text), &before);
    assert_eq!(abilities, before);
    assert!(
        !classes.contains(&RepairClass::TargetDeclared),
        "{classes:?}"
    );
}

#[rstest]
#[case::no_target_in_the_text(
    "Instant",
    "Draw a card.",
    json!([{ "Spell": {
        "targets": [creature()],
        "effects": [{ "Draw": { "count": 1, "who": "You" } }]
    } }]),
    json!([{ "Spell": {
        "targets": [],
        "effects": [{ "Draw": { "count": 1, "who": "You" } }]
    } }])
)]
#[case::the_used_target_moves_down(
    "Instant",
    "Destroy a creature.",
    json!([{ "Spell": {
        "targets": [creature(), { "what": "Player" }],
        "effects": [{ "LoseLife": { "who": { "Target": 1 }, "amount": 1 } }]
    } }]),
    json!([{ "Spell": {
        "targets": [{ "what": "Player" }],
        "effects": [{ "LoseLife": { "who": { "Target": 0 }, "amount": 1 } }]
    } }])
)]
#[case::the_only_effect_on_its_own_object(
    "Creature \u{2014} Elf",
    "When ~ enters, it deals 5 damage to target creature an opponent controls.",
    json!([{ "Triggered": {
        "targets": [creature()],
        "effects": [{ "DealDamage": { "amount": 5, "to": "This", "source": "This" } }]
    } }]),
    json!([{ "Triggered": {
        "targets": [creature()],
        "effects": [{ "DealDamage": { "amount": 5, "to": { "Target": 0 }, "source": "This" } }]
    } }])
)]
fn a_target_no_effect_uses_is_dropped_or_wired(
    #[case] type_line: &str,
    #[case] text: &str,
    #[case] before: Value,
    #[case] after: Value,
) {
    let (abilities, classes) = run(&card(type_line, "{1}", text), &before);
    assert_eq!(abilities, after);
    assert_eq!(classes, [RepairClass::TargetUnused]);
}

#[rstest]
#[case::text_has_target_and_no_own_object(
    "Instant",
    "Destroy target creature.",
    json!([{ "Spell": {
        "targets": [creature()],
        "effects": [{ "Draw": { "count": 1, "who": "You" } }]
    } }])
)]
#[case::two_effects_on_their_own_object(
    "Creature \u{2014} Elf",
    "When ~ enters, target creature gets +1/+1 and ~ gets +1/+1.",
    json!([{ "Triggered": {
        "targets": [creature()],
        "effects": [
            { "Continuous": { "affected": "This", "mods": [] } },
            { "Continuous": { "affected": "Triggering", "mods": [] } }
        ]
    } }])
)]
#[case::two_unused_targets_and_a_phrase(
    "Instant",
    "Destroy target creature.",
    json!([{ "Spell": {
        "targets": [creature(), creature()],
        "effects": [{ "Destroy": { "what": "This" } }]
    } }])
)]
#[case::a_player_target_is_not_wired(
    "Creature \u{2014} Elf",
    "When ~ enters, target player loses 2 life.",
    json!([{ "Triggered": {
        "targets": [{ "what": "Player" }],
        "effects": [{ "Destroy": { "what": "This" } }]
    } }])
)]
#[case::the_target_is_used(
    "Instant",
    "Draw a card.",
    json!([{ "Spell": {
        "targets": [creature()],
        "effects": [{ "Destroy": { "what": { "Target": 0 } } }]
    } }])
)]
#[case::the_line_is_not_settled(
    "Creature \u{2014} Elf",
    "When ~ enters, draw a card.\nWhen ~ dies, draw a card.",
    json!([{ "Triggered": {
        "targets": [creature()],
        "effects": [{ "Draw": { "count": 1 } }]
    } }])
)]
fn a_target_no_effect_uses_is_left_when_the_text_does_not_settle_it(
    #[case] type_line: &str,
    #[case] text: &str,
    #[case] before: Value,
) {
    let (abilities, classes) = run(&card(type_line, "{1}", text), &before);
    assert_eq!(abilities, before);
    assert!(classes.is_empty(), "{classes:?}");
}

#[rstest]
#[case::damage(
    "Land",
    "",
    "Channel \u{2014} {2}{W}, Discard this card: It deals 4 damage to each player.",
    json!([{ "Activated": { "effects": [{ "DealDamage": { "amount": "X", "to": "EachPlayer" } }] } }]),
    json!([{ "Activated": { "effects": [{ "DealDamage": { "amount": 4, "to": "EachPlayer" } }] } }])
)]
#[case::a_number_word(
    "Land",
    "",
    "When this land enters, surveil one.",
    json!([{ "Triggered": { "effects": [{ "Surveil": { "count": "X" } }] } }]),
    json!([{ "Triggered": { "effects": [{ "Surveil": { "count": 1 } }] } }])
)]
#[case::in_a_branch(
    "Land",
    "",
    "When this land enters, return a land you control to its owner's hand. If another Desert was returned this way, surveil 1.",
    json!([{ "Triggered": { "effects": [
        { "Move": { "what": "This", "to": "Hand" } },
        { "If": { "then": [{ "Surveil": { "count": "X" } }] } }
    ] } }]),
    json!([{ "Triggered": { "effects": [
        { "Move": { "what": "This", "to": "Hand" } },
        { "If": { "then": [{ "Surveil": { "count": 1 } }] } }
    ] } }])
)]
fn an_invented_x_is_the_number_the_text_states(
    #[case] type_line: &str,
    #[case] mana_cost: &str,
    #[case] text: &str,
    #[case] before: Value,
    #[case] after: Value,
) {
    let (abilities, classes) = run(&card(type_line, mana_cost, text), &before);
    assert_eq!(abilities, after);
    assert_eq!(classes, [RepairClass::InventedX]);
}

#[rstest]
#[case::a_real_x_in_the_cost(
    "{X}{R}",
    "~ deals X damage to you.",
    json!([{ "Spell": { "effects": [{ "DealDamage": { "amount": "X", "to": "You" } }] } }])
)]
#[case::an_x_in_the_text(
    "",
    "Search your library for a creature card with mana value X or less, where X is 2.",
    json!([{ "Spell": { "effects": [{ "Search": { "count": "X" } }] } }])
)]
#[case::two_numbers(
    "",
    "Deal 3 damage to each of 2 creatures.",
    json!([{ "Spell": { "effects": [{ "DealDamage": { "amount": "X" } }] } }])
)]
#[case::no_number(
    "",
    "Deal damage equal to the chosen number.",
    json!([{ "Spell": { "effects": [{ "DealDamage": { "amount": "X" } }] } }])
)]
#[case::an_x_in_an_event(
    "",
    "Whenever an opponent casts a spell with mana value equal to the chosen number, that player loses 2 life.",
    json!([{ "Triggered": {
        "when": { "Cast": { "what": [{ "Compare": { "to": "X" } }] } },
        "effects": [{ "LoseLife": { "amount": 2 } }]
    } }])
)]
#[case::an_x_in_a_condition_beside_an_amount(
    "",
    "Whenever an opponent casts a spell, that player loses 2 life.",
    json!([{ "Triggered": {
        "when": { "Cast": { "what": [{ "Compare": { "to": "X" } }] } },
        "effects": [{ "LoseLife": { "amount": "X" } }]
    } }])
)]
fn a_real_or_unsettled_x_is_left_alone(
    #[case] mana_cost: &str,
    #[case] text: &str,
    #[case] before: Value,
) {
    let (abilities, classes) = run(&card("Sorcery", mana_cost, text), &before);
    assert_eq!(abilities, before);
    assert!(!classes.contains(&RepairClass::InventedX), "{classes:?}");
}

#[rstest]
#[case::damage_to_opponents(
    json!([{ "Replacement": { "when": { "Damage": { "to": { "ControllerOf": "Opponents" } } } } }]),
    json!([{ "Replacement": { "when": { "Damage": { "to": "Opponents" } } } }])
)]
#[case::exile_top(
    json!([{ "Triggered": { "effects": [{ "ExileTop": { "count": 20, "who": { "ControllerOf": "Defending" } } }] } }]),
    json!([{ "Triggered": { "effects": [{ "ExileTop": { "count": 20, "who": "Defending" } }] } }])
)]
fn the_controller_of_a_player_in_a_player_field_is_the_player(
    #[case] before: Value,
    #[case] after: Value,
) {
    let (abilities, classes) = run(&card("Creature", "{1}", ""), &before);
    assert_eq!(abilities, after);
    assert_eq!(classes, [RepairClass::ControllerOfPlayer]);
}

#[rstest]
#[case::an_object_slot(
    json!([{ "Triggered": { "effects": [{ "Untap": { "what": { "ControllerOf": "You" } } }] } }])
)]
#[case::the_controller_of_an_object(
    json!([{ "Triggered": { "effects": [{ "Draw": { "count": 1, "who": { "ControllerOf": { "Target": 0 } } } }] } }])
)]
#[case::a_selector(
    json!([{ "Static": { "affected": [{ "Is": { "ControllerOf": "You" } }] } }])
)]
fn the_controller_of_anything_else_is_left_alone(#[case] before: Value) {
    let (abilities, classes) = run(&card("Creature", "{1}", ""), &before);
    assert_eq!(abilities, before);
    assert!(classes.is_empty(), "{classes:?}");
}
