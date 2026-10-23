//! Goal: the exceptions "it isn't legendary" and "it's an artifact and it loses all other card types" of a token copy and of a copy of a spell read into `RemoveSupertype` and `SetTypes`, alone and joined with other exceptions, write back as the card's text, and the near-misses stay with the table.
use super::*;

const OF_TARGET: &str =
    r#""targets":[{"what":{"Object":[{"Type":"Creature"},{"ControlledBy":"You"}]}}]"#;

fn token_copy(except: &str) -> String {
    format!(
        r#"{{"Spell":{{{OF_TARGET},"effects":[{{"CreateToken":{{"token":{{"CopyOf":{{"of":{{"Target":0}},"except":[{except}]}}}}}}}}]}}}}"#
    )
}

#[rstest]
#[case::not_legendary(
    "Create a token that's a copy of target creature you control, except it isn't legendary.",
    r#"{"RemoveSupertype":"Legendary"}"#
)]
#[case::not_legendary_and_a_keyword(
    "Create a token that's a copy of target creature you control, except it isn't legendary and it has haste.",
    r#"{"RemoveSupertype":"Legendary"},{"AddKeyword":"Haste"}"#
)]
#[case::a_keyword_and_not_legendary(
    "Create a token that's a copy of target creature you control, except it has haste and it isn't legendary.",
    r#"{"AddKeyword":"Haste"},{"RemoveSupertype":"Legendary"}"#
)]
#[case::not_legendary_and_a_subtype(
    "Create a token that's a copy of target creature you control, except it isn't legendary and it's an Illusion in addition to its other types.",
    r#"{"RemoveSupertype":"Legendary"},{"AddSubtype":"Illusion"}"#
)]
#[case::only_an_artifact(
    "Create a token that's a copy of target creature you control, except it's an artifact and it loses all other card types.",
    r#"{"SetTypes":["Artifact"]}"#
)]
#[case::an_artifact_creature(
    "Create a token that's a copy of target creature you control, except it's an artifact creature and it loses all other card types.",
    r#"{"SetTypes":["Artifact","Creature"]}"#
)]
fn a_token_copy_may_lose_a_supertype_or_every_other_type(#[case] text: &str, #[case] except: &str) {
    reads_as(text, ClauseKind::Spell, false, &token_copy(except));
}

#[test]
fn the_token_is_worded_like_it() {
    let token = spell(
        "Create a token that's a copy of target creature you control, except the token isn't legendary.",
    );
    let it = spell(
        "Create a token that's a copy of target creature you control, except it isn't legendary.",
    );
    assert!(token.is_some());
    assert_eq!(token, it);
}

#[rstest]
#[case::the_copy_of_a_spell(
    "Whenever you cast a spell, copy it, except the copy isn't legendary.",
    r#"{"RemoveSupertype":"Legendary"}"#
)]
#[case::the_copy_of_a_spell_as_that(
    "Whenever you cast a spell, copy it, except that the copy isn't legendary.",
    r#"{"RemoveSupertype":"Legendary"}"#
)]
fn a_copy_of_a_spell_may_lose_a_supertype(#[case] text: &str, #[case] except: &str) {
    reads_as(
        text,
        ClauseKind::Triggered,
        false,
        &format!(
            r#"{{"Triggered":{{"when":{{"SpellCast":{{"by":"You"}}}},"effects":[{{"CopySpell":{{"what":"Triggering","except":[{except}]}}}}]}}}}"#
        ),
    );
}

#[test]
fn a_permanent_may_enter_as_a_copy_that_is_not_legendary() {
    reads_as(
        "You may have this creature enter as a copy of any creature on the battlefield, except it isn't legendary.",
        ClauseKind::Static,
        false,
        r#"{"Replacement":{"when":{"Enters":{"what":["This"]}},"by":{"AsCopy":{"of":[{"Type":"Creature"}],"except":[{"RemoveSupertype":"Legendary"}]}}}}"#,
    );
}

#[rstest]
#[case::another_supertype(
    "Create a token that's a copy of target creature you control, except it isn't basic."
)]
#[case::not_a_word("Create a token that's a copy of target creature you control, except it isn't.")]
#[case::loses_without_a_type(
    "Create a token that's a copy of target creature you control, except it loses all other card types."
)]
#[case::a_size_with_the_types(
    "Create a token that's a copy of target creature you control, except it's a 2/2 artifact and it loses all other card types."
)]
#[case::several_tokens_say_they(
    "Create two tokens that are copies of target creature you control, except they aren't legendary."
)]
fn near_misses_stay_with_the_table(#[case] text: &str) {
    assert_eq!(spell(text), None, "{text}");
}
