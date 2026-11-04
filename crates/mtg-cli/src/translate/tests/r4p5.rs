//! Goal: round 4 piece 5 translates the activated-ability cost reduction and stays a gap on a value it cannot count.
use rstest::rstest;
use serde_json::json;

use super::{abilities, gaps};

fn land(reduce: &str) -> String {
    format!(
        "Name:T\nTypes:Artifact\nA:AB$ Draw | Cost$ 3 U | NumCards$ 1 | Defined$ You | ReduceCost$ {reduce}\nSVar:X:Count$Valid Creature.YouCtrl\n"
    )
}

#[rstest]
#[case::svar("X")]
#[case::inline("Count$Valid Creature.YouCtrl")]
fn a_counted_reduction_is_a_changed_cost_item(#[case] reduce: &str) {
    assert_eq!(gaps(&land(reduce)), Vec::<String>::new());
    let found = abilities(&land(reduce));
    let cost = &found[0]["Activated"]["cost"];
    assert_eq!(cost[1]["Changed"]["change"], json!("Reduce"));
    assert_eq!(cost[1]["Changed"]["mana"], json!("{1}"));
    assert!(cost[1]["Changed"]["per"].is_object());
}

#[test]
fn a_fixed_reduction_is_a_gap() {
    assert_eq!(gaps(&land("2")), ["key:AB.ReduceCost=fixed"]);
}

fn upto(who: &str) -> String {
    format!("Name:T\nTypes:Sorcery\nA:SP$ Draw | NumCards$ 2 | Defined$ {who} | Upto$ True\n")
}

#[test]
fn draw_up_to_picks_a_number_then_draws_it() {
    let found = abilities(&upto("You"));
    let effects = &found[0]["Spell"]["effects"];
    assert_eq!(effects[0]["ChooseValue"]["kind"], json!({"Number": [0, 2]}));
    let name = &effects[0]["ChooseValue"]["as"];
    assert_eq!(effects[1]["Draw"]["count"], json!({"ChosenAs": name}));
}

#[rstest]
#[case::opponents("Opponent")]
#[case::each("Player")]
fn draw_up_to_for_a_group_is_a_gap(#[case] who: &str) {
    assert_eq!(gaps(&upto(who)), ["key:Draw.Upto.group"]);
}

#[test]
fn a_delayed_draw_for_the_remembered_controller_binds_the_controller_first() {
    let found = abilities(
        "Name:T\nTypes:Instant\nA:SP$ Counter | TargetType$ Spell | ValidTgts$ Card | RememberTargets$ True | SubAbility$ Del\nSVar:Del:DB$ DelayedTrigger | Mode$ Phase | Phase$ Upkeep | ValidPlayer$ Player | RememberObjects$ RememberedController | Execute$ Dr\nSVar:Dr:DB$ Draw | Defined$ DelayTriggerRemembered\n",
    );
    let effects = &found[0]["Spell"]["effects"];
    let name = &effects[1]["Bind"]["as"];
    assert_eq!(
        effects[1]["Bind"]["what"],
        json!({"ControllerOf": {"Target": 0}})
    );
    assert_eq!(
        effects[2]["DelayedTrigger"]["effects"][0]["Draw"]["who"],
        json!({"Bound": name})
    );
}
