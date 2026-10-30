//! Goal: each repair of the second round (`Triggering` outside a trigger, the controller of a player or a bare target where the text names a group) changes a proposal only when the file and the printed text leave one reading, and leaves every other shape as it is.
use rstest::rstest;
use serde_json::{Value, json};

use super::{RepairClass, repair};
use crate::support::{PrintedCard, PrintedFace};

/// A one-face card as imported.
fn card(type_line: &str, text: &str) -> PrintedCard {
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

/// The abilities after repairing a proposal of those abilities, and the classes of the second round that repaired.
fn run(printed: &PrintedCard, abilities: &Value) -> (Value, Vec<RepairClass>) {
    let mut value = json!({ "faces": [{ "abilities": abilities }] });
    let classes = repair(&mut value, printed)
        .iter()
        .map(|repaired| repaired.class)
        .filter(|class| {
            matches!(
                class,
                RepairClass::TriggeringDeclared
                    | RepairClass::ControllerGroup
                    | RepairClass::StaticSubject
            )
        })
        .collect();
    (value["faces"][0]["abilities"].clone(), classes)
}

fn creature() -> Value {
    json!({ "what": { "Object": [{ "Type": "Creature" }] } })
}

#[rstest]
#[case::activated(
    "Artifact",
    "{T}: Destroy target creature.",
    json!([{ "Activated": { "cost": ["Tap"], "effects": [{ "Destroy": { "what": "Triggering" } }] } }]),
    json!([{ "Activated": {
        "cost": ["Tap"],
        "targets": [creature()],
        "effects": [{ "Destroy": { "what": { "Target": 0 } } }]
    } }])
)]
#[case::spell(
    "Sorcery",
    "Exile target creature.",
    json!([{ "Spell": { "effects": [{ "Exile": { "what": "Triggering" } }] } }]),
    json!([{ "Spell": {
        "targets": [creature()],
        "effects": [{ "Exile": { "what": { "Target": 0 } } }]
    } }])
)]
#[case::every_mention_is_the_one_target(
    "Artifact",
    "{T}: Untap target creature. Prevent all damage that would be dealt to that creature this turn.",
    json!([{ "Activated": { "cost": ["Tap"], "effects": [
        { "Untap": { "what": "Triggering" } },
        { "Shield": { "by": "Prevent", "when": { "Damage": { "to": "Triggering" } } } }
    ] } }]),
    json!([{ "Activated": {
        "cost": ["Tap"],
        "targets": [creature()],
        "effects": [
            { "Untap": { "what": { "Target": 0 } } },
            { "Shield": { "by": "Prevent", "when": { "Damage": { "to": { "Target": 0 } } } } }
        ]
    } }])
)]
#[case::after_a_declared_target(
    "Sorcery",
    "Tap target creature. Destroy target creature.",
    json!([{ "Spell": {
        "targets": [creature()],
        "effects": [{ "Tap": { "what": { "Target": 0 } } }, { "Destroy": { "what": "Triggering" } }]
    } }]),
    json!([{ "Spell": {
        "targets": [creature(), creature()],
        "effects": [{ "Tap": { "what": { "Target": 0 } } }, { "Destroy": { "what": { "Target": 1 } } }]
    } }])
)]
fn triggering_in_a_plain_ability_is_the_target_the_text_declares(
    #[case] type_line: &str,
    #[case] text: &str,
    #[case] before: Value,
    #[case] after: Value,
) {
    let (abilities, classes) = run(&card(type_line, text), &before);
    assert_eq!(abilities, after);
    assert!(classes.contains(&RepairClass::TriggeringDeclared));
}

#[rstest]
#[case::a_triggered_ability(
    "Creature \u{2014} Elf",
    "When ~ enters, destroy target creature.",
    json!([{ "Triggered": { "effects": [{ "Destroy": { "what": "Triggering" } }] } }])
)]
#[case::no_phrase(
    "Sorcery",
    "Exile it.",
    json!([{ "Spell": { "effects": [{ "Exile": { "what": "Triggering" } }] } }])
)]
#[case::more_phrases_than_targets(
    "Sorcery",
    "Exile target creature. Destroy target land.",
    json!([{ "Spell": { "effects": [{ "Exile": { "what": "Triggering" } }] } }])
)]
#[case::a_modal_spell(
    "Sorcery",
    "Choose one \u{2014}\n\u{2022} Exile target creature.\n\u{2022} Draw a card.",
    json!([{ "Spell": { "effects": [{ "Modal": { "modes": [{ "effects": [{ "Exile": { "what": "Triggering" } }] }] } }] } }])
)]
#[case::an_unused_declared_target_is_the_other_repair(
    "Sorcery",
    "Exile target creature.",
    json!([{ "Spell": { "targets": [creature()], "effects": [{ "Exile": { "what": "Triggering" } }] } }])
)]
fn triggering_is_left_when_the_text_does_not_settle_it(
    #[case] type_line: &str,
    #[case] text: &str,
    #[case] before: Value,
) {
    let (abilities, classes) = run(&card(type_line, text), &before);
    assert!(!classes.contains(&RepairClass::TriggeringDeclared));
    if before[0].get("Spell").is_none() || before[0]["Spell"].get("targets").is_none() {
        assert_eq!(abilities, before);
    }
}

fn lands_you_control() -> Value {
    json!({ "Each": [{ "Type": "Land" }, { "ControlledBy": "You" }] })
}

