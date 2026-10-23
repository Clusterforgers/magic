//! Goal: an Equipment or Aura that gives "is a Monk in addition to its other types" reads into `AddSubtype` after its bonus and keywords, writes back as the card's own text, and a kind the rules do not list or a missing tail stays with the table.
use super::*;

#[rstest]
#[case::a_bonus_and_a_kind(
    "Equipped creature gets +1/+0 and is a Monk in addition to its other types.",
    r#"{"Static":{"mods":[{"ModifyPT":{"power":1,"toughness":0}},{"AddSubtype":"Monk"}],"affected":"Attached"}}"#
)]
#[case::a_kind_with_a_vowel(
    "Equipped creature gets +2/+2 and is an Assassin in addition to its other types.",
    r#"{"Static":{"mods":[{"ModifyPT":{"power":2,"toughness":2}},{"AddSubtype":"Assassin"}],"affected":"Attached"}}"#
)]
#[case::a_bonus_keywords_and_a_kind(
    "Equipped creature gets +2/+2, has trample and haste, and is a Samurai in addition to its other types.",
    r#"{"Static":{"mods":[{"ModifyPT":{"power":2,"toughness":2}},{"AddKeyword":"Trample"},{"AddKeyword":"Haste"},{"AddSubtype":"Samurai"}],"affected":"Attached"}}"#
)]
#[case::an_aura(
    "Enchanted creature gets +1/+1 and is a Warrior in addition to its other types.",
    r#"{"Static":{"mods":[{"ModifyPT":{"power":1,"toughness":1}},{"AddSubtype":"Warrior"}],"affected":"Attached"}}"#
)]
fn the_kind_after_a_bonus_reads_and_writes_back(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Static, false, json);
}

#[rstest]
#[case::no_tail("Equipped creature gets +1/+0 and is a Monk.")]
#[case::a_word_that_is_no_kind(
    "Equipped creature gets +1/+0 and is a Banana in addition to its other types."
)]
#[case::no_article("Equipped creature gets +1/+0 and is Monk in addition to its other types.")]
#[case::a_quoted_trigger(
    "Equipped creature gets +1/+1, has \"Whenever this creature attacks, you gain 1 life,\" and is a Cleric in addition to its other types."
)]
fn a_tail_the_template_does_not_model_is_left_to_the_table(#[case] text: &str) {
    assert!(clause(text, ClauseKind::Static, false).is_none(), "{text}");
}
