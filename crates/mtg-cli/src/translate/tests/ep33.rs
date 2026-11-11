//! Goal: the pieces of ep33 translate as the card prints them. "Whenever you collect evidence" (`Mode$ CollectEvidence`) is the action of the player it names, and `FaceDown$ True` onto the battlefield is a face-down move under the player `GainControl$` names; another key on the line, a face-down move elsewhere or one with set characteristics stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

fn trigger(line: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Elf\nT:{line} | TriggerZones$ Battlefield | Execute$ Go\nSVar:Go:DB$ Draw | NumCards$ 1\n"
    )
}

#[rstest]
#[case::you("ValidPlayer$ You", json!({"action": "CollectedEvidence", "who": "You"}))]
#[case::an_opponent(
    "ValidPlayer$ Opponent",
    json!({"action": "CollectedEvidence", "who": "Opponents"})
)]
fn ep33_collect_evidence_is_did(#[case] line: &str, #[case] node: Value) {
    let script = trigger(&format!("Mode$ CollectEvidence | {line}"));
    let expected = normal(json!({"Triggered": {"when": {"Did": node}, "effects": [{"Draw": {}}]}}));
    assert_eq!(abilities(&script), vec![expected]);
}

#[test]
fn ep33_collect_evidence_with_another_key_is_a_gap() {
    let script = trigger("Mode$ CollectEvidence | ValidPlayer$ You | Amount$ 4");
    assert!(!gaps(&script).is_empty());
}

/// Ashcloud Phoenix's dies trigger with `part` added to its move.
fn phoenix(part: &str, destination: &str) -> String {
    format!(
        "Name:T\nTypes:Creature Phoenix\nPT:4/1\nT:Mode$ ChangesZone | Origin$ Battlefield | Destination$ Graveyard | ValidCard$ Card.Self | Execute$ Back | TriggerDescription$ When CARDNAME dies, return it face down.\nSVar:Back:DB$ ChangeZone | Defined$ TriggeredNewCardLKICopy | Origin$ Graveyard | Destination$ {destination} | FaceDown$ True{part}\n"
    )
}

#[rstest]
#[case::under_you(" | GainControl$ True", json!({"under": "You"}))]
#[case::under_the_owner("", json!({}))]
fn ep33_a_card_returned_face_down(#[case] part: &str, #[case] enters: Value) {
    let script = phoenix(part, "Battlefield");
    assert_eq!(gaps(&script), Vec::<String>::new());
    let found = abilities(&script);
    assert_eq!(
        found[0]["Triggered"]["effects"],
        json!([{"Move": {"what": "This", "to": {"BattlefieldFaceDown": enters}}}])
    );
}

#[rstest]
#[case::tapped(" | Tapped$ True", "Battlefield")]
#[case::with_counters(" | WithCountersType$ P1P1", "Battlefield")]
#[case::a_set_power(" | FaceDownPower$ 3", "Battlefield")]
#[case::a_set_type(" | FaceDownSetType$ Land & Forest", "Battlefield")]
#[case::into_the_hand("", "Hand")]
#[case::not_true(" | GainControl$ True", "Battlefield")]
fn ep33_a_face_down_move_the_engine_lacks_is_a_gap(#[case] part: &str, #[case] destination: &str) {
    let script = if part == " | GainControl$ True" {
        phoenix(part, destination).replace("FaceDown$ True", "FaceDown$ False")
    } else {
        phoenix(part, destination)
    };
    assert!(!gaps(&script).is_empty(), "{:?}", abilities(&script));
}
