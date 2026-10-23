//! Goal: an X/X token whose size is an amount (a characteristic of an object, a count, a spell's mana value) reads into `TokenRef::Sized` and writes back as the same text with its where clause, alone and in a trigger.
use super::*;

fn trigger(text: &str) -> Option<Ability> {
    clause(text, ClauseKind::Triggered, false)
}

#[rstest]
#[case::its_power(
    "When this creature dies, create an X/X black Horror creature token, where X is this creature's power."
)]
#[case::lands_in_a_graveyard(
    "When this creature dies, create an X/X colorless Shapeshifter creature token with changeling and deathtouch, where X is the number of land cards in your graveyard."
)]
#[case::creatures_you_control(
    "When this creature enters, create an X/X green Beast creature token, where X is the number of creatures you control."
)]
fn a_token_sized_by_an_amount_writes_back_in_a_trigger(#[case] text: &str) {
    assert!(trigger(text).is_some(), "{text}");
    golden(text, ClauseKind::Triggered, false);
}

#[rstest]
#[case::a_count(
    "Create an X/X colorless Shapeshifter creature token with changeling and deathtouch, where X is the number of land cards in your graveyard."
)]
#[case::creatures_that_died(
    "Create an X/X black Horror creature token, where X is the number of creatures that died this turn."
)]
fn a_token_sized_by_a_count_writes_back_as_a_spell(#[case] text: &str) {
    golden(text, ClauseKind::Spell, false);
}

#[test]
fn the_size_is_the_amount_of_the_where_clause() {
    reads_as(
        "Create an X/X colorless Shapeshifter creature token with deathtouch, where X is the number of land cards in your graveyard.",
        ClauseKind::Spell,
        false,
        r#"{"Spell":{"effects":[{"CreateToken":{"token":{"Sized":{"size":{"Count":[{"Type":"Land"},{"In":"Graveyard"},{"OwnedBy":"You"}]},"token":{"name":"Shapeshifter","types":["Creature"],"subtypes":["Shapeshifter"],"abilities":[{"Keyword":"Deathtouch"}]}}}}}]}}"#,
    );
}

#[rstest]
#[case::no_where_clause_and_no_x_in_the_cost("Create an X/X green Beast creature token.")]
#[case::a_size_that_is_a_number_and_x(
    "Create an X/3 green Beast creature token, where X is the number of creatures you control."
)]
fn a_size_with_nothing_to_name_it_stays_unread(#[case] text: &str) {
    let read = spell(text);
    assert!(
        read.as_ref().is_none_or(|ability| render_variants(ability)
            .iter()
            .all(|written| !same_text(written, text))),
        "{text}: {read:?}"
    );
}
