//! Goal: the card of a permanent that died, moved as the triggering object, writes as "that card" or "it" (the clause of a "dies" trigger); another event, another destination and another object do not write it.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn dies(who: &str, to: &str, from: &str) -> String {
    format!(
        r#"{{"Triggered":{{"effects":[{{"Move":{{"to":{to},"what":"Triggering"}}}}],"when":{{"ZoneChange":{{"from":"{from}","to":"Graveyard","what":[{who}]}}}}}}}}"#
    )
}

#[rstest]
#[case::attached_land(
    r#"{"Is":"Attached"}"#,
    r#""Hand""#,
    "When enchanted creature dies, return that card to its owner's hand."
)]
#[case::attacking(
    r#"{"Type":"Creature"},"Attacking",{"ControlledBy":"You"}"#,
    r#""Hand""#,
    "Whenever an attacking creature you control dies, return that card to its owner's hand."
)]
fn the_card_that_died_returns(#[case] who: &str, #[case] to: &str, #[case] text: &str) {
    let json = dies(who, to, "Battlefield");
    assert!(
        written(&json, text),
        "{:?}",
        written_variants(&ability(&json))
    );
}

#[rstest]
#[case::to_the_battlefield(r#"{"Is":"Attached"}"#, r#"{"Battlefield":{}}"#, "Battlefield")]
#[case::to_the_library(
    r#"{"Is":"Attached"}"#,
    r#"{"Library":{"place":"Top"}}"#,
    "Battlefield"
)]
#[case::from_the_hand(r#"{"Is":"Attached"}"#, r#""Hand""#, "Hand")]
fn another_move_does_not_write_the_hand_text(
    #[case] who: &str,
    #[case] to: &str,
    #[case] from: &str,
) {
    let json = dies(who, to, from);
    assert!(!written(
        &json,
        "When enchanted creature dies, return that card to its owner's hand."
    ));
}
