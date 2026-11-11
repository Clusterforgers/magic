//! Goal: the pieces of ep25 translate as the card prints them: a `Repeat` the caster may go on with asks them before each pass after the first, with a check, a bound or neither, and a target on the line is the target its body names; another decider, another value of the key and a check before the next pass stay gaps.
use rstest::rstest;
use serde_json::{Value, json};

use super::{abilities, gaps};

fn spell(line: &str) -> String {
    format!(
        "Name:T\nTypes:Sorcery\nA:SP$ {line}\nSVar:Body:DB$ GainLife | LifeAmount$ 2\nSVar:Hit:DB$ LoseLife | LifeAmount$ 1 | Defined$ Targeted\nSVar:X:Count$ValidHand Card.YouOwn\nOracle:Target opponent loses 1 life. You may repeat this process any number of times.\n"
    )
}

fn spell_ability(script: &str) -> Value {
    let found = abilities(script);
    assert_eq!(found.len(), 1, "{:?}", gaps(script));
    found[0]["Spell"].clone()
}

const ALWAYS: &str = r#"{"Compare": {"left": 0, "op": "Eq", "right": 0}}"#;

#[rstest]
#[case::until_stopped("", ALWAYS, 1000)]
#[case::bounded("MaxRepeat$ 3 | ", ALWAYS, 3)]
#[case::checked(
    "RepeatCheckSVar$ X | RepeatSVarCompare$ LT7 | ",
    r#"{"Compare": {"left": {"Count": [{"OwnedBy": "You"}, {"In": "Hand"}]}, "op": "Lt", "right": 7}}"#,
    1000
)]
fn ep25_an_optional_repeat_asks_the_caster_after_each_pass(
    #[case] keys: &str,
    #[case] condition: &str,
    #[case] most: u32,
) {
    let script = spell(&format!(
        "Repeat | RepeatSubAbility$ Body | {keys}RepeatOptional$ True"
    ));
    assert_eq!(gaps(&script), Vec::<String>::new());
    let condition: Value = serde_json::from_str(condition).unwrap();
    assert_eq!(
        spell_ability(&script)["effects"],
        json!([{"RepeatWhile": {"ask": "You", "at_least_once": true, "condition": condition,
            "effects": [{"GainLife": {"amount": 2}}], "most": most}}])
    );
}

#[test]
fn ep25_a_target_on_the_repeat_is_the_target_of_its_body() {
    let script =
        spell("Repeat | ValidTgts$ Opponent | RepeatSubAbility$ Hit | RepeatOptional$ True");
    assert_eq!(gaps(&script), Vec::<String>::new());
    let ability = spell_ability(&script);
    assert_eq!(ability["targets"], json!([{"what": "Opponent"}]));
    assert_eq!(
        ability["effects"][0]["RepeatWhile"]["effects"],
        json!([{"LoseLife": {"amount": 1, "who": {"Target": 0}}}])
    );
}

#[rstest]
#[case::another_decider(
    "RepeatOptional$ True | RepeatOptionalDecider$ Remembered",
    "key:Repeat.RepeatOptionalDecider"
)]
#[case::another_value("RepeatOptional$ False", "key:Repeat.RepeatOptional=False")]
#[case::not_optional_and_unbounded(
    "RepeatCheckSVar$ X | RepeatSVarCompare$ LT7 | StackDescription$ If you have fewer than 7 cards in hand, repeat this process. | RepeatOptional$ True",
    "key:Repeat.check-first"
)]
fn ep25_other_optional_repeats_stay_gaps(#[case] keys: &str, #[case] gap: &str) {
    let script = spell(&format!("Repeat | RepeatSubAbility$ Body | {keys}"));
    assert_eq!(gaps(&script), [gap]);
}
