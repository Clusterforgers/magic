//! Goal: a delayed sacrifice takes every bound object, and a move of a bound object that later steps name again binds it anew.
use rstest::rstest;
use serde_json::{Value, json};

use super::abilities;
use crate::translate::rebind::rebind;

fn text(value: &Value) -> String {
    value.to_string()
}

#[rstest]
#[case::thatcher(
    "A:SP$ Token | TokenAmount$ 3 | TokenScript$ g_2_2_boar_trample | TokenOwner$ You | AtEOT$ Sacrifice"
)]
#[case::your_sacrifice(
    "A:SP$ Token | TokenAmount$ 2 | TokenScript$ g_2_2_boar_trample | TokenOwner$ You | AtEOT$ YourSacrifice"
)]
#[case::combat(
    "A:SP$ Token | TokenAmount$ 2 | TokenScript$ g_2_2_boar_trample | TokenOwner$ You | AtEOT$ SacrificeCombat"
)]
fn delayed_sacrifice_takes_all_tokens(#[case] line: &str) {
    let out = text(&json!(abilities(&format!(
        "Name:T\nTypes:Sorcery\n{line}\n"
    ))));
    assert!(
        out.contains(r#""count":{"Count":[{"Is":{"Bound":"token"}}]}"#),
        "{out}"
    );
}

#[rstest]
#[case::hoarding_dragon(
    "Name:T\nTypes:Creature Dragon\nT:Mode$ ChangesZone | Origin$ Any | Destination$ Battlefield | ValidCard$ Card.Self | Execute$ Go\nSVar:Go:DB$ ChangeZone | Origin$ Library | Destination$ Exile | ChangeType$ Artifact | ChangeNum$ 1 | RememberChanged$ True | SubAbility$ R\nSVar:R:DB$ ChangeZone | Defined$ Remembered | Origin$ Exile | Destination$ Hand\n",
    r#""Exile":{"as":"found_exiled","what":{"Bound":"found"}}"#
)]
#[case::fevered_suspicion(
    "Name:T\nTypes:Sorcery\nA:SP$ DigUntil | Defined$ Opponent | Valid$ Card.nonLand | FoundDestination$ Exile | RevealedDestination$ Exile | RememberFound$ True | SubAbility$ P\nSVar:P:DB$ Play | Defined$ Remembered | WithoutManaCost$ True | ValidSA$ Spell | Optional$ True | Amount$ All\n",
    r#""Exile":{"as":"found_exiled","what":{"Bound":"found"}}"#
)]
fn exile_of_a_found_card_rebinds_it(#[case] script: &str, #[case] step: &str) {
    let out = text(&json!(abilities(script)));
    assert!(out.contains(step), "{out} {:?}", super::gaps(script));
}

#[rstest]
#[case::read_later(
    json!([{"Exile": {"what": {"Bound": "a"}}}, {"Remember": {"what": {"Bound": "a"}, "slot": "s"}}]),
    json!([{"Exile": {"what": {"Bound": "a"}, "as": "a_exiled"}}, {"Remember": {"what": {"Bound": "a_exiled"}, "slot": "s"}}])
)]
#[case::branch_does_not_bind_outside(
    json!([{"May": {"effects": [{"Exile": {"what": {"Bound": "a"}}}]}}, {"Draw": {"count": {"Count": [{"Is": {"Bound": "a"}}]}}}]),
    json!([{"May": {"effects": [{"Exile": {"what": {"Bound": "a"}}}]}}, {"Draw": {"count": {"Count": [{"Is": {"Bound": "a"}}]}}}])
)]
#[case::inside_branch(
    json!([{"May": {"effects": [{"Exile": {"what": {"Bound": "a"}}}, {"Remember": {"what": {"Bound": "a"}, "slot": "s"}}]}}]),
    json!([{"May": {"effects": [{"Exile": {"what": {"Bound": "a"}, "as": "a_exiled"}}, {"Remember": {"what": {"Bound": "a_exiled"}, "slot": "s"}}]}}])
)]
#[case::move_is_left_alone(
    json!([{"Move": {"what": {"Bound": "a"}, "to": "Hand"}}, {"Remember": {"what": {"Bound": "a"}, "slot": "s"}}]),
    json!([{"Move": {"what": {"Bound": "a"}, "to": "Hand"}}, {"Remember": {"what": {"Bound": "a"}, "slot": "s"}}])
)]
#[case::never_read(
    json!([{"Exile": {"what": {"Bound": "a"}}}, {"Draw": {}}]),
    json!([{"Exile": {"what": {"Bound": "a"}}}, {"Draw": {}}])
)]
#[case::read_before_only(
    json!([{"Remember": {"what": {"Bound": "a"}, "slot": "s"}}, {"Exile": {"what": {"Bound": "a"}}}]),
    json!([{"Remember": {"what": {"Bound": "a"}, "slot": "s"}}, {"Exile": {"what": {"Bound": "a"}}}])
)]
#[case::already_named(
    json!([{"Exile": {"what": {"Bound": "a"}, "as": "b"}}, {"Draw": {"count": {"Count": [{"Is": {"Bound": "a"}}]}}}]),
    json!([{"Exile": {"what": {"Bound": "a"}, "as": "b"}}, {"Draw": {"count": {"Count": [{"Is": {"Bound": "a"}}]}}}])
)]
#[case::sacrifice_of_bound_takes_all(
    json!([{"Sacrifice": {"who": {"Bound": "p"}, "filter": [{"Is": {"Bound": "rest"}}]}}]),
    json!([{"Sacrifice": {"who": {"Bound": "p"}, "filter": [{"Is": {"Bound": "rest"}}], "count": {"Count": [{"Is": {"Bound": "rest"}}]}}}])
)]
#[case::sacrifice_with_count_is_left_alone(
    json!([{"Sacrifice": {"filter": [{"Is": {"Bound": "x"}}], "count": 2}}]),
    json!([{"Sacrifice": {"filter": [{"Is": {"Bound": "x"}}], "count": 2}}])
)]
#[case::sacrifice_of_a_type_is_left_alone(
    json!([{"Sacrifice": {"filter": [{"Type": "Land"}]}}]),
    json!([{"Sacrifice": {"filter": [{"Type": "Land"}]}}])
)]
fn rebind_marks_only_moves_read_later(#[case] mut before: Value, #[case] after: Value) {
    rebind(&mut before);
    assert_eq!(before, after);
}
