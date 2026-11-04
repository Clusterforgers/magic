//! Goal: piece P2 of the rules plan translates its constructs, one rstest table per rule.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn drawing(svars: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ Pump | ValidTgts$ Creature | SubAbility$ D\nSVar:D:DB$ Draw | NumCards$ X\n{svars}\n"
    )
}

fn drawn(text: &str) -> Value {
    let found = abilities(text);
    assert!(!found.is_empty(), "{:?}", gaps(text));
    found[0]["Spell"]["effects"]
        .as_array()
        .and_then(|effects| effects.iter().find_map(|e| e.get("Draw")))
        .map_or(Value::Null, |draw| draw["count"].clone())
}

#[rstest]
#[case::colors("Targeted$CardNumColors", json!({"Stat": {"of": {"Target": 0}, "stat": "Colors"}}))]
#[case::spent("Targeted$CastTotalManaSpent", json!({"Stat": {"of": {"Target": 0}, "stat": "ManaSpent"}}))]
#[case::lki("Targeted$CardPowerLKI", json!({"Stat": {"of": {"Target": 0}, "stat": "Power"}}))]
#[case::counters(
    "Targeted$CardCounters.P1P1",
    json!({"Counters": {"on": {"Target": 0}, "counter": "+1/+1"}})
)]
fn object_stat(#[case] expression: &str, #[case] expected: Value) {
    assert_eq!(drawn(&drawing(&format!("SVar:X:{expression}"))), expected);
}

#[rstest]
#[case("Targeted$CardBogus", "amount:Targeted$CardBogus")]
#[case(
    "Targeted$CardCounters.bad kind",
    "amount:Targeted$CardCounters.bad kind"
)]
fn object_stat_gap(#[case] expression: &str, #[case] gap: &str) {
    assert_eq!(
        gaps(&drawing(&format!("SVar:X:{expression}"))),
        vec![gap.to_owned()]
    );
}

#[test]
fn triggered_attacker_power() {
    let script = "Name:T\nTypes:Enchantment\nT:Mode$ Attacks | ValidCard$ Creature | TriggerZones$ Battlefield | Execute$ TrigDraw\nSVar:TrigDraw:DB$ Draw | NumCards$ X\nSVar:X:TriggeredAttacker$CardPower\n";
    let text = abilities(script)
        .first()
        .map(ToString::to_string)
        .unwrap_or_default();
    assert!(
        text.contains(r#""stat":"Power""#),
        "{text} {:?}",
        gaps(script)
    );
}
