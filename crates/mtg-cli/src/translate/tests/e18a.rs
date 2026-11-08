//! Goal: pack e18a's turn's activations by kind read from Forge lines: `ThisTurnActivated_Activated.<kind>+YouCtrl` as the
//! tally of the kind, a source filter without a kind as the plain tally, and `AlternativeCost` of equip, cycling and unearth
//! abilities at {0} as a `FreeActivation` with its condition, `CastWithFlash` of equip as an `InstantActivation`; another player, a targeting filter, another kind or another
//! cost stays a gap.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

/// The card text with a draw ability that may be activated only if `svar` (as X) is at least 1.
fn gated(svar: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact\nA:AB$ Draw | Cost$ T | CheckSVar$ X | SpellDescription$ Draw a card.\nSVar:X:{svar}\n"
    )
}

fn tally(event: Value, filter: Value) -> Value {
    json!({"Tally": {"event": event, "who": "You", "filter": filter}})
}

#[rstest]
#[case::equip(
    "Count$ThisTurnActivated_Activated.Equip+YouCtrl",
    tally(json!({"KindActivated": "Equip"}), json!([]))
)]
#[case::cycling(
    "Count$ThisTurnActivated_Activated.Cycling+YouCtrl",
    tally(json!({"KindActivated": "Cycling"}), json!([]))
)]
#[case::loyalty_of_a_planeswalker(
    "Count$ThisTurnActivated_Activated.Loyalty+Planeswalker+YouCtrl",
    tally(json!({"KindActivated": "Loyalty"}), json!([]))
)]
#[case::unearth(
    "Count$ThisTurnActivated_Activated.Unearth+YouCtrl",
    tally(json!({"KindActivated": "Unearth"}), json!([]))
)]
#[case::of_an_artifact(
    "Count$ThisTurnActivated_Activated.Artifact+YouCtrl",
    tally(json!("Activated"), json!([{"Type": "Artifact"}]))
)]
fn activations_of_the_turn_read(#[case] svar: &str, #[case] expected: Value) {
    let text = gated(svar);
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(
        abilities(&text)[0]["Activated"]["condition"]["Compare"]["left"],
        expected
    );
}

#[rstest]
#[case::targeting("Count$ThisTurnActivated_Activated.YouCtrl+IsTargeting Valid Creature.YouCtrl")]
#[case::of_any_player("Count$ThisTurnActivated_Activated.Equip")]
#[case::exhaust("Count$ThisTurnActivated_Activated.Exhaust+YouCtrl")]
#[case::in_a_graveyard("Count$ThisTurnActivated_Activated.YouCtrl+inZoneGraveyard")]
fn other_activation_tallies_stay_gaps(#[case] svar: &str) {
    assert!(!gaps(&gated(svar)).is_empty(), "{svar}");
}

/// A card with the static line `line` and `X` the equip tally.
fn free(line: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact\nS:Mode$ AlternativeCost | {line} | Description$ D.\nSVar:X:Count$ThisTurnActivated_Activated.Equip+YouCtrl\n"
    )
}

const FIRST_EQUIP: &str = r#"{"Compare": {"left": {"Tally": {"event": {"KindActivated": "Equip"}, "filter": [], "who": "You"}}, "op": "Lt", "right": 1}}"#;

#[rstest]
#[case::first_equip(
    "ValidSA$ Activated.Equip | ValidPlayer$ You | Cost$ 0 | CheckSVar$ X | SVarCompare$ LT1",
    json!({"FreeActivation": {"kind": "Equip", "condition": serde_json::from_str::<Value>(FIRST_EQUIP).unwrap()}})
)]
#[case::every_cycling(
    "ValidSA$ Activated.Cycling | ValidPlayer$ You | Cost$ Discard<1/CARDNAME> | CostDesc$ 0",
    json!({"FreeActivation": {"kind": "Cycling"}})
)]
#[case::unearth(
    "ValidSA$ Activated.Unearth | ValidPlayer$ You | Cost$ 0",
    json!({"FreeActivation": {"kind": "Unearth"}})
)]
fn free_activations_read(#[case] line: &str, #[case] expected: Value) {
    let text = free(line);
    assert_eq!(gaps(&text), Vec::<String>::new());
    assert_eq!(abilities(&text)[0], expected);
}

#[rstest]
#[case::a_discard_for_equip("ValidSA$ Activated.Equip | ValidPlayer$ You | Cost$ Discard<1/CARDNAME> | CostDesc$ 0")]
#[case::mana("ValidSA$ Activated.Equip | ValidPlayer$ You | Cost$ 1")]
#[case::loyalty("ValidSA$ Activated.Loyalty | ValidPlayer$ You | Cost$ 0")]
#[case::another_kind("ValidSA$ Activated.PowerUp | ValidPlayer$ You | Cost$ 0")]
#[case::any_player("ValidSA$ Activated.Equip | Cost$ 0")]
fn other_alternative_activation_costs_stay_gaps(#[case] line: &str) {
    assert!(!gaps(&free(line)).is_empty(), "{line}");
}

fn flash(line: &str) -> String {
    format!("Name:T\nTypes:Artifact\nS:Mode$ CastWithFlash | {line} | Description$ D.\n")
}

#[rstest]
#[case::equip("ValidSA$ Activated.Equip | Caster$ You", Some(json!({"InstantActivation": {"kind": "Equip"}})))]
#[case::your_turn(
    "ValidSA$ Activated.Equip | Caster$ You | Condition$ PlayerTurn",
    Some(json!({"InstantActivation": {"kind": "Equip", "condition": "YourTurn"}}))
)]
#[case::loyalty("ValidCard$ Planeswalker.YouCtrl | ValidSA$ Activated.Loyalty | Caster$ You", None)]
#[case::any_player("ValidSA$ Activated.Equip", None)]
fn instant_activations_read(#[case] line: &str, #[case] expected: Option<Value>) {
    let text = flash(line);
    match expected {
        Some(expected) => {
            assert_eq!(gaps(&text), Vec::<String>::new());
            assert_eq!(abilities(&text)[0], expected);
        }
        None => assert!(!gaps(&text).is_empty(), "{line}"),
    }
}
