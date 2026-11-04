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
#[case::each_opponent_by_a_count_of_their_own(
    "Name:T\nTypes:Sorcery\nA:SP$ RepeatEach | RepeatPlayers$ Player.Opponent | RepeatSubAbility$ Dmg\nSVar:Dmg:DB$ DealDamage | Defined$ Remembered | NumDmg$ X\nSVar:X:Count$Valid Island.RememberedPlayerCtrl\n",
    json!({"Spell": {"effects": [{"RepeatEach": {"over": "Opponents", "as": "each", "effects": [
        {"DealDamage": {"to": {"Bound": "each"}, "amount": {"Count": [{"Subtype": "Island"}, {"ControlledBy": {"Bound": "each"}}]}}}]}}]}})
)]
#[case::choose_a_color_then_sacrifice_creatures_of_it(
    "Name:T\nTypes:Sorcery\nA:SP$ ChooseColor | Defined$ You | SubAbility$ Sac\nSVar:Sac:DB$ DestroyAll | ValidCards$ Creature.ChosenColor\n",
    json!({"Spell": {"effects": [
        {"ChooseValue": {"kind": "Color", "as": "chosen"}},
        {"Destroy": {"what": {"Each": [{"Type": "Creature"}, {"ChosenAs": "chosen"}]}}}]}})
)]
fn a_construct_with_a_node_maps(#[case] script: &str, #[case] expected: Value) {
    let mut found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    assert_eq!(normal(found.remove(0)), normal(expected));
}
