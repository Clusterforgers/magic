//! Goal: the rmt4 wordings ("When this creature enters, if it was bargained, ...") read a Bargain card's enters trigger back to the same rule, and a trigger of another condition or event still reads Differs.
use rstest::rstest;

use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

/// An enters trigger of this permanent under `condition` doing `effects`.
fn enters(condition: &str, effects: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"if":{condition},"effects":[{effects}]}}}}"#
    )
}

const DRAIN: &str = r#"{"LoseLife":{"who":"Opponents","amount":3}},{"GainLife":{"amount":3}}"#;
const DRAIN_TEXT: &str = "When this creature enters, if it was bargained, each opponent loses 3 life and you gain 3 life.";

#[rstest]
#[case::bargained(enters(r#""AdditionalCostPaid""#, DRAIN), DRAIN_TEXT, true)]
#[case::kicked_is_not_bargained(enters(r#""Kicked""#, DRAIN), DRAIN_TEXT, false)]
#[case::not_bargained(enters(r#"{"Not":"AdditionalCostPaid"}"#, DRAIN), DRAIN_TEXT, false)]
#[case::other_amount(
    enters(r#""AdditionalCostPaid""#, DRAIN),
    "When this creature enters, if it was bargained, each opponent loses 2 life and you gain 3 life.",
    false
)]
#[case::dies_is_not_enters(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":["This"],"from":"Battlefield","to":"Graveyard"}},"if":"AdditionalCostPaid","effects":[{"LoseLife":{"who":"Opponents","amount":3}},{"GainLife":{"amount":3}}]}}"#.to_owned(),
    DRAIN_TEXT,
    false
)]
#[case::another_creature_entering(
    r#"{"Triggered":{"when":{"ZoneChange":{"what":[{"Type":"Creature"},"Other"],"to":"Battlefield"}},"if":"AdditionalCostPaid","effects":[{"LoseLife":{"who":"Opponents","amount":3}},{"GainLife":{"amount":3}}]}}"#.to_owned(),
    DRAIN_TEXT,
    false
)]
fn rmt4_bargained_enters_trigger(#[case] json: String, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(&json, clause), reads, "{json}");
}
