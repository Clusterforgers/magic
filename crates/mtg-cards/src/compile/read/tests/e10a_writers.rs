//! Goal: targets that depend on earlier targets read back: a return of up to some or any number of target cards with a
//! total mana value (a fixed most or X), and "another target" after a target that may be a player; another most, another
//! count, a missing total, another zone or a missing "another" never reads back.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

const YARD: &str = r#"{"Object":[{"Type":"Creature"},{"OwnedBy":"You"},{"In":"Graveyard"}]}"#;

fn patch(bounds: &str, most: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"bounds":{bounds},"what":{{"Together":{{"rule":{{"TotalManaValueAtMost":{most}}},"what":{YARD}}}}}}}],"effects":[{{"Move":{{"what":{{"Target":0}},"to":{{"Battlefield":{{}}}}}}}}]}}}}"#
    )
}

const UP_TO_THREE: &str = r#"{"min":0,"max":3}"#;
const ANY: &str = r#"{"min":0,"max":{"Count":[{"Type":"Creature"},{"OwnedBy":"You"},{"In":"Graveyard"}]}}"#;
const ANY_OTHER: &str = r#"{"min":0,"max":{"Count":[{"Type":"Creature"},{"In":"Graveyard"}]}}"#;
const RETURN_THREE: &str = "Return up to three target creature cards with total mana value 3 or less from your graveyard to the battlefield.";

#[rstest]
#[case::up_to_three(patch(UP_TO_THREE, "3"), RETURN_THREE, true)]
#[case::any_number(
    patch(ANY, "8"),
    "Return any number of target creature cards with total mana value 8 or less from your graveyard to the battlefield.",
    true
)]
#[case::x(
    patch(UP_TO_THREE, r#""X""#),
    "Return up to three target creature cards with total mana value X or less from your graveyard to the battlefield.",
    true
)]
#[case::another_most(patch(UP_TO_THREE, "4"), RETURN_THREE, false)]
#[case::another_count(
    patch(r#"{"min":0,"max":2}"#, "3"),
    RETURN_THREE,
    false
)]
#[case::count_of_another_set(
    patch(ANY_OTHER, "8"),
    "Return any number of target creature cards with total mana value 8 or less from your graveyard to the battlefield.",
    false
)]
#[case::no_total_in_the_clause(
    patch(UP_TO_THREE, "3"),
    "Return up to three target creature cards from your graveyard to the battlefield.",
    false
)]
fn a_total_mana_value_reads_back(#[case] json: String, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(written(&json, clause), same, "{json}");
}

fn trail(second: &str) -> String {
    format!(
        r#"{{"Spell":{{"targets":[{{"what":"AnyTarget"}},{{"what":{second}}}],"effects":[{{"DealDamage":{{"to":{{"Target":0}},"amount":2}}}},{{"DealDamage":{{"to":{{"Target":1}},"amount":1}}}}]}}}}"#
    )
}

const ANOTHER: &str = r#"{"Another":{"what":"AnyTarget","earlier":[0]}}"#;
const TRAIL: &str = "~ deals 2 damage to any target and 1 damage to another target.";

#[rstest]
#[case::another(trail(ANOTHER), TRAIL, true)]
#[case::the_same_may_be_chosen(trail(r#""AnyTarget""#), TRAIL, false)]
#[case::another_in_the_clause_only(
    trail(ANOTHER),
    "~ deals 2 damage to any target and 1 damage to any target.",
    false
)]
fn another_target_reads_back(#[case] json: String, #[case] clause: &str, #[case] same: bool) {
    assert_eq!(written(&json, clause), same, "{json}");
}
