//! Goal: the turn-count and named-part amounts translate into the DSL's `Amount`, every translated amount parses as one, and what the DSL cannot count stays a gap.
use mtg_cards::value::Amount;
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn drawing(svars: &str, amount: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ Draw | NumCards$ {amount}\n{svars}\n")
}

fn count(text: &str) -> Value {
    let found = abilities(text);
    found[0]["Spell"]["effects"][0]["Draw"]["count"].clone()
}

#[rstest]
#[case::life_gained("SVar:X:Count$LifeYouGainedThisTurn", json!({"Tallied": {"who": "You", "what": "LifeGainedTotal"}}))]
#[case::attackers("SVar:X:Count$AttackersDeclared", json!({"Tallied": {"who": "You", "what": "Attackers"}}))]
#[case::drew("SVar:X:Count$YouDrewThisTurn", json!({"Tallied": {"who": "You", "what": "Drawn"}}))]
#[case::opponents_lost("SVar:X:Count$LifeOppsLostThisTurn", json!({"Tallied": {"who": "Opponents", "what": "LifeLost"}}))]
#[case::morbid(
    "SVar:X:Count$Morbid.2.1",
    json!({"When": {"condition": {"Compare": {"left": {"Tallied": {"who": "EachPlayer", "what": "CreaturesDied"}}, "op": "Ge", "right": 1}}, "then": 2, "otherwise": 1}})
)]
#[case::cast_by_you(
    "SVar:X:Count$ThisTurnCast_Card.YouCtrl",
    json!({"Tally": {"event": "SpellCast", "who": "EachPlayer", "filter": [{"ControlledBy": "You"}]}})
)]
#[case::cast_by_opponents(
    "SVar:X:Count$ThisTurnCast_Card.OppCtrl",
    json!({"Tally": {"event": "SpellCast", "who": "EachPlayer", "filter": [{"ControlledBy": "Opponents"}]}})
)]
#[case::named_part_plus_part(
    "SVar:X:SVar$Y/Plus.Z\nSVar:Y:Count$YouDrewThisTurn\nSVar:Z:Number$2",
    json!({"Sum": [{"Tallied": {"who": "You", "what": "Drawn"}}, 2]})
)]
#[case::named_part_minus_part(
    "SVar:X:SVar$Y/Minus.Z\nSVar:Y:Count$YouDrewThisTurn\nSVar:Z:Count$LifeYouGainedThisTurn",
    json!({"Sum": [{"Tallied": {"who": "You", "what": "Drawn"}},
        {"Times": {"by": -1, "of": {"Tallied": {"who": "You", "what": "LifeGainedTotal"}}}}]})
)]
#[case::named_part_doubled(
    "SVar:X:SVar$Y/Twice\nSVar:Y:Count$YouDrewThisTurn",
    json!({"Times": {"by": 2, "of": {"Tallied": {"who": "You", "what": "Drawn"}}}})
)]
fn turn_counts_and_named_parts(#[case] svars: &str, #[case] expected: Value) {
    let found = count(&drawing(svars, "X"));
    assert_eq!(found, expected);
    assert!(serde_json::from_value::<Amount>(found).is_ok());
}

#[rstest]
#[case::resolved("SVar:X:Count$ResolvedThisTurn", "amount:Count$ResolvedThisTurn")]
#[case::limit(
    "SVar:X:Count$LifeYouGainedThisTurn/LimitMax.1",
    "amount:Count/LimitMax"
)]
#[case::cast_from_exile(
    "SVar:X:Count$ThisTurnCast_Card.YouCtrl+bogusProperty",
    "valid:bogusProperty"
)]
fn what_the_dsl_cannot_count_is_a_gap(#[case] svars: &str, #[case] gap: &str) {
    assert_eq!(gaps(&drawing(svars, "X")), vec![gap.to_owned()]);
}
