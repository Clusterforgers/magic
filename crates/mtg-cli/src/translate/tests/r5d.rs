//! Goal: whole translations the offer refused for a fault of the translation read as the card prints: Fuse on each half, the commander rule of a `Text:` line, one cause for "whenever you put", a valiant source named by its controller, and the champion shape the reader writes.
#![allow(clippy::unwrap_used)]

use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, translate};

/// A refusal is a gap name: one line, no separator of the cards file, names and numbers folded.
#[rstest]
#[case::name(
    "no imported card is named `Nissa's Chosen`",
    "no imported card is named `_`"
)]
#[case::numbers(
    "target 12 does not exist: 3 target(s)",
    "target # does not exist: # target(s)"
)]
#[case::separators("a; b\tc", "a  b c")]
fn a_refusal_message_is_a_gap_name(#[case] message: &str, #[case] gap: &str) {
    assert_eq!(super::super::stored::shape(message), gap);
}

#[rstest]
#[case::two_halves(2)]
#[case::one_face(1)]
fn fuse_is_on_every_half_of_a_split_card(#[case] faces: usize) {
    let first = "Name:Left\nManaCost:1 R\nTypes:Sorcery\nK:Fuse\nA:SP$ Draw | Defined$ You | SpellDescription$ Draw a card.\n";
    let second = "\nALTERNATE\n\nName:Right\nManaCost:1 G\nTypes:Sorcery\nA:SP$ GainLife | LifeAmount$ 2 | SpellDescription$ You gain 2 life.\n";
    let text = if faces == 2 {
        format!("{first}AlternateMode:Split\n{second}")
    } else {
        first.to_owned()
    };
    let card = translate(&text);
    let fuse = json!({"Keyword": "Fuse"});
    assert_eq!(card.faces.len(), faces);
    for face in &card.faces {
        assert_eq!(
            face.abilities.iter().filter(|a| **a == fuse).count(),
            1,
            "{:?}",
            face.abilities
        );
    }
}

#[rstest]
#[case::commander("Text:CARDNAME can be your commander.", Some(json!({"Rule": {"rule": "CanBeCommander"}})))]
#[case::other_text(
    "Text:Cast CARDNAME only during combat after blockers are declared.",
    None
)]
fn the_commander_text_line_is_the_commander_rule(#[case] line: &str, #[case] rule: Option<Value>) {
    let text = format!("Name:Subject\nTypes:Legendary Planeswalker Subject\nLoyalty:3\n{line}\n");
    assert_eq!(abilities(&text), rule.into_iter().collect::<Vec<_>>());
}

#[rstest]
#[case::written_and_printed("ValidSource$ You | ")]
#[case::printed_only("")]
fn whenever_you_put_names_one_cause(#[case] source: &str) {
    let text = format!(
        "Name:Subject\nTypes:Creature Ant\nPT:1/1\nT:Mode$ CounterAdded | ValidCard$ Creature | {source}TriggerZones$ Battlefield | CounterType$ P1P1 | Execute$ D | TriggerDescription$ Whenever you put a +1/+1 counter on a creature, draw a card.\nSVar:D:DB$ Draw | Defined$ You\n"
    );
    let json = serde_json::to_string(&abilities(&text)).unwrap();
    assert_eq!(json.matches("\"Caused\"").count(), 1, "{json}");
    assert!(json.contains("\"by\":\"You\""), "{json}");
}

#[rstest]
#[case::valiant("Valiant$ True | ", json!([{"ControlledBy": "You"}]))]
#[case::not_counted("", json!([{"AnyOf": ["Spell", {"Ability": "Activated"}, {"Ability": "Triggered"}]}, {"ControlledBy": "You"}]))]
fn a_counted_targeting_names_its_source_by_the_controller(
    #[case] valiant: &str,
    #[case] by: Value,
) {
    let text = format!(
        "Name:Subject\nTypes:Creature Mouse\nPT:1/1\nT:Mode$ BecomesTarget | ValidTarget$ Card.Self | ValidSource$ SpellAbility.YouCtrl | TriggerZones$ Battlefield | {valiant}Execute$ D\nSVar:D:DB$ PutCounter | Defined$ Self | CounterType$ P1P1 | CounterNum$ 1\n"
    );
    let json = serde_json::to_value(abilities(&text)).unwrap();
    let found = json.pointer("/0/Triggered/when").unwrap().to_string();
    assert!(found.contains(&format!("\"by\":{by}")), "{found}");
}
