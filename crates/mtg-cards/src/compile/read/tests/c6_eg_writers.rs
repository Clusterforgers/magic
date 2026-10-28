//! Goal: the writers of the engine-gap pass give the oracle wording of a payment of the life of the event, and a wrong amount, cost or event never reads back.
use rstest::rstest;

use super::*;

/// Whether some wording the checker accepts for the rules is the clause.
fn c6_eg_written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const C6_EG_NIV: &str = r#"{"Triggered":{"when":{"LifeGained":{"who":"You"}},"effects":[{"Pay":{"cost":[{"PayLife":"Triggering"}],"paid":[{"Draw":{"count":"Triggering"}}]}}]}}"#;
const C6_EG_FIXED: &str = r#"{"Triggered":{"when":{"LifeGained":{"who":"You"}},"effects":[{"Pay":{"cost":[{"PayLife":2}],"paid":[{"Draw":{"count":"Triggering"}}]}}]}}"#;
const C6_EG_ENERGY: &str = r#"{"Triggered":{"when":{"LifeGained":{"who":"You"}},"effects":[{"Pay":{"cost":[{"PayEnergy":"Triggering"}],"paid":[{"Draw":{"count":"Triggering"}}]}}]}}"#;

#[rstest]
#[case::that_much_life(
    C6_EG_NIV,
    "Whenever you gain life, you may pay that much life. If you do, draw that many cards.",
    true
)]
#[case::fixed_life_is_not_that_much(
    C6_EG_FIXED,
    "Whenever you gain life, you may pay that much life. If you do, draw that many cards.",
    false
)]
#[case::energy_is_not_life(
    C6_EG_ENERGY,
    "Whenever you gain life, you may pay that much life. If you do, draw that many cards.",
    false
)]
#[case::another_event(
    C6_EG_NIV,
    "Whenever you lose life, you may pay that much life. If you do, draw that many cards.",
    false
)]
fn c6_eg_writes_the_oracle_wording(#[case] json: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(c6_eg_written(json, clause), reads, "{clause}");
}
