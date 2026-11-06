//! Goal: the writers of round 4 builder h word an ability as the card prints it, and only the data that means it.
use super::*;

fn written(json: &str, clause: &str) -> bool {
    written_variants(&ability(json))
        .iter()
        .any(|wording| same_text(wording, clause))
}

fn champion(choices: &str, then: &str) -> String {
    format!(
        r#"{{"Triggered":{{"when":{{"ZoneChange":{{"what":["This"],"to":"Battlefield"}}}},"effects":[
{{"Choose":{{"from":{{"Each":{choices}}},"count":{{"min":0,"max":1}},"as":"champion"}}}},
{{"Exile":{{"what":{{"Bound":"champion"}},"duration":"WhileSourceOnBattlefield"}}}},
{{"If":{{"condition":{{"Not":"Performed"}},"then":[{then}]}}}}]}}}}"#
    )
}

const SACRIFICE: &str = r#"{"Sacrifice":{"filter":["This"]}}"#;

#[rstest]
#[case::goblin(
    r#"[{"Subtype":"Goblin"},"Other",{"ControlledBy":"You"}]"#,
    "Champion a Goblin",
    true
)]
#[case::elf(
    r#"[{"Subtype":"Elf"},"Other",{"ControlledBy":"You"}]"#,
    "Champion an Elf",
    true
)]
#[case::creature(
    r#"[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]"#,
    "Champion a creature",
    true
)]
#[case::two(
    r#"[{"AnyOf":[{"Subtype":"Goblin"},{"Subtype":"Shaman"}]},"Other",{"ControlledBy":"You"}]"#,
    "Champion a Goblin or Shaman",
    true
)]
#[case::wrong_article(
    r#"[{"Subtype":"Elf"},"Other",{"ControlledBy":"You"}]"#,
    "Champion a Elf",
    false
)]
#[case::not_other(
    r#"[{"Subtype":"Goblin"},{"ControlledBy":"You"}]"#,
    "Champion a Goblin",
    false
)]
#[case::not_yours(r#"[{"Subtype":"Goblin"},"Other"]"#, "Champion a Goblin", false)]
fn champion_line(#[case] choices: &str, #[case] clause: &str, #[case] reads: bool) {
    assert_eq!(written(&champion(choices, SACRIFICE), clause), reads);
}

#[test]
fn a_trigger_that_does_not_sacrifice_is_no_champion() {
    let json = champion(
        r#"[{"Subtype":"Goblin"},"Other",{"ControlledBy":"You"}]"#,
        "",
    );
    assert!(!written(&json, "Champion a Goblin"));
}
