//! Goal: the predefined tokens Blood, Junk and Powerstone (beside Treasure, Clue, Food and Gold) read into `CreateToken` of their file, alone, counted, tapped, for another player and in a trigger, and write back as the same text; a token no file describes stays with the table.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::blood(
    "Create a Blood token.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"blood"}}}]}}"#
)]
#[case::junk(
    "Create a Junk token.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"junk"}}}]}}"#
)]
#[case::powerstones(
    "Create two tapped Powerstone tokens.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"powerstone"},"count":2,"tapped":true}}]}}"#
)]
#[case::three_junk(
    "Create three Junk tokens.",
    r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Named":"junk"},"count":3}}]}}"#
)]
fn a_named_token_reads_in_the_shape_of_its_file(#[case] text: &str, #[case] json: &str) {
    reads_as(text, ClauseKind::Spell, false, json);
}

#[rstest]
#[case("Create a Blood token.")]
#[case("Create a Junk token.")]
#[case("Create a tapped Powerstone token.")]
#[case("Create two Blood tokens.")]
#[case("Exile target creature. Create a Gold token.")]
#[case("Destroy target artifact or enchantment. Create a Junk token.")]
#[case("Target opponent creates a Blood token.")]
#[case("Destroy target creature. Its controller creates a Junk token.")]
fn a_named_token_writes_back_as_the_text(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[rstest]
#[case("When this creature enters, create a Blood token.")]
#[case("Whenever a creature you control dies, create a Blood token.")]
#[case("Whenever this creature attacks, create a Junk token.")]
fn a_named_token_in_a_trigger_reads_and_writes_back(#[case] text: &str) {
    assert!(trigger(text).is_some(), "{text}");
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::no_file("Create a Shard token.")]
#[case::a_lower_case_name("Create a blood token.")]
fn a_token_without_a_file_stays_unread(#[case] text: &str) {
    assert!(spell(text).is_none(), "{text}");
}
