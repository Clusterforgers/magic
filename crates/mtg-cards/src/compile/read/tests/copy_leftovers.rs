//! Goal: a token copy read with the sentence that chooses its targets first ("Choose target creature you control. For each creature chosen this way, create a token that's a copy of it."), the sentence that gives the copy keywords ("Those tokens gain haste.") and the sentence that exiles or sacrifices it at the next end step or at end of combat reads into `CreateToken` of `CopyOf`, `Continuous` and `DelayedTrigger` of one bound copy, writes back as the same text in each wording of its pronouns, and leaves every count, noun or subject that does not match to the table.
use super::*;

#[rstest]
#[case::one_target(
    "Choose target creature you control. For each creature chosen this way, create a token that's a copy of it.",
    r#""CopyOf":{"of":{"Target":0}}"#
)]
#[case::up_to_two_with_haste_and_exile(
    "Choose up to two target creatures you don't control. For each of those creatures, create a token that's a copy of that creature. Those tokens gain haste. Exile them at the beginning of the next end step.",
    r#""duration":"Forever""#
)]
#[case::for_each_of_them(
    "Choose any number of target creatures you control. For each of them, create a token that's a copy of that creature, except it has haste. Exile those tokens at the beginning of the next end step.",
    r#""except":[{"AddKeyword":"Haste"}]"#
)]
#[case::several_alone(
    "Choose up to two target creatures you don't control. For each of those creatures, create a token that's a copy of that creature.",
    r#""CopyOf":{"of":{"Target":0}}"#
)]
#[case::gains_haste_and_is_sacrificed(
    "Create a token that's a copy of target creature you control. It gains haste until end of turn. Sacrifice it at the beginning of the next end step.",
    r#""Sacrifice""#
)]
#[case::that_token_at_end_of_combat(
    "Create a token that's a copy of target creature you control. That token gains haste until end of turn. Exile the token at end of combat.",
    r#""step":"EndCombat""#
)]
#[case::gains_haste_and_stays(
    "Create a token that's a copy of target creature you control. It gains haste until end of turn.",
    r#""as":"copy""#
)]
fn a_token_copy_with_what_happens_to_it(#[case] text: &str, #[case] part: &str) {
    let read = golden(text, ClauseKind::Spell, false);
    let json = serde_json::to_string(&read).unwrap_or_default();
    assert!(json.contains(part), "{json}");
}

#[rstest]
#[case::the_pronoun_of_several(
    "Create two tokens that are copies of target creature you control. It gains haste until end of turn."
)]
#[case::the_pronoun_of_one(
    "Create a token that's a copy of target creature you control. They gain haste until end of turn."
)]
#[case::another_verb(
    "Create a token that's a copy of target creature you control. It gets +1/+1 until end of turn. Exile it at the beginning of the next end step."
)]
#[case::another_step(
    "Create a token that's a copy of target creature you control. Exile it at the beginning of the next upkeep."
)]
#[case::the_wrong_noun(
    "Choose target creature you control. For each land chosen this way, create a token that's a copy of it."
)]
#[case::two_for_each(
    "Choose target creature you control. For each creature chosen this way, create two tokens that are copies of it."
)]
#[case::a_tapped_token(
    "Choose target creature you control. For each creature chosen this way, create a tapped token that's a copy of it."
)]
#[case::a_second_target(
    "Choose target creature you control. For each creature chosen this way, create a token that's a copy of target creature."
)]
fn what_is_not_the_shape_of_a_token_copy_stays_with_the_table(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
