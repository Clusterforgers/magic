//! Goal: the Effect forms of round 4 are table rows: a remembered `CantRegenerate` static is the rule `CantBeRegenerated`
//! for the objects the effect remembers, for the duration of the effect.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(line: &str) -> String {
    format!("Name:T\nTypes:Instant\nA:SP$ {line}\n")
}

fn effects(script: &str) -> Value {
    let found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    found[0]["Spell"]["effects"].clone()
}

#[rstest]
#[case::target(
    "Effect | ValidTgts$ Creature | RememberObjects$ Targeted | ForgetOnMoved$ Battlefield | StaticAbilities$ NoRegen\nSVar:NoRegen:Mode$ CantRegenerate | ValidCard$ Card.IsRemembered",
    json!([{"Rule": {"rule": {"CantBeRegenerated": {"Target": 0}}, "duration": "UntilEndOfTurn"}}])
)]
fn a_remembered_cant_regenerate_static_is_a_rule(#[case] script: &str, #[case] expected: Value) {
    assert_eq!(effects(&spell(script)), expected);
}