#[rstest]
#[case::spell(
    "Instant",
    "Untap all lands you control.",
    json!([{ "Spell": { "effects": [{ "Untap": { "what": { "ControllerOf": "You" } } }] } }]),
    json!([{ "Spell": { "effects": [{ "Untap": { "what": lands_you_control() } }] } }])
)]
#[case::triggered_after_the_event(
    "Artifact \u{2014} Equipment",
    "Whenever equipped creature deals combat damage to a player, that player discards a card and you untap all lands you control.",
    json!([{ "Triggered": { "effects": [
        { "Discard": { "who": "Triggering" } },
        { "Untap": { "what": { "ControllerOf": "You" } } }
    ] } }]),
    json!([{ "Triggered": { "effects": [
        { "Discard": { "who": "Triggering" } },
        { "Untap": { "what": lands_you_control() } }
    ] } }])
)]
fn the_controller_of_a_player_is_the_group_the_text_names(
    #[case] type_line: &str,
    #[case] text: &str,
    #[case] before: Value,
    #[case] after: Value,
) {
    let (abilities, classes) = run(&card(type_line, text), &before);
    assert_eq!(abilities, after);
    assert_eq!(classes, vec![RepairClass::ControllerGroup]);
}

#[rstest]
#[case::two_groups(
    "Untap all lands you control and all creatures you control.",
    json!([{ "Spell": { "effects": [{ "Untap": { "what": { "ControllerOf": "You" } } }] } }])
)]
#[case::no_group(
    "Untap target land.",
    json!([{ "Spell": { "effects": [{ "Untap": { "what": { "ControllerOf": "You" } } }] } }])
)]
#[case::the_group_of_the_other_player(
    "Untap all lands your opponents control.",
    json!([{ "Spell": { "effects": [{ "Untap": { "what": { "ControllerOf": "You" } } }] } }])
)]
#[case::two_slots(
    "Untap all lands you control.",
    json!([{ "Spell": { "effects": [
        { "Untap": { "what": { "ControllerOf": "You" } } },
        { "Tap": { "what": { "ControllerOf": "You" } } }
    ] } }])
)]
#[case::a_valid_group(
    "Untap all lands you control.",
    json!([{ "Spell": { "effects": [{ "Untap": { "what": lands_you_control() } }] } }])
)]
fn the_controller_of_a_player_is_left_when_the_text_does_not_settle_it(
    #[case] text: &str,
    #[case] before: Value,
) {
    let (abilities, classes) = run(&card("Instant", text), &before);
    assert_eq!(abilities, before);
    assert_eq!(classes, Vec::<RepairClass>::new());
}

fn artifacts_you_control() -> Value {
    json!({ "Each": [{ "Type": "Artifact" }, { "ControlledBy": "You" }] })
}

fn creatures_you_control() -> Value {
    json!({ "Each": [{ "Type": "Creature" }, { "ControlledBy": "You" }] })
}

#[rstest]
#[case::one_static(
    "Artifacts you control have hexproof.",
    json!([{ "Static": { "affected": { "Target": 0 }, "mods": [{ "AddKeyword": "Hexproof" }] } }]),
    json!([{ "Static": { "affected": artifacts_you_control(), "mods": [{ "AddKeyword": "Hexproof" }] } }])
)]
#[case::two_statics_in_order(
    "Artifacts you control have hexproof.\nCreatures you control have lifelink.",
    json!([
        { "Static": { "affected": { "Target": 0 }, "mods": [{ "AddKeyword": "Hexproof" }] } },
        { "Static": { "affected": { "Target": 0 }, "mods": [{ "AddKeyword": "Lifelink" }] } }
    ]),
    json!([
        { "Static": { "affected": artifacts_you_control(), "mods": [{ "AddKeyword": "Hexproof" }] } },
        { "Static": { "affected": creatures_you_control(), "mods": [{ "AddKeyword": "Lifelink" }] } }
    ])
)]
fn a_bare_target_of_a_static_ability_is_the_group_its_line_starts_with(
    #[case] text: &str,
    #[case] before: Value,
    #[case] after: Value,
) {
    let (abilities, classes) = run(&card("Artifact", text), &before);
    assert_eq!(abilities, after);
    assert!(
        classes
            .iter()
            .all(|class| *class == RepairClass::StaticSubject)
    );
    assert_eq!(classes.len(), after.as_array().map_or(0, Vec::len));
}

#[rstest]
#[case::no_group_line(
    "Until end of turn, target creature gains flying.",
    json!([{ "Static": { "affected": { "Target": 0 }, "mods": [{ "AddKeyword": "Flying" }] } }])
)]
#[case::more_statics_than_lines(
    "Artifacts you control have hexproof.",
    json!([
        { "Static": { "affected": { "Target": 0 }, "mods": [{ "AddKeyword": "Hexproof" }] } },
        { "Static": { "affected": { "Target": 0 }, "mods": [{ "AddKeyword": "Lifelink" }] } }
    ])
)]
#[case::the_static_declares_a_target(
    "Artifacts you control have hexproof.",
    json!([{ "Static": { "targets": [creature()], "affected": { "Target": 0 }, "mods": [{ "AddKeyword": "Hexproof" }] } }])
)]
#[case::already_a_group(
    "Artifacts you control have hexproof.",
    json!([{ "Static": { "affected": artifacts_you_control(), "mods": [{ "AddKeyword": "Hexproof" }] } }])
)]
fn a_bare_target_of_a_static_ability_is_left_when_the_text_does_not_settle_it(
    #[case] text: &str,
    #[case] before: Value,
) {
    let (abilities, classes) = run(&card("Artifact", text), &before);
    assert_eq!(abilities, before);
    assert_eq!(classes, Vec::<RepairClass>::new());
}
