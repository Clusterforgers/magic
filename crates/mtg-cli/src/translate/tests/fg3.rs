//! Goal: the mappers of the fg3 family turn invented Forge-format lines into the DSL nodes the engine has and leave the rest as gaps.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps, normal};

#[rstest]
#[case::conjunction_among_alternatives(
    "Name:T\nTypes:Sorcery\nA:SP$ Destroy | ValidTgts$ Creature.Red+withFlying,Artifact\n",
    json!({"Spell": {"targets": [{"what": {"Object": [{"AnyOf": [
        {"Not": {"AnyOf": [{"Not": {"Type": "Creature"}}, {"Not": {"Color": "Red"}}, {"Not": {"HasKeyword": "Flying"}}]}},
        {"Type": "Artifact"}]}]}}],
        "effects": [{"Destroy": {"what": {"Target": 0}}}]}})
)]
fn a_construct_with_a_node_maps(#[case] script: &str, #[case] expected: Value) {
    let mut found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    assert_eq!(normal(found.remove(0)), normal(expected));
}
