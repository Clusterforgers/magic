//! Goal: two statics in one sentence after one condition (the card gets a bonus and a group gets a bonus or a keyword) read into the two abilities in order with the same condition, write back as the one sentence, an ability word before it changes nothing, and a sentence of a different shape stays with the table.
use super::*;
use crate::compile::read::{Own, same_text};
use crate::compile::read::{static_pair, static_pair_texts};

/// "As long as you control your commander" as a condition.
const COMMANDER: &str = r#"{"Compare":{"left":{"Count":["Commander",{"OwnedBy":"You"},{"ControlledBy":"You"}]},"op":"Ge","right":1}}"#;

fn pair(text: &str) -> Option<Vec<Ability>> {
    static_pair(text, false, &Own::default())
}

fn both(group: &str, mods: &str) -> Vec<Ability> {
    vec![
        ability(&format!(
            r#"{{"Static":{{"affected":"This","mods":[{{"ModifyPT":{{"power":2,"toughness":2}}}}],"condition":{COMMANDER}}}}}"#
        )),
        ability(&format!(
            r#"{{"Static":{{"affected":{{"Each":{group}}},"mods":{mods},"condition":{COMMANDER}}}}}"#
        )),
    ]
}

#[rstest]
#[case::a_keyword_for_the_creatures_you_control(
    "As long as you control your commander, ~ gets +2/+2 and creatures you control have vigilance.",
    r#"[{"Type":"Creature"},{"ControlledBy":"You"}]"#,
    r#"[{"AddKeyword":"Vigilance"}]"#
)]
#[case::a_bonus_and_a_keyword_for_the_others(
    "As long as you control your commander, ~ gets +2/+2 and other creatures you control get +2/+2 and have trample.",
    r#"[{"Type":"Creature"},"Other",{"ControlledBy":"You"}]"#,
    r#"[{"ModifyPT":{"power":2,"toughness":2}},{"AddKeyword":"Trample"}]"#
)]
#[case::a_bonus_for_a_subtype(
    "As long as you control your commander, ~ gets +2/+2 and other Elves you control get +2/+2.",
    r#"[{"Subtype":"Elf"},"Other",{"ControlledBy":"You"}]"#,
    r#"[{"ModifyPT":{"power":2,"toughness":2}}]"#
)]
fn two_statics_read_in_order_and_write_back(
    #[case] text: &str,
    #[case] group: &str,
    #[case] mods: &str,
) {
    let read = pair(text).unwrap_or_else(|| panic!("not read: {text}"));
    assert_eq!(read, both(group, mods), "{text}");
    let written = static_pair_texts(&read[0], &read[1]);
    assert!(
        written.iter().any(|variant| same_text(variant, text)),
        "{text}: {written:?}"
    );
}

#[rstest]
#[case::the_ability_word_is_flavor(
    "Lieutenant \u{2014} As long as you control your commander, ~ gets +2/+2 and creatures you control have vigilance."
)]
fn the_ability_word_changes_nothing(#[case] with: &str) {
    let plain = with.split(" \u{2014} ").last().unwrap_or(with);
    assert!(pair(with).is_some(), "{with}");
    assert_eq!(pair(with), pair(plain), "{with}");
}

#[rstest]
#[case::no_condition("~ gets +2/+2 and creatures you control have vigilance.")]
#[case::a_condition_after(
    "~ gets +2/+2 and creatures you control have vigilance as long as you control your commander."
)]
#[case::the_card_gets_a_keyword(
    "As long as you control your commander, ~ has flying and creatures you control have vigilance."
)]
#[case::an_unknown_keyword(
    "As long as you control your commander, ~ gets +2/+2 and creatures you control have frobnication."
)]
#[case::a_single_creature(
    "As long as you control your commander, ~ gets +2/+2 and target creature has vigilance."
)]
#[case::a_third_part(
    "As long as you control your commander, ~ gets +2/+2 and creatures you control have vigilance and lands you control have haste."
)]
fn a_near_miss_is_left_to_the_table(#[case] text: &str) {
    assert_eq!(pair(text), None, "{text}");
    assert_eq!(clause(text, ClauseKind::Static, false), None, "{text}");
}

#[rstest]
#[case::a_different_condition(
    r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":2,"toughness":2}}],"condition":"YourTurn"}}"#,
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"AddKeyword":"Vigilance"}],"condition":{"Compare":{"left":{"Count":["Commander"]},"op":"Ge","right":1}}}}"#
)]
#[case::no_condition(
    r#"{"Static":{"affected":"This","mods":[{"ModifyPT":{"power":2,"toughness":2}}]}}"#,
    r#"{"Static":{"affected":{"Each":[{"Type":"Creature"}]},"mods":[{"AddKeyword":"Vigilance"}]}}"#
)]
fn two_statics_that_do_not_share_a_condition_are_two_sentences(
    #[case] first: &str,
    #[case] second: &str,
) {
    assert_eq!(
        static_pair_texts(&ability(first), &ability(second)),
        Vec::<String>::new()
    );
}
